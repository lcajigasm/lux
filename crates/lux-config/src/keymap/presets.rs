//! The three shipped keymap presets — Sublime Text, VS Code, and
//! JetBrains — loaded from checked-in `.jsonc` files under `keymaps/` at
//! the crate root (not `src/`, since these are data, not code), parsed
//! with the same JSONC parser `layering` already uses.

use crate::layering::jsonc;

use super::binding::{KeyBinding, Keymap};

const SUBLIME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/keymaps/sublime.jsonc"
));
const VSCODE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/keymaps/vscode.jsonc"));
const JETBRAINS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/keymaps/jetbrains.jsonc"
));

/// Parses a preset's `.jsonc` text into a [`Keymap`]. Panics on a parse or
/// schema mismatch rather than returning `Result`: these are compiled-in
/// constants, never user input, so a failure here means this crate itself
/// shipped broken — a bug to fail loudly on, not a condition a caller could
/// meaningfully recover from.
fn load(name: &str, jsonc_text: &str) -> Keymap {
    let value = jsonc::parse(jsonc_text)
        .unwrap_or_else(|e| panic!("{name} keymap preset failed to parse: {e}"));
    let bindings: Vec<KeyBinding> = serde_json::from_value(value)
        .unwrap_or_else(|e| panic!("{name} keymap preset doesn't match the binding schema: {e}"));
    Keymap {
        name: name.to_string(),
        bindings,
    }
}

pub fn sublime() -> Keymap {
    load("Sublime Text", SUBLIME)
}

pub fn vscode() -> Keymap {
    load("VS Code", VSCODE)
}

pub fn jetbrains() -> Keymap {
    load("JetBrains", JETBRAINS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_three_presets_parse_without_panicking() {
        assert!(!sublime().bindings.is_empty());
        assert!(!vscode().bindings.is_empty());
        assert!(!jetbrains().bindings.is_empty());
    }

    #[test]
    fn preset_names_are_set() {
        assert_eq!(sublime().name, "Sublime Text");
        assert_eq!(vscode().name, "VS Code");
        assert_eq!(jetbrains().name, "JetBrains");
    }
}
