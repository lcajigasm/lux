//! Real PTY-backed process, VT100/xterm emulation, resize and signals.
//!
//! `portable-pty` opens the actual OS pseudo-terminal (ConPTY on Windows)
//! and owns the child process; `alacritty_terminal` turns the raw bytes it
//! produces into grid state. See `docs/adr/0012-terminal-real-pty.md`.

use std::io::{self, Read, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Line;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
use portable_pty::{Child, MasterPty, native_pty_system};

pub use portable_pty::{CommandBuilder, ExitStatus, PtySize};

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("failed to open pty: {0}")]
    Open(#[source] anyhow::Error),
    #[error("failed to spawn command: {0}")]
    Spawn(#[source] anyhow::Error),
    #[error("failed to resize pty: {0}")]
    Resize(#[source] anyhow::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

struct GridSize {
    rows: usize,
    cols: usize,
}

impl GridSize {
    fn from_pty_size(size: PtySize) -> Self {
        Self {
            rows: size.rows as usize,
            cols: size.cols as usize,
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

#[derive(Clone)]
pub(crate) struct ChannelListener(mpsc::Sender<Event>);

impl EventListener for ChannelListener {
    fn send_event(&self, event: Event) {
        // The other end drops as soon as the Pty (and its Receiver) does;
        // a send failing at that point just means nobody is listening.
        let _ = self.0.send(event);
    }
}

/// A real, persistent PTY-backed process with VT100/xterm emulation.
///
/// Ctrl-C, Ctrl-Z and friends need no dedicated API: they are ordinary
/// bytes (`0x03`, `0x1a`, ...) written to the pty, and the kernel's tty
/// line discipline turns them into real `SIGINT`/`SIGTSTP` delivered to
/// the child's foreground process group — the same mechanism a real
/// terminal emulator relies on. Resize is the other half of that: writing
/// the new size to the kernel makes it deliver `SIGWINCH`.
pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn Child + Send + Sync>,
    pub(crate) term: Arc<FairMutex<Term<ChannelListener>>>,
    events: Receiver<Event>,
    reader: Option<JoinHandle<()>>,
}

impl Pty {
    /// Spawn `command` into a new real PTY of the given size.
    pub fn spawn(command: CommandBuilder, size: PtySize) -> Result<Self, PtyError> {
        let pair = native_pty_system().openpty(size).map_err(PtyError::Open)?;
        let child = pair.slave.spawn_command(command).map_err(PtyError::Spawn)?;
        // Drop our copy of the slave now that the child holds its own via
        // inherited stdio: it's the last reference closing that makes the
        // master reader see EOF when the child exits (see `bash.rs` in
        // portable-pty's own examples).
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(PtyError::Open)?;
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(
            pair.master.take_writer().map_err(PtyError::Open)?,
        ));

        // `term_tx` feeds `alacritty_terminal`'s own events to the reader
        // thread below, which answers terminal queries (cursor position
        // reports, ...) itself and republishes everything else through
        // `events` for callers to observe via `try_recv_event`.
        let (term_tx, term_rx) = mpsc::channel();
        let (events_tx, events) = mpsc::channel();
        let term = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &GridSize::from_pty_size(size),
            ChannelListener(term_tx),
        )));

        let reader_term = Arc::clone(&term);
        let reply_writer = Arc::clone(&writer);
        let reader = std::thread::spawn(move || {
            let mut processor = Processor::<StdSyncHandler>::new();
            let mut buf = [0u8; 8192];
            let mut total = 0usize;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        eprintln!("lux-term: pty reader saw EOF after {total} bytes total");
                        break;
                    }
                    // A real I/O error ends the terminal same as EOF, but
                    // silently was wrong: it left no trace of *why* the
                    // grid stopped updating. `eprintln!` here is cheap
                    // and, unlike a panic, can't take the whole process
                    // down over what a real terminal treats as "the
                    // stream closed."
                    Err(err) => {
                        eprintln!("lux-term: pty reader stopped after {total} bytes: {err}");
                        break;
                    }
                    Ok(n) => {
                        total += n;
                        processor.advance(&mut *reader_term.lock(), &buf[..n]);

                        // Any real terminal emulator must answer control
                        // queries the client sends it (cursor position
                        // reports and the like) — `alacritty_terminal`
                        // already computes the correct reply, we just have
                        // to write it back. Windows' ConPTY sends a cursor
                        // position query (`ESC[6n`) as part of its own
                        // startup handshake and *blocks all further output*
                        // until it gets an answer — found via a real,
                        // reported upstream issue
                        // (wezterm/wezterm#6783), not a guess: this is why
                        // spawning a process was hanging with an
                        // eternally-blank grid only on Windows.
                        while let Ok(event) = term_rx.try_recv() {
                            match event {
                                Event::PtyWrite(text) => {
                                    if let Ok(mut w) = reply_writer.lock() {
                                        let _ = w.write_all(text.as_bytes());
                                    }
                                }
                                other => {
                                    let _ = events_tx.send(other);
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            child,
            term,
            events,
            reader: Some(reader),
        })
    }

    /// Write raw bytes (keystrokes, pastes) to the child's stdin.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        self.writer.lock().unwrap().write_all(bytes)?;
        Ok(())
    }

    /// Resize both the kernel's notion of the pty and the VT grid.
    pub fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        self.master.resize(size).map_err(PtyError::Resize)?;
        self.term.lock().resize(GridSize::from_pty_size(size));
        Ok(())
    }

    /// Render the current viewport as plain text, one line per row —
    /// honors `scroll`, so a scrolled-back viewport renders scrollback
    /// content instead of always the live bottom.
    pub fn snapshot(&self) -> String {
        let term = self.term.lock();
        let grid = term.grid();
        let top = -(grid.display_offset() as i32);
        let mut lines = Vec::with_capacity(grid.screen_lines());
        for row in 0..grid.screen_lines() {
            let mut text = String::with_capacity(grid.columns());
            for cell in &grid[Line(top + row as i32)] {
                text.push(cell.c);
            }
            lines.push(text.trim_end().to_string());
        }
        lines.join("\n")
    }

    /// The VT grid's current (rows, cols) — reflects the last `resize` call.
    pub fn size(&self) -> (usize, usize) {
        let term = self.term.lock();
        (term.grid().screen_lines(), term.grid().columns())
    }

    /// The child's process id, if the platform exposes one.
    pub fn pid(&self) -> Option<u32> {
        self.child.process_id()
    }

    /// Non-blocking poll for the child's exit status.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>, PtyError> {
        Ok(self.child.try_wait()?)
    }

    /// Next pending terminal event (title change, bell, exit...), if any.
    pub fn try_recv_event(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Terminate the child and block until it is reaped, leaving no
    /// orphaned or zombie process behind.
    pub fn kill_and_wait(self) -> Result<ExitStatus, PtyError> {
        let Pty {
            master,
            writer,
            mut child,
            reader,
            ..
        } = self;

        // Best-effort: if it's already dead, `kill` erroring is fine, the
        // `wait` below still reaps it.
        let _ = child.kill();
        let status = child.wait()?;

        // Drop our copy of the writer and the master pty handle *before*
        // joining the reader thread. `master` is the load-bearing one: on
        // Unix, closing our `slave` copy at spawn time plus the child's
        // own exit already produce EOF independent of `master`'s
        // lifetime — but on Windows, `portable-pty`'s ConPTY master and
        // slave share the same underlying `Arc<Mutex<Inner>>` (see
        // `portable-pty`'s own `src/win/conpty.rs`), so dropping our
        // `slave` copy at spawn time was a no-op there. The
        // pseudoconsole — and the OS pipe handle it owns — only actually
        // closes, via `ClosePseudoConsole`, when the *last* reference to
        // that shared `Inner` drops. `master` stays alive for `Pty`'s
        // whole life (needed for `resize`), so without dropping it here
        // first, the reader thread's blocking read on that pipe would
        // never see EOF while we're joining it below: a real deadlock,
        // confirmed by a 6-hour CI hang on real Windows before this fix,
        // not a hypothetical one. `writer` is shared with the reader
        // thread too (it auto-answers terminal queries), so dropping our
        // copy here is just tidiness, not itself load-bearing.
        drop(writer);
        drop(master);

        if let Some(reader) = reader {
            let _ = reader.join();
        }
        Ok(status)
    }
}

// Unix-only: every test here spawns `/bin/sh`/`/bin/cat` and asserts on
// POSIX signal semantics. `tests/conpty.rs` pins the analogous
// Windows/ConPTY behavior against `cmd.exe`.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn small_size() -> PtySize {
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if condition() {
                return true;
            }
            if Instant::now() >= deadline {
                return condition();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn spawn_and_echo_reaches_the_grid() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "printf hello-lux"]);
        let mut pty = Pty::spawn(cmd, small_size()).expect("spawn real shell");

        assert!(
            wait_until(Duration::from_secs(5), || pty
                .snapshot()
                .contains("hello-lux")),
            "expected output missing from grid:\n{}",
            pty.snapshot()
        );

        let status = wait_until(Duration::from_secs(5), || {
            pty.try_wait().expect("poll child").is_some()
        });
        assert!(status, "child did not exit");
    }

    #[test]
    fn resize_reaches_the_kernel_winsize() {
        let cmd = CommandBuilder::new("/bin/sh");
        let mut pty = Pty::spawn(cmd, small_size()).expect("spawn real shell");

        pty.resize(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resize pty");

        pty.write(b"stty size\r").expect("write to pty");

        assert!(
            wait_until(Duration::from_secs(5), || pty.snapshot().contains("40 120")),
            "shell did not observe the resized kernel winsize:\n{}",
            pty.snapshot()
        );

        pty.write(b"exit\r").expect("write to pty");
    }

    #[test]
    fn control_byte_delivers_a_real_signal() {
        // `cat` with no args blocks reading stdin until it is signaled.
        let cmd = CommandBuilder::new("/bin/cat");
        let mut pty = Pty::spawn(cmd, small_size()).expect("spawn real cat");

        pty.write(&[0x03]).expect("write ETX (Ctrl-C)"); // SIGINT

        let status = {
            let mut found = None;
            wait_until(Duration::from_secs(5), || {
                found = pty.try_wait().expect("poll child");
                found.is_some()
            });
            found.expect("cat did not exit after SIGINT")
        };

        assert!(!status.success(), "cat should have been killed by a signal");
        // `strsignal`'s exact wording is platform/locale-dependent (macOS:
        // "Interrupt: 2", Linux: "Interrupt") — what matters is that a real
        // signal, not a plain exit code, ended the process.
        let signal = status.signal().expect("cat should have died from a signal");
        assert!(
            signal.to_ascii_lowercase().contains("interrupt"),
            "expected an interrupt signal, got {signal:?}"
        );
    }

    #[test]
    fn kill_and_wait_leaves_no_orphan() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "sleep 30"]);
        let pty = Pty::spawn(cmd, small_size()).expect("spawn real shell");
        let pid = pty.pid().expect("child pid");

        let status = pty.kill_and_wait().expect("kill and reap child");
        assert!(!status.success());

        // ESRCH: no such process. Confirms the child was actually reaped,
        // not merely signaled and left as a zombie.
        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        assert_eq!(rc, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}

// Portable — no real process involved, just `alacritty_terminal`'s own VT
// parser, so this runs on every platform, Windows included.
#[cfg(test)]
mod device_status_tests {
    use super::*;

    #[test]
    fn cursor_position_queries_get_a_real_answer() {
        // The exact mechanism `Pty::spawn`'s reader thread relies on:
        // ConPTY sends `ESC[6n` (a cursor position query) as part of its
        // own Windows startup handshake and blocks all further output
        // until it's answered (confirmed via wezterm/wezterm#6783) — a
        // real terminal must answer it, not just relay bytes one way.
        // `Term` already computes the correct reply; this pins that it
        // keeps doing so.
        let (tx, rx) = mpsc::channel();
        let mut term = Term::new(
            Config::default(),
            &GridSize { rows: 24, cols: 80 },
            ChannelListener(tx),
        );
        let mut processor = Processor::<StdSyncHandler>::new();
        processor.advance(&mut term, b"\x1b[6n");

        match rx.try_recv() {
            Ok(Event::PtyWrite(text)) => assert_eq!(text, "\x1b[1;1R"),
            Ok(_) => panic!("expected a PtyWrite reply to the cursor position query"),
            Err(_) => panic!("device_status did not emit any reply at all"),
        }
    }
}
