//! Project tasks: a named command running on a real PTY, with cancellation
//! that kills and reaps the child (no leftover processes).

use std::collections::BTreeMap;

use crate::pty::{CommandBuilder, ExitStatus, Pty, PtyError, PtyNotify, PtySize};

/// Handle for one running (or just-finished) task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(u64);

impl TaskId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Several concurrent tasks. Cancellation is `Pty::kill_and_wait`.
pub struct TaskPanel {
    next_id: u64,
    tasks: BTreeMap<TaskId, NamedTask>,
}

struct NamedTask {
    name: String,
    pty: Pty,
}

impl Default for TaskPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskPanel {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            tasks: BTreeMap::new(),
        }
    }

    pub fn start(
        &mut self,
        name: impl Into<String>,
        command: CommandBuilder,
        size: PtySize,
    ) -> Result<TaskId, PtyError> {
        self.start_with(name, command, size, |_| PtyNotify::default())
    }

    /// `start`, with callbacks for output and exit. `notify` receives the
    /// new task's id so the callbacks can name it.
    pub fn start_with(
        &mut self,
        name: impl Into<String>,
        command: CommandBuilder,
        size: PtySize,
        notify: impl FnOnce(TaskId) -> PtyNotify,
    ) -> Result<TaskId, PtyError> {
        let id = TaskId(self.next_id);
        let pty = Pty::spawn_with(command, size, notify(id))?;
        self.next_id += 1;
        self.tasks.insert(
            id,
            NamedTask {
                name: name.into(),
                pty,
            },
        );
        Ok(id)
    }

    pub fn get(&self, id: TaskId) -> Option<&Pty> {
        self.tasks.get(&id).map(|t| &t.pty)
    }

    pub fn get_mut(&mut self, id: TaskId) -> Option<&mut Pty> {
        self.tasks.get_mut(&id).map(|t| &mut t.pty)
    }

    pub fn name(&self, id: TaskId) -> Option<&str> {
        self.tasks.get(&id).map(|t| t.name.as_str())
    }

    /// Every task's terminal, for resizing them together.
    pub fn ptys(&self) -> impl Iterator<Item = &Pty> {
        self.tasks.values().map(|t| &t.pty)
    }

    /// Kill the child and drop it from the panel. Real cancellation.
    pub fn cancel(&mut self, id: TaskId) -> Result<ExitStatus, PtyError> {
        let task = self.tasks.remove(&id).ok_or_else(|| {
            PtyError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such task",
            ))
        })?;
        task.pty.kill_and_wait()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn small() -> PtySize {
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    #[test]
    fn cancel_kills_the_child_and_leaves_no_orphan() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "sleep 30"]);
        let mut panel = TaskPanel::new();
        let id = panel.start("sleep", cmd, small()).expect("start");
        assert_eq!(panel.name(id), Some("sleep"));
        let pid = panel.get(id).and_then(Pty::pid).expect("pid");

        let status = panel.cancel(id).expect("cancel");
        assert!(!status.success());
        assert!(panel.get(id).is_none());

        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        assert_eq!(rc, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[test]
    fn cancelling_one_task_does_not_cancel_another() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "sleep 30"]);
        let mut panel = TaskPanel::new();
        let mut cmd_b = CommandBuilder::new("/bin/sh");
        cmd_b.args(["-c", "sleep 30"]);
        let a = panel.start("a", cmd, small()).expect("a");
        let b = panel.start("b", cmd_b, small()).expect("b");
        let pid_b = panel.get(b).and_then(Pty::pid).expect("b pid");
        panel.cancel(a).expect("cancel a");
        let rc = unsafe { libc::kill(pid_b as libc::pid_t, 0) };
        assert_eq!(rc, 0);
        panel.cancel(b).expect("cancel b");
    }

    fn notified(
        panel: &mut TaskPanel,
        script: &str,
    ) -> (TaskId, std::sync::mpsc::Receiver<(TaskId, u32)>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", script]);
        let id = panel
            .start_with("t", cmd, small(), move |id| PtyNotify {
                output: None,
                exit: Some(Box::new(move |status: &ExitStatus| {
                    let _ = tx.send((id, status.exit_code()));
                })),
            })
            .expect("start");
        (id, rx)
    }

    #[test]
    fn the_exit_notice_carries_the_code_after_the_output_is_in_the_grid() {
        let mut panel = TaskPanel::new();
        let (id, rx) = notified(&mut panel, "printf 'built ok\\n'; exit 3");
        let (noticed, code) = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("exit notice");
        assert_eq!((noticed, code), (id, 3));
        let pty = panel.get_mut(id).unwrap();
        assert!(pty.snapshot().contains("built ok"), "{}", pty.snapshot());
        assert_eq!(pty.try_wait().unwrap().map(|s| s.exit_code()), Some(3));

        pty.append_line("[Finished in 0.1s with exit code 3]");
        let lines: Vec<String> = pty.snapshot().lines().map(str::to_string).collect();
        let built = lines.iter().position(|l| l == "built ok").unwrap();
        assert_eq!(lines[built + 1], "[Finished in 0.1s with exit code 3]");
    }

    #[test]
    fn request_kill_returns_at_once_and_keeps_the_task() {
        let mut panel = TaskPanel::new();
        // Ignores the hangup, so only the forced kill ends it.
        let (id, rx) = notified(&mut panel, "trap '' HUP; echo started; sleep 30");
        let pid = panel.get(id).and_then(Pty::pid).expect("pid");
        let asked = std::time::Instant::now();
        panel.get_mut(id).unwrap().request_kill();
        assert!(asked.elapsed() < std::time::Duration::from_millis(100));
        let (noticed, _) = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("exit notice after the forced kill");
        assert_eq!(noticed, id);
        assert!(panel.get(id).is_some(), "output kept");
        let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
        assert_eq!(rc, -1, "child still alive");
    }
}
