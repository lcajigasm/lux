//! Imports a VS Code `settings.json` file — JSONC (comments and trailing
//! commas), parsed with the same JSONC parser `layering` uses. VS Code's
//! own settings are already flat, dotted keys (`"editor.tabSize"`, not a
//! nested `{"editor": {"tabSize": ...}}` object), so no separate
//! flattening step is needed before mapping.
//!
//! Source keys `editor.tabSize`, `editor.insertSpaces`, and
//! `editor.wordWrap` are confirmed directly against
//! `code.visualstudio.com`'s own documentation.
//! `files.trimTrailingWhitespace`, `files.insertFinalNewline`,
//! `editor.fontFamily`, `editor.fontSize`, and `editor.lineHeight` weren't
//! named on the specific pages reachable from here — they come from
//! well-established, long-stable prior knowledge of VS Code's defaults
//! instead (these are default-`settings.json`-template keys, not obscure
//! ones), same transparency as the two importers alongside this one. See
//! `docs/receipts/phase-05-importadores-ajustes-externos.md`.

use serde_json::{Value, json};

use super::{ImportReport, Mapped, report_from_object};
use crate::layering::JsoncError;

pub fn import(content: &str) -> Result<ImportReport, JsoncError> {
    let value = crate::layering::jsonc::parse(content)?;
    Ok(report_from_object(value, map_key))
}

fn map_key(key: &str, value: &Value) -> Mapped {
    match key {
        "editor.tabSize" => value.as_i64().map_or(Mapped::Unsupported("integer"), |n| {
            Mapped::Imported("editor.tabWidth", json!(n))
        }),
        "editor.insertSpaces" => value.as_bool().map_or(Mapped::Unsupported("boolean"), |b| {
            Mapped::Imported("editor.indentStyle", json!(if b { "space" } else { "tab" }))
        }),
        "files.trimTrailingWhitespace" => {
            value.as_bool().map_or(Mapped::Unsupported("boolean"), |b| {
                Mapped::Imported("editor.trimTrailingWhitespace", json!(b))
            })
        }
        "files.insertFinalNewline" => value.as_bool().map_or(Mapped::Unsupported("boolean"), |b| {
            Mapped::Imported("editor.insertFinalNewline", json!(b))
        }),
        // Lux's own schema only has a boolean word-wrap toggle so far —
        // "wordWrapColumn"/"bounded" are real VS Code values this crate
        // genuinely has no equivalent for yet, reported as unsupported
        // rather than silently coerced to a guess.
        "editor.wordWrap" => match value.as_str() {
            Some("on") => Mapped::Imported("editor.wordWrap", json!(true)),
            Some("off") => Mapped::Imported("editor.wordWrap", json!(false)),
            _ => Mapped::Unsupported("\"on\" or \"off\""),
        },
        "editor.fontFamily" => value.as_str().map_or(Mapped::Unsupported("string"), |s| {
            Mapped::Imported("editor.fontFamily", json!(s))
        }),
        // `f64`, not an integer: both are routinely fractional in real VS
        // Code configs (`editor.lineHeight` especially, as a multiplier
        // like `1.5`) and Lux's own schema stores them as `f64` — see
        // `font_settings::FontSettings`.
        "editor.fontSize" => value.as_f64().map_or(Mapped::Unsupported("number"), |n| {
            Mapped::Imported("editor.fontSize", json!(n))
        }),
        "editor.lineHeight" => value.as_f64().map_or(Mapped::Unsupported("number"), |n| {
            Mapped::Imported("editor.lineHeight", json!(n))
        }),
        _ => Mapped::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_known_keys_with_the_right_lux_key_and_value() {
        let report = import(
            r#"{
                "editor.tabSize": 2,
                "editor.insertSpaces": true,
                "editor.fontSize": 14,
            }"#,
        )
        .unwrap();
        assert_eq!(report.imported.len(), 3);
        assert!(
            report
                .imported
                .iter()
                .any(|s| s.source_key == "editor.tabSize"
                    && s.lux_key == "editor.tabWidth"
                    && s.value == json!(2))
        );
        assert!(
            report
                .imported
                .iter()
                .any(|s| s.source_key == "editor.insertSpaces" && s.value == json!("space"))
        );
    }

    #[test]
    fn insert_spaces_false_maps_to_tab_indent_style() {
        let report = import(r#"{"editor.insertSpaces": false}"#).unwrap();
        assert_eq!(report.imported[0].value, json!("tab"));
    }

    #[test]
    fn word_wrap_on_and_off_map_to_booleans() {
        let on = import(r#"{"editor.wordWrap": "on"}"#).unwrap();
        assert_eq!(on.imported[0].value, json!(true));
        let off = import(r#"{"editor.wordWrap": "off"}"#).unwrap();
        assert_eq!(off.imported[0].value, json!(false));
    }

    #[test]
    fn fractional_line_height_and_font_size_are_imported_not_dropped() {
        let report = import(r#"{"editor.lineHeight": 1.5, "editor.fontSize": 13.5}"#).unwrap();
        assert_eq!(report.imported.len(), 2);
        assert!(
            report
                .imported
                .iter()
                .any(|s| s.source_key == "editor.lineHeight" && s.value == json!(1.5))
        );
        assert!(
            report
                .imported
                .iter()
                .any(|s| s.source_key == "editor.fontSize" && s.value == json!(13.5))
        );
    }

    #[test]
    fn word_wrap_column_is_unsupported_not_silently_coerced() {
        let report = import(r#"{"editor.wordWrap": "wordWrapColumn"}"#).unwrap();
        assert_eq!(report.imported.len(), 0);
        assert_eq!(
            report.ignored[0].reason,
            super::super::IgnoreReason::UnsupportedValue {
                expected: "\"on\" or \"off\""
            }
        );
    }

    #[test]
    fn an_unrecognized_key_is_reported_as_unknown() {
        let report = import(r#"{"workbench.colorTheme": "Dark+"}"#).unwrap();
        assert_eq!(report.imported.len(), 0);
        assert_eq!(report.ignored[0].source_key, "workbench.colorTheme");
        assert_eq!(
            report.ignored[0].reason,
            super::super::IgnoreReason::UnknownKey
        );
    }

    #[test]
    fn trailing_commas_and_comments_parse_like_a_real_vscode_settings_file() {
        let report = import(
            r#"{
                // trailing comma and a comment, both real settings.json features
                "editor.tabSize": 4,
            }"#,
        )
        .unwrap();
        assert_eq!(report.imported.len(), 1);
    }
}
