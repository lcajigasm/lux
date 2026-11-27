//! Integrated PTY terminal panel and project tasks state.
//! Desktop only (real PTY processes, portable-pty + alacritty_terminal).

pub(crate) mod dock;
pub(crate) mod input;
#[cfg(test)]
mod tests;

use std::path::Path;

use lux_term::{
    CommandBuilder, Pty, PtyError, PtyNotify, PtySize, ShellProfile, TaskId, TaskPanel, TerminalId,
    Terminals,
};

pub(crate) const DEFAULT_TERM_HEIGHT: f32 = 240.0;

/// State for the bottom dock terminal panel and project tasks runner.
pub(crate) struct TerminalState {
    pub(crate) visible: bool,
    pub(crate) focused: bool,
    pub(crate) height: f32,
    pub(crate) terminals: Terminals,
    pub(crate) session_ids: Vec<TerminalId>,
    pub(crate) session_names: Vec<String>,
    pub(crate) active_tab: usize,
    pub(crate) tasks: TaskPanel,
    pub(crate) task_ids: Vec<TaskId>,
    pub(crate) active_task: Option<TaskId>,
    pub(crate) show_tasks: bool,
    pub(crate) last_size: Option<PtySize>,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalState {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            focused: false,
            height: DEFAULT_TERM_HEIGHT,
            terminals: Terminals::new(),
            session_ids: Vec::new(),
            session_names: Vec::new(),
            active_tab: 0,
            tasks: TaskPanel::new(),
            task_ids: Vec::new(),
            active_task: None,
            show_tasks: false,
            last_size: None,
        }
    }

    pub(crate) fn active_id(&self) -> Option<TerminalId> {
        self.session_ids.get(self.active_tab).copied()
    }

    pub(crate) fn active_pty(&self) -> Option<&Pty> {
        let id = self.active_id()?;
        self.terminals.get(id)
    }

    pub(crate) fn active_pty_mut(&mut self) -> Option<&mut Pty> {
        let id = self.active_id()?;
        self.terminals.get_mut(id)
    }

    /// The task shown in the Tasks view, if it is still in the panel.
    pub(crate) fn active_task_pty(&self) -> Option<&Pty> {
        self.tasks.get(self.active_task?)
    }

    #[cfg(test)]
    pub(crate) fn active_name(&self) -> Option<&str> {
        self.session_names.get(self.active_tab).map(String::as_str)
    }

    fn default_size(&self) -> PtySize {
        self.last_size.unwrap_or(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    /// Spawns the default system shell session if no session exists yet.
    pub(crate) fn ensure_session(&mut self, cwd: Option<&Path>) -> Result<TerminalId, PtyError> {
        if let Some(id) = self.active_id() {
            return Ok(id);
        }
        self.spawn_session(None, None, cwd)
    }

    /// Spawns a new shell session with unique name and given profile.
    pub(crate) fn spawn_session(
        &mut self,
        name: Option<String>,
        profile: Option<&ShellProfile>,
        cwd: Option<&Path>,
    ) -> Result<TerminalId, PtyError> {
        let mut default_profile = ShellProfile::system_default();
        if let Some(dir) = cwd {
            default_profile.cwd = Some(dir.to_path_buf());
        }
        let prof = profile.unwrap_or(&default_profile);

        let count = self.session_ids.len() + 1;
        let session_name = name.unwrap_or_else(|| format!("Terminal {count}"));
        let size = self.default_size();

        let id = self.terminals.spawn(&session_name, prof, size)?;
        self.session_ids.push(id);
        self.session_names.push(session_name);
        self.active_tab = self.session_ids.len() - 1;
        self.show_tasks = false;
        Ok(id)
    }

    /// Closes the currently active session tab, killing its child process cleanly.
    pub(crate) fn close_active_session(&mut self) -> bool {
        if self.session_ids.is_empty() {
            return false;
        }
        self.close_session(self.active_tab)
    }

    /// Closes the session tab at `index`, killing its child process cleanly.
    pub(crate) fn close_session(&mut self, idx: usize) -> bool {
        if idx >= self.session_ids.len() {
            return false;
        }
        let id = self.session_ids.remove(idx);
        self.session_names.remove(idx);
        let _ = self.terminals.kill(id);

        if self.session_ids.is_empty() {
            self.active_tab = 0;
            self.visible = false;
            self.focused = false;
        } else if self.active_tab >= self.session_ids.len() {
            self.active_tab = self.session_ids.len() - 1;
        }
        true
    }

    /// Resizes the terminal the dock shows (the active task in the Tasks
    /// view, else the active shell session) and remembers the size for
    /// new ones. Called every frame, so an unchanged size costs nothing,
    /// and hidden terminals are resized when they are shown.
    pub(crate) fn resize(&mut self, rows: u16, cols: u16) {
        if rows == 0 || cols == 0 {
            return;
        }
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        self.last_size = Some(size);
        let shown = if self.show_tasks {
            self.active_task_pty()
        } else {
            self.active_pty()
        };
        if let Some(pty) = shown
            && pty.size() != (rows as usize, cols as usize)
        {
            let _ = pty.resize(size);
        }
    }

    /// Starts a project task command in the TaskPanel.
    pub(crate) fn run_task(
        &mut self,
        name: &str,
        command: CommandBuilder,
    ) -> Result<TaskId, PtyError> {
        self.run_task_with(name, command, |_| PtyNotify::default())
    }

    /// `run_task`, reporting the task's output and exit through `notify`.
    pub(crate) fn run_task_with(
        &mut self,
        name: &str,
        command: CommandBuilder,
        notify: impl FnOnce(TaskId) -> PtyNotify,
    ) -> Result<TaskId, PtyError> {
        let size = self.default_size();
        let id = self.tasks.start_with(name, command, size, notify)?;
        self.task_ids.push(id);
        self.active_task = Some(id);
        self.show_tasks = true;
        self.visible = true;
        self.focused = true;
        Ok(id)
    }

    /// Cancels a task: drops it from the panel at once and stops and
    /// reaps its child on a worker, so the UI thread never waits.
    pub(crate) fn cancel_task(&mut self, id: TaskId) -> bool {
        let Some(pty) = self.tasks.remove(id) else {
            return false;
        };
        reap_detached(pty);
        self.task_ids.retain(|&t| t != id);
        if self.active_task == Some(id) {
            self.active_task = self.task_ids.last().copied();
        }
        true
    }

    /// Cancels the currently active task.
    pub(crate) fn cancel_active_task(&mut self) -> bool {
        if let Some(id) = self.active_task {
            self.cancel_task(id)
        } else {
            false
        }
    }

    /// Shuts down all terminals and tasks: every child is stopped and
    /// reaped on its own worker, all at once, and quitting waits for them
    /// at most `SHUTDOWN_WAIT` in total, never unbounded.
    pub(crate) fn shutdown_all(&mut self) {
        let mut ptys = Vec::new();
        for id in self.session_ids.drain(..) {
            ptys.extend(self.terminals.remove(id));
        }
        self.session_names.clear();
        for id in self.task_ids.drain(..) {
            ptys.extend(self.tasks.remove(id));
        }
        self.active_task = None;
        let (done, reaped) = std::sync::mpsc::channel();
        let mut count = 0;
        for pty in ptys {
            let done = done.clone();
            let spawned = std::thread::Builder::new()
                .name("lux-pty-reap".into())
                .spawn(move || {
                    let _ = pty.kill_and_wait();
                    let _ = done.send(());
                });
            // A reaper that could not start dropped its PTY: nothing to wait for.
            count += usize::from(spawned.is_ok());
        }
        let deadline = std::time::Instant::now() + SHUTDOWN_WAIT;
        for _ in 0..count {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if reaped.recv_timeout(left).is_err() {
                break;
            }
        }
    }
}

/// The longest quitting waits for terminal and task children to be reaped.
const SHUTDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

/// Stops and reaps `pty`'s child on a worker thread.
fn reap_detached(pty: Pty) {
    let spawned = std::thread::Builder::new()
        .name("lux-pty-reap".into())
        .spawn(move || {
            let _ = pty.kill_and_wait();
        });
    if let Err(error) = spawned {
        eprintln!("lux: cannot reap a task: {error}");
    }
}
