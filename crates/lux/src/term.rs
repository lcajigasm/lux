//! Integrated PTY terminal panel and project tasks state.
//! Desktop only (real PTY processes, portable-pty + alacritty_terminal).

pub(crate) mod input;
#[cfg(test)]
mod tests;

use std::path::Path;

use lux_term::{
    CommandBuilder, Pty, PtyError, PtySize, ShellProfile, TaskId, TaskPanel, TerminalId, Terminals,
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

    /// Resizes the active PTY (and remembers the dimensions for future sessions).
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
        for id in &self.session_ids {
            if let Some(pty) = self.terminals.get(*id) {
                let _ = pty.resize(size);
            }
        }
    }

    /// Starts a project task command in the TaskPanel.
    pub(crate) fn run_task(
        &mut self,
        name: &str,
        command: CommandBuilder,
    ) -> Result<TaskId, PtyError> {
        let size = self.default_size();
        let id = self.tasks.start(name, command, size)?;
        self.task_ids.push(id);
        self.active_task = Some(id);
        self.show_tasks = true;
        self.visible = true;
        self.focused = true;
        Ok(id)
    }

    /// Cancels a running task and reaps its child.
    pub(crate) fn cancel_task(&mut self, id: TaskId) -> bool {
        if let Ok(_status) = self.tasks.cancel(id) {
            self.task_ids.retain(|&t| t != id);
            if self.active_task == Some(id) {
                self.active_task = self.task_ids.last().copied();
            }
            true
        } else {
            false
        }
    }

    /// Cancels the currently active task.
    pub(crate) fn cancel_active_task(&mut self) -> bool {
        if let Some(id) = self.active_task {
            self.cancel_task(id)
        } else {
            false
        }
    }

    /// Shuts down all terminals and tasks cleanly, killing and reaping all child processes.
    pub(crate) fn shutdown_all(&mut self) {
        for id in self.session_ids.drain(..) {
            let _ = self.terminals.kill(id);
        }
        self.session_names.clear();
        for id in self.task_ids.drain(..) {
            let _ = self.tasks.cancel(id);
        }
        self.active_task = None;
    }
}
