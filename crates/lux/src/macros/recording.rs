//! Macro recording state and persistence directory discovery (ST19).

use std::path::{Path, PathBuf};

use super::command::MacroCommand;
use super::macro_def::Macro;

/// Macro engine state held on `UiOverlays`.
#[derive(Debug)]
pub struct MacroState {
    /// Whether macro recording is currently active.
    pub is_recording: bool,
    /// In-progress recorded commands during an active recording session.
    pub current_recording: Vec<MacroCommand>,
    /// The most recently recorded macro (or loaded macro) available for playback.
    pub last_macro: Option<Macro>,
    /// Named saved macros discovered from disk.
    pub saved_macros: Vec<Macro>,
}

/// Starts empty. Saved macros are read from disk on a worker
/// (`library::start_loading`) and handed over with `set_saved`.
impl Default for MacroState {
    fn default() -> Self {
        Self {
            is_recording: false,
            current_recording: Vec::new(),
            last_macro: None,
            saved_macros: Vec::new(),
        }
    }
}

impl MacroState {
    /// Creates an empty initial macro state.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new macro recording session.
    pub fn start_recording(&mut self) {
        self.is_recording = true;
        self.current_recording.clear();
    }

    /// Stops the active recording session and returns the recorded macro.
    pub fn stop_recording(&mut self) -> Option<Macro> {
        self.is_recording = false;
        if self.current_recording.is_empty() {
            return None;
        }
        let commands = std::mem::take(&mut self.current_recording);
        let m = Macro::from_commands(commands);
        self.last_macro = Some(m.clone());
        Some(m)
    }

    /// Toggles the recording state. Returns `true` if recording was started, `false` if stopped.
    pub fn toggle_recording(&mut self) -> bool {
        if self.is_recording {
            self.stop_recording();
            false
        } else {
            self.start_recording();
            true
        }
    }

    /// Records a command if currently in recording mode.
    pub fn record_command(&mut self, cmd: MacroCommand) {
        if !self.is_recording {
            return;
        }

        // Optimize consecutive single-character insertions if applicable
        if let MacroCommand::InsertText { ref text } = cmd
            && let Some(MacroCommand::InsertText { text: prev_text }) =
                self.current_recording.last_mut()
            && !prev_text.ends_with('\n')
            && !text.starts_with('\n')
        {
            prev_text.push_str(text);
            return;
        }

        self.current_recording.push(cmd);
    }

    /// Picks the macro to save and its target path. No disk access: the
    /// caller writes `Macro::save_to_file` on a worker, then calls
    /// `note_saved`.
    pub fn prepare_save(
        &self,
        name: &str,
        custom_dir: Option<&Path>,
    ) -> std::io::Result<(PathBuf, Macro)> {
        let mut macro_to_save = match &self.last_macro {
            Some(m) => m.clone(),
            None => {
                if !self.current_recording.is_empty() {
                    Macro::from_commands(self.current_recording.clone())
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "No macro has been recorded to save",
                    ));
                }
            }
        };

        let target_dir = if let Some(dir) = custom_dir {
            dir.to_path_buf()
        } else {
            default_macros_dir()
        };

        let sanitized_name = super::library::valid_name(name)?;
        let filename = format!("{sanitized_name}.sublime-macro");
        macro_to_save.name = Some(sanitized_name.to_string());
        Ok((target_dir.join(filename), macro_to_save))
    }

    /// Lists a macro that was written to disk.
    pub fn note_saved(&mut self, saved: Macro) {
        self.saved_macros.retain(|m| m.name != saved.name);
        self.saved_macros.push(saved);
        self.saved_macros.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Replaces the saved-macro list with what a worker read from disk.
    pub fn set_saved(&mut self, macros: Vec<Macro>) {
        self.saved_macros = macros;
    }

    /// The saved macro called `name`, if one is loaded.
    pub fn saved(&self, name: &str) -> Option<&Macro> {
        self.saved_macros
            .iter()
            .find(|m| m.name.as_deref() == Some(name))
    }

    /// Saves the current or last recorded macro synchronously.
    #[cfg(test)]
    pub fn save_macro_as(
        &mut self,
        name: &str,
        custom_dir: Option<&Path>,
    ) -> std::io::Result<PathBuf> {
        let (path, saved) = self.prepare_save(name, custom_dir)?;
        saved.save_to_file(&path)?;
        self.note_saved(saved);
        Ok(path)
    }
}

/// Returns the primary default directory where saved macros are placed (`~/.lux/macros/`).
pub(super) fn default_macros_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(dir) = test_dir() {
        return dir;
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        home.join(".lux").join("macros")
    } else {
        PathBuf::from(".lux").join("macros")
    }
}

#[cfg(test)]
thread_local! {
    static TEST_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Points this test thread's macro folder at `dir`, so tests never touch
/// the real `~/.lux/macros`. Paths are picked on the calling thread.
#[cfg(test)]
pub(crate) fn set_test_dir(dir: Option<PathBuf>) {
    TEST_DIR.with(|cell| *cell.borrow_mut() = dir);
}

#[cfg(test)]
pub(super) fn test_dir() -> Option<PathBuf> {
    TEST_DIR.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_recording_lifecycle() {
        let mut state = MacroState::new();
        assert!(!state.is_recording);

        assert!(state.toggle_recording());
        assert!(state.is_recording);

        state.record_command(MacroCommand::InsertText {
            text: "fn main() ".to_string(),
        });
        state.record_command(MacroCommand::InsertText {
            text: "{}".to_string(),
        });

        assert!(!state.toggle_recording());
        assert!(!state.is_recording);

        let last = state.last_macro.as_ref().expect("macro recorded");
        assert_eq!(last.commands.len(), 1);
        assert_eq!(
            last.commands[0],
            MacroCommand::InsertText {
                text: "fn main() {}".to_string(),
            }
        );
    }
}
