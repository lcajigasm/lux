//! The PTY's child process, watched by a thread blocked in `wait()`.
//!
//! The thread is the child-exit notification: it records the exit status
//! the moment the OS reports it and runs the caller's notice. Nothing polls
//! for the exit (CLAUDE.md rule 5).

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use portable_pty::{Child, ChildKiller, ExitStatus};

use crate::pty::PtyError;

/// Runs once, on the waiter thread, after the child has exited and the
/// reader has finished turning its output into grid state.
pub type ExitNotice = Box<dyn FnOnce(&ExitStatus) + Send>;

/// How long a notice waits for the reader to finish the output the child
/// wrote before it exited. On Unix the reader sees EOF right after the exit;
/// a grandchild that keeps the terminal open (or ConPTY, which reports EOF
/// only when the pseudoconsole closes) must not hold the notice forever.
const DRAIN: Duration = Duration::from_millis(500);

/// Grace period between the polite hangup and the forced kill.
const GRACE: Duration = Duration::from_millis(250);

/// Tells the waiter the PTY reader has stopped, however the reader ends.
pub(crate) struct DropSignal(pub(crate) std::sync::mpsc::Sender<()>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[derive(Default)]
struct Exit {
    status: Mutex<Option<ExitStatus>>,
    done: Condvar,
}

pub(crate) struct ChildWatch {
    pid: Option<u32>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    exit: Arc<Exit>,
}

impl ChildWatch {
    pub(crate) fn spawn(
        mut child: Box<dyn Child + Send + Sync>,
        drained: Receiver<()>,
        notice: Option<ExitNotice>,
    ) -> Self {
        let pid = child.process_id();
        let killer = child.clone_killer();
        let exit = Arc::new(Exit::default());
        let shared = Arc::clone(&exit);
        std::thread::Builder::new()
            .name("lux-pty-wait".into())
            .spawn(move || {
                // An error from `wait` means the child is gone and cannot be
                // reaped again; report it as a failed exit.
                let status = child
                    .wait()
                    .unwrap_or_else(|_| ExitStatus::with_exit_code(1));
                if let Ok(mut slot) = shared.status.lock() {
                    *slot = Some(status.clone());
                }
                shared.done.notify_all();
                if let Some(notice) = notice {
                    let _ = drained.recv_timeout(DRAIN);
                    notice(&status);
                }
            })
            .map(drop)
            .unwrap_or_else(|error| eprintln!("lux-term: cannot watch child: {error}"));
        Self { pid, killer, exit }
    }

    pub(crate) fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub(crate) fn try_wait(&self) -> Option<ExitStatus> {
        self.exit.status.lock().ok().and_then(|slot| slot.clone())
    }

    fn wait_timeout(&self, timeout: Option<Duration>) -> Option<ExitStatus> {
        let mut slot = self.exit.status.lock().ok()?;
        match timeout {
            Some(timeout) => {
                let (guard, _) = self
                    .exit
                    .done
                    .wait_timeout_while(slot, timeout, |s| s.is_none())
                    .ok()?;
                guard.clone()
            }
            None => {
                while slot.is_none() {
                    slot = self.exit.done.wait(slot).ok()?;
                }
                slot.clone()
            }
        }
    }

    /// Asks the child to stop and returns at once; the waiter thread
    /// reports the exit. A child that ignores the hangup is killed after a
    /// short grace period by a helper thread.
    pub(crate) fn request_kill(&mut self) {
        if self.try_wait().is_some() {
            return;
        }
        let _ = self.killer.kill();
        let exit = Arc::clone(&self.exit);
        let pid = self.pid;
        let mut killer = self.killer.clone_killer();
        let _ = std::thread::Builder::new()
            .name("lux-pty-kill".into())
            .spawn(move || {
                let Ok(slot) = exit.status.lock() else {
                    return;
                };
                let Ok((slot, _)) = exit.done.wait_timeout_while(slot, GRACE, |s| s.is_none())
                else {
                    return;
                };
                if slot.is_none() {
                    drop(slot);
                    force_kill(pid, killer.as_mut());
                }
            });
    }

    /// Kills the child and blocks until it is reaped.
    pub(crate) fn kill_and_wait(&mut self) -> Result<ExitStatus, PtyError> {
        if let Some(status) = self.try_wait() {
            return Ok(status);
        }
        let _ = self.killer.kill();
        if let Some(status) = self.wait_timeout(Some(GRACE)) {
            return Ok(status);
        }
        force_kill(self.pid, self.killer.as_mut());
        self.wait_timeout(None)
            .ok_or_else(|| PtyError::Io(std::io::Error::other("child exit status unavailable")))
    }
}

/// `SIGKILL` on Unix (the killer only sends `SIGHUP` there); on Windows the
/// killer already calls `TerminateProcess`.
fn force_kill(pid: Option<u32>, killer: &mut (dyn ChildKiller + Send + Sync)) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        // SAFETY: `kill` with a pid we spawned and SIGKILL has no memory
        // effects; a stale pid just yields ESRCH.
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        return;
    }
    let _ = pid;
    let _ = killer.kill();
}
