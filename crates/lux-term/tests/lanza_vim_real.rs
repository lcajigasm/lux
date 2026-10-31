//! Launches a real `vim` process through a real PTY: persistent process,
//! live resize, and clean shutdown with no orphaned process — the concrete
//! proof Phase 8's acceptance criteria (LUX_PROMPT_MAESTRO.md) asks for.
//!
//! Nested under `pty` so the qualified test name is `pty::lanza_vim_real`,
//! matching the crate's own `pty::` unit test namespace.

mod pty {
    use std::time::{Duration, Instant};

    use lux_term::{CommandBuilder, Pty, PtySize};

    fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if condition() {
                return true;
            }
            if Instant::now() >= deadline {
                return condition();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn lanza_vim_real() {
        let mut cmd = CommandBuilder::new("vim");
        // No vimrc/plugins and no swapfile: a deterministic screen to assert on.
        cmd.args(["-u", "NONE", "-n"]);
        cmd.env("TERM", "xterm-256color");

        let mut pty = Pty::spawn(
            cmd,
            PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        )
        .expect("spawn real vim through a real pty");

        // 60s: this workspace has grown substantially (plugins, chrome,
        // pack, mobile hosts), and `cargo test --workspace` on a shared
        // CI runner now has far more concurrent load than when this test
        // was written — confirmed by CI history: this exact test passed
        // cleanly right after it was last touched, and only started
        // flaking once unrelated commits grew the workspace. Bumped
        // twice already (5s -> 15s -> 30s) and still exhausted 30s on a
        // busy ubuntu-latest run.
        assert!(
            wait_until(Duration::from_secs(60), || pty.snapshot().contains("VIM")),
            "vim did not draw its startup screen:\n{}",
            pty.snapshot()
        );

        // Live resize, as if the user dragged the panel edge.
        pty.resize(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resize the real pty");

        assert_eq!(pty.size(), (40, 120), "grid did not adopt the new size");

        // Ask vim itself what size it thinks the terminal is: this only
        // matches if the kernel actually delivered SIGWINCH to it and vim
        // re-queried the pty's winsize, not just that our own grid resized.
        // One write is not enough on a busy runner: SIGWINCH redraws the
        // screen and drops that command line, and waiting cannot recover a
        // query that was never accepted. Re-issue it until the answer shows.
        let mut last_query = Instant::now() - Duration::from_secs(1);
        assert!(
            wait_until(Duration::from_secs(60), || {
                if last_query.elapsed() >= Duration::from_secs(1) {
                    pty.write(b"\x1b:echo &columns . \"x\" . &lines\r")
                        .expect("write the size-echo command");
                    last_query = Instant::now();
                }
                pty.snapshot().contains("120x40")
            }),
            "vim did not observe the resize (no SIGWINCH?):\n{}",
            pty.snapshot()
        );

        pty.write(b":q!\r").expect("write the quit command");

        let mut exit_status = None;
        let exited = wait_until(Duration::from_secs(60), || {
            exit_status = pty.try_wait().expect("poll the child");
            exit_status.is_some()
        });
        assert!(exited, "vim did not exit after :q!");
        assert!(
            exit_status.expect("checked above").success(),
            "vim did not exit cleanly"
        );
    }
}
