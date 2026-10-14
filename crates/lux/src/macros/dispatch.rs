//! Macro event recording hooks and command dispatch helpers (ST19).

use lux_platform::NamedKey;

use super::command::{MacroCommand, MoveBy, MoveTo};
use crate::app::App;

/// Toggles macro recording on the active App instance.
pub fn toggle_record_macro(app: &mut App) {
    let is_recording = app.ui.macros.toggle_recording();
    if is_recording {
        app.ui.announcement = Some("Macro recording started".to_string());
        app.documents.message = Some("● Recording Macro".to_string());
    } else {
        let count = app
            .ui
            .macros
            .last_macro
            .as_ref()
            .map(|m| m.commands.len())
            .unwrap_or(0);
        app.ui.announcement = Some(format!("Macro recording stopped ({count} steps)"));
        app.documents.message = Some(format!("Recorded macro with {count} commands"));
    }
    app.scheduler.invalidate_all();
}

/// Plays back the most recently recorded macro on the active App instance.
pub fn playback_macro(app: &mut App) {
    let success = super::playback::playback_last_macro(app);
    if !success {
        app.documents.message = Some("No macro recorded to playback".to_string());
    }
}

/// Saves the last recorded macro as a `.sublime-macro` file under `~/.lux/macros/`.
pub fn save_macro_as(app: &mut App, name: &str) {
    match app.ui.macros.save_macro_as(name, None) {
        Ok(path) => {
            let filename = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("macro.sublime-macro");
            app.documents.message = Some(format!("Saved macro to {filename}"));
            app.ui.announcement = Some(format!("Macro saved as {filename}"));
        }
        Err(e) => {
            app.documents.message = Some(format!("Failed to save macro: {e}"));
        }
    }
}

/// Records text input if macro recording is currently active.
pub fn record_text_input(app: &mut App, text: &str) {
    if app.ui.macros.is_recording {
        app.ui.macros.record_command(MacroCommand::InsertText {
            text: text.to_string(),
        });
    }
}

/// Records a newline insertion if macro recording is active.
pub fn record_enter(app: &mut App) {
    if app.ui.macros.is_recording {
        app.ui.macros.record_command(MacroCommand::InsertNewline);
    }
}

/// Records a backward delete (backspace) if macro recording is active.
pub fn record_backspace(app: &mut App) {
    if app.ui.macros.is_recording {
        app.ui.macros.record_command(MacroCommand::DeleteBackward);
    }
}

/// Records a forward delete if macro recording is active.
pub fn record_delete(app: &mut App) {
    if app.ui.macros.is_recording {
        app.ui.macros.record_command(MacroCommand::DeleteForward);
    }
}

/// Records a word deletion if macro recording is active.
pub fn record_word_delete(app: &mut App, forward: bool) {
    if app.ui.macros.is_recording {
        app.ui
            .macros
            .record_command(MacroCommand::DeleteWord { forward });
    }
}

/// Records cursor movement if macro recording is active.
pub fn record_move(app: &mut App, key: NamedKey, word: bool, command: bool, extend: bool) {
    if !app.ui.macros.is_recording {
        return;
    }

    use NamedKey::*;
    let cmd = match key {
        Home if command => MacroCommand::MoveTo {
            to: MoveTo::Bof,
            extend,
        },
        End if command => MacroCommand::MoveTo {
            to: MoveTo::Eof,
            extend,
        },
        ArrowLeft if command && cfg!(target_os = "macos") => MacroCommand::MoveTo {
            to: MoveTo::Bol,
            extend,
        },
        ArrowRight if command && cfg!(target_os = "macos") => MacroCommand::MoveTo {
            to: MoveTo::Eol,
            extend,
        },
        ArrowUp if command => MacroCommand::MoveTo {
            to: MoveTo::Bof,
            extend,
        },
        ArrowDown if command => MacroCommand::MoveTo {
            to: MoveTo::Eof,
            extend,
        },
        ArrowLeft if word => MacroCommand::Move {
            by: MoveBy::Words,
            forward: false,
            extend,
        },
        ArrowRight if word => MacroCommand::Move {
            by: MoveBy::Words,
            forward: true,
            extend,
        },
        ArrowLeft => MacroCommand::Move {
            by: MoveBy::Characters,
            forward: false,
            extend,
        },
        ArrowRight => MacroCommand::Move {
            by: MoveBy::Characters,
            forward: true,
            extend,
        },
        ArrowUp => MacroCommand::Move {
            by: MoveBy::Lines,
            forward: false,
            extend,
        },
        ArrowDown => MacroCommand::Move {
            by: MoveBy::Lines,
            forward: true,
            extend,
        },
        Home => MacroCommand::MoveTo {
            to: MoveTo::Bol,
            extend,
        },
        End => MacroCommand::MoveTo {
            to: MoveTo::Eol,
            extend,
        },
        _ => return,
    };

    app.ui.macros.record_command(cmd);
}

/// Records an editor command if macro recording is active and the command is recordable.
pub fn record_command_if_recording(app: &mut App, cmd: &str) {
    if !app.ui.macros.is_recording {
        return;
    }

    // Do not record macro meta commands
    if matches!(
        cmd,
        "toggle_record_macro"
            | "playback_macro"
            | "save_macro"
            | "save_macro_as"
            | "command_palette"
            | "quick_open"
    ) {
        return;
    }

    app.ui.macros.record_command(MacroCommand::Action {
        name: cmd.to_string(),
        args: None,
    });
}
