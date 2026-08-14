//! Pins `lux_term::Pty`'s behavior under real Windows ConPTY.
//!
//! `portable-pty`'s `NativePtySystem` already resolves to its own
//! `win::conpty::ConPtySystem` on Windows (`#[cfg(windows)] pub type
//! NativePtySystem = win::conpty::ConPtySystem;`), so `Pty::spawn` from
//! the "PTY real / señales / resize" deliverable already goes through
//! ConPTY whenever this crate is compiled for Windows — there is no
//! separate ConPTY-specific production code path to add. What this file
//! pins is real, observed ConPTY behavior per ADR 0012's own guidance
//! ("Phase 8 should write tests that pin portable-pty's currently-observed
//! behavior ... ConPTY quirks on Windows"), using `cmd.exe` instead of the
//! Unix shells the `pty::` tests use.
//!
//! Compiled and gated for Windows only (`#[cfg(windows)]`): on every other
//! platform this file has zero tests, not a fake pass — it simply doesn't
//! build for a target it can't say anything true about. It has not been
//! run on real Windows from this session (no Windows/Wine execution
//! environment was available); see
//! `docs/receipts/phase-08-conpty-windows.md` for that gap and what
//! verifying this for real requires (the repo's own `windows-latest` CI
//! job, or direct access to a Windows machine).

#![cfg(windows)]

mod conpty {
    use std::time::{Duration, Instant};

    use lux_term::{CommandBuilder, Pty, PtySize};

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
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn spawns_a_real_process_through_conpty() {
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.args(["/C", "echo hello-lux-conpty"]);
        let mut pty = Pty::spawn(cmd, small_size()).expect("spawn cmd.exe via real ConPTY");

        assert!(
            wait_until(Duration::from_secs(5), || pty
                .snapshot()
                .contains("hello-lux-conpty")),
            "expected output missing from the ConPTY-backed grid:\n{}",
            pty.snapshot()
        );

        assert!(
            wait_until(Duration::from_secs(5), || pty
                .try_wait()
                .expect("poll child")
                .is_some()),
            "cmd.exe did not exit"
        );
    }

    #[test]
    fn resize_reaches_the_real_conpty_pseudoconsole() {
        let cmd = CommandBuilder::new("cmd.exe");
        let pty = Pty::spawn(cmd, small_size()).expect("spawn cmd.exe via real ConPTY");

        assert_eq!(pty.size(), (24, 80));

        // `MasterPty::resize` on Windows calls ConPTY's own
        // `ResizePseudoConsole`, distinct from Unix's `TIOCSWINSZ` ioctl —
        // the actual thing this deliverable is about pinning.
        pty.resize(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resize the real ConPTY pseudoconsole");

        assert_eq!(pty.size(), (40, 120));
    }

    #[test]
    fn kill_and_wait_reaps_a_conpty_child_without_hanging() {
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.args(["/C", "timeout /T 30"]);
        let pty = Pty::spawn(cmd, small_size()).expect("spawn cmd.exe via real ConPTY");

        // portable-pty's Windows `ChildKiller` calls `TerminateProcess`
        // directly (no signal name, exit code 1) rather than Unix's
        // SIGHUP-then-SIGKILL grace period — read from `portable-pty`'s
        // own `src/win/mod.rs`, not guessed.
        let status = pty.kill_and_wait().expect("kill and reap the ConPTY child");
        assert!(!status.success());
    }
}
