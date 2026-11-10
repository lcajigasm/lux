//! The settings page rows. Values come from the resolved user config.

use serde_json::Value;

use super::panel::{SettingItem, SettingKind};

pub(crate) fn page_items(resolved: &Value) -> Vec<SettingItem> {
    let font_size = resolved["editor.fontSize"].as_f64().unwrap_or(13.0);
    let line_height = resolved["editor.lineHeight"].as_f64().unwrap_or(1.4);
    let font_ligatures = resolved["editor.fontLigatures"].as_bool().unwrap_or(true);
    let tab_width = resolved["editor.tabWidth"].as_f64().unwrap_or(4.0);
    let indent_style = resolved["editor.indentStyle"]
        .as_str()
        .unwrap_or("space")
        .to_string();
    let trim_ws = resolved["editor.trimTrailingWhitespace"]
        .as_bool()
        .unwrap_or(false);
    let preserve_cursor = resolved["editor.trimTrailingWhitespacePreserveCursor"]
        .as_bool()
        .unwrap_or(false);
    let final_nl = resolved["editor.insertFinalNewline"]
        .as_bool()
        .unwrap_or(false);
    let word_wrap = resolved["editor.wordWrap"].as_bool().unwrap_or(false);
    let df_col = resolved["editor.distractionFreeColumnWidth"]
        .as_f64()
        .unwrap_or(80.0);
    let theme = resolved["workbench.colorTheme"]
        .as_str()
        .unwrap_or("monokai")
        .to_string();
    let keymap = resolved["workbench.keymap"]
        .as_str()
        .unwrap_or("sublime")
        .to_string();

    vec![
        number(
            "Text",
            "editor.fontSize",
            "Font Size",
            "Typeface size in points (10 to 24)",
            font_size,
            &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 24.0],
        ),
        number(
            "Text",
            "editor.lineHeight",
            "Line Height",
            "Multiplier of font size (1.2 to 2.0)",
            line_height,
            &[1.2, 1.3, 1.4, 1.5, 1.6, 1.8, 2.0],
        ),
        boolean(
            "Text",
            "editor.fontLigatures",
            "Font Ligatures",
            "Enable or disable programming ligatures",
            font_ligatures,
        ),
        number(
            "Editing",
            "editor.tabWidth",
            "Tab Size",
            "Number of columns a tab character occupies",
            tab_width,
            &[2.0, 4.0, 8.0],
        ),
        enumeration(
            "Editing",
            "editor.indentStyle",
            "Indent Style",
            "Indentation character (space or tab)",
            indent_style,
            &["space", "tab"],
        ),
        boolean(
            "Editing",
            "editor.trimTrailingWhitespace",
            "Trim Trailing Whitespace",
            "Remove whitespace at end of lines on save",
            trim_ws,
        ),
        boolean(
            "Editing",
            "editor.trimTrailingWhitespacePreserveCursor",
            "Preserve Whitespace On Caret",
            "Keep trailing whitespace on lines with active cursors",
            preserve_cursor,
        ),
        boolean(
            "Editing",
            "editor.insertFinalNewline",
            "Insert Final Newline",
            "Ensure file ends with a trailing newline",
            final_nl,
        ),
        boolean(
            "Editing",
            "editor.wordWrap",
            "Word Wrap",
            "Wrap lines exceeding the viewport",
            word_wrap,
        ),
        number(
            "Editing",
            "editor.distractionFreeColumnWidth",
            "Distraction Free Column Width",
            "Character width of centered text in distraction free mode",
            df_col,
            &[72.0, 80.0, 90.0, 100.0, 120.0],
        ),
        enumeration(
            "Workbench",
            "workbench.colorTheme",
            "Color Theme",
            "Visual color palette. \"system\" follows the OS appearance",
            theme,
            crate::color_scheme::theme_setting_options(),
        ),
        enumeration(
            "Workbench",
            "workbench.keymap",
            "Keymap Preset",
            "Keyboard shortcut mapping profile",
            keymap,
            &["sublime", "vscode", "jetbrains"],
        ),
    ]
}

fn number(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    description: &'static str,
    value: f64,
    options: &'static [f64],
) -> SettingItem {
    SettingItem {
        category,
        key,
        label,
        description,
        kind: SettingKind::Number { value, options },
    }
}

fn boolean(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    description: &'static str,
    value: bool,
) -> SettingItem {
    SettingItem {
        category,
        key,
        label,
        description,
        kind: SettingKind::Boolean { value },
    }
}

fn enumeration(
    category: &'static str,
    key: &'static str,
    label: &'static str,
    description: &'static str,
    value: String,
    options: &'static [&'static str],
) -> SettingItem {
    SettingItem {
        category,
        key,
        label,
        description,
        kind: SettingKind::Enum { value, options },
    }
}
