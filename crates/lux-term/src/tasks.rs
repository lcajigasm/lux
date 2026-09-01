//! Project tasks: a named command running on a real PTY, with cancellation
//! that kills and reaps the child (no leftover processes).

use std::collections::BTreeMap;

use crate::pty::{CommandBuilder, ExitStatus, Pty, PtyError, PtySize};

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
        let pty = Pty::spawn(command, size)?;
        let id = TaskId(self.next_id);
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

    pub fn name(&self, id: TaskId) -> Option<&str> {
        self.tasks.get(&id).map(|t| t.name.as_str())
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
}
