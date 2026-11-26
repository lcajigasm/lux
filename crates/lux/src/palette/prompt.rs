//! A palette that asks for a line of text instead of listing commands.
//!
//! It is the command palette with no rows: the same card, query field and
//! keys. Enter hands the typed text to the prompt's action; Escape closes it.

use crate::app::App;

/// What the typed text is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PalettePrompt {
    /// Tools: Save Macro… — the name of the `.sublime-macro` file.
    SaveMacro,
    /// Save Macro found `name` already saved. The query starts as `name`:
    /// Enter on it replaces the file, an edited name saves a new one.
    ReplaceMacro {
        name: String,
        dir: Option<std::path::PathBuf>,
    },
}

impl PalettePrompt {
    pub(crate) fn placeholder(&self) -> &'static str {
        match self {
            Self::SaveMacro | Self::ReplaceMacro { .. } => "Macro name, then Enter to save…",
        }
    }

    /// Runs the action for `text`. Returns `false` to keep the prompt open
    /// (the text was refused and the reason is in the status message).
    pub(crate) fn submit(&self, app: &mut App, text: &str) -> bool {
        let name = match crate::macros::library::valid_name(text) {
            Ok(name) => name.to_string(),
            Err(error) => {
                app.documents.message = Some(format!("Cannot save macro: {error}"));
                return false;
            }
        };
        match self {
            Self::SaveMacro => crate::macros::dispatch::save_macro_as(app, &name),
            Self::ReplaceMacro {
                name: existing,
                dir,
            } => {
                let replace = name == *existing;
                crate::macros::dispatch::save_macro_to(app, &name, dir.as_deref(), replace);
            }
        }
        true
    }
}

impl App {
    /// Opens the command palette as a text prompt holding `text`.
    pub(crate) fn open_palette_prompt(&mut self, prompt: PalettePrompt, text: &str) {
        self.open_command_palette(text);
        if let Some(palette) = self.ui.command_palette.as_mut() {
            palette.prompt = Some(prompt);
        }
    }
}
