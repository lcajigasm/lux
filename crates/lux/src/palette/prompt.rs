//! A palette that asks for a line of text instead of listing commands.
//!
//! It is the command palette with no rows: the same card, query field and
//! keys. Enter hands the typed text to the prompt's action; Escape closes it.

use crate::app::App;

/// What the typed text is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PalettePrompt {
    /// Tools: Save Macro… — the name of the `.sublime-macro` file.
    SaveMacro,
}

impl PalettePrompt {
    pub(crate) fn placeholder(self) -> &'static str {
        match self {
            Self::SaveMacro => "Macro name, then Enter to save…",
        }
    }

    /// Runs the action for `text`. Returns `false` to keep the prompt open
    /// (the text was refused and the reason is in the status message).
    pub(crate) fn submit(self, app: &mut App, text: &str) -> bool {
        match self {
            Self::SaveMacro => match crate::macros::library::valid_name(text) {
                Ok(name) => {
                    let name = name.to_string();
                    crate::macros::dispatch::save_macro_as(app, &name);
                    true
                }
                Err(error) => {
                    app.documents.message = Some(format!("Cannot save macro: {error}"));
                    false
                }
            },
        }
    }
}

impl App {
    /// Opens the command palette as a text prompt.
    pub(crate) fn open_palette_prompt(&mut self, prompt: PalettePrompt) {
        self.open_command_palette("");
        if let Some(palette) = self.ui.command_palette.as_mut() {
            palette.prompt = Some(prompt);
        }
    }
}
