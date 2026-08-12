//! Imports a Sublime Text `Preferences.sublime-settings` file — JSON with
//! `//`/`/* */` comments allowed, the same as the rest of Sublime's own
//! config format, parsed with the same JSONC parser `layering` uses.
//!
//! Source keys `font_face`, `tab_size`, and `word_wrap` are confirmed
//! against `sublimetext.com/docs/settings.html`, the one place Sublime's
//! own docs name concrete setting keys. `translate_tabs_to_spaces`,
//! `trim_trailing_white_space_on_save`, `ensure_newline_at_eof_on_save`,
//! and `font_size` aren't documented in prose anywhere reachable from
//! here (Sublime's docs point at its shipped default settings file
//! instead) — they come from well-established, long-stable prior
//! knowledge of Sublime's defaults, same as this phase's keymap preset
//! loop had to do for Sublime's shortcuts.

use serde_json::{Value, json};

use super::{ImportReport, Mapped, report_from_object};
use crate::layering::JsoncError;

pub fn import(content: &str) -> Result<ImportReport, JsoncError> {
    let value = crate::layering::jsonc::parse(content)?;
    Ok(report_from_object(value, map_key))
}

fn map_key(key: &str, value: &Value) -> Mapped {
    match key {
        "tab_size" => value.as_i64().map_or(Mapped::Unsupported("integer"), |n| {
            Mapped::Imported("editor.tabWidth", json!(n))
        }),
        "translate_tabs_to_spaces" => value.as_bool().map_or(Mapped::Unsupported("boolean"), |b| {
            Mapped::Imported("editor.indentStyle", json!(if b { "space" } else { "tab" }))
        }),
        "trim_trailing_white_space_on_save" => {
            value.as_bool().map_or(Mapped::Unsupported("boolean"), |b| {
                Mapped::Imported("editor.trimTrailingWhitespace", json!(b))
            })
        }
        "ensure_newline_at_eof_on_save" => {
            value.as_bool().map_or(Mapped::Unsupported("boolean"), |b| {
                Mapped::Imported("editor.insertFinalNewline", json!(b))
            })
        }
        // Sublime allows either a plain boolean or the special "auto".
        "word_wrap" => match value {
            Value::Bool(b) => Mapped::Imported("editor.wordWrap", json!(*b)),
            Value::String(s) if s == "auto" => Mapped::Imported("editor.wordWrap", json!(true)),
            _ => Mapped::Unsupported("boolean or \"auto\""),
        },
        "font_face" => value.as_str().map_or(Mapped::Unsupported("string"), |s| {
            Mapped::Imported("editor.fontFamily", json!(s))
        }),
        "font_size" => value.as_i64().map_or(Mapped::Unsupported("integer"), |n| {
            Mapped::Imported("editor.fontSize", json!(n))
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
                // a real Sublime settings file allows comments
                "tab_size": 4,
                "translate_tabs_to_spaces": true,
                "font_face": "Menlo",
            }"#,
        )
        .unwrap();
        assert_eq!(report.imported.len(), 3);
        assert!(report.imported.iter().any(|s| s.source_key == "tab_size"
            && s.lux_key == "editor.tabWidth"
            && s.value == json!(4)));
        assert!(
            report
                .imported
                .iter()
                .any(|s| s.source_key == "translate_tabs_to_spaces"
                    && s.lux_key == "editor.indentStyle"
                    && s.value == json!("space"))
        );
        assert!(report.imported.iter().any(|s| s.source_key == "font_face"
            && s.lux_key == "editor.fontFamily"
            && s.value == json!("Menlo")));
    }

    #[test]
    fn translate_tabs_to_spaces_false_maps_to_tab_indent_style() {
        let report = import(r#"{"translate_tabs_to_spaces": false}"#).unwrap();
        assert_eq!(report.imported[0].value, json!("tab"));
    }

    #[test]
    fn word_wrap_auto_maps_to_true() {
        let report = import(r#"{"word_wrap": "auto"}"#).unwrap();
        assert_eq!(report.imported[0].lux_key, "editor.wordWrap");
        assert_eq!(report.imported[0].value, json!(true));
    }

    #[test]
    fn an_unrecognized_key_is_reported_as_unknown() {
        let report = import(r#"{"ignored_packages": ["Vintage"]}"#).unwrap();
        assert_eq!(report.imported.len(), 0);
        assert_eq!(report.ignored.len(), 1);
        assert_eq!(report.ignored[0].source_key, "ignored_packages");
        assert_eq!(
            report.ignored[0].reason,
            super::super::IgnoreReason::UnknownKey
        );
    }

    #[test]
    fn a_recognized_key_with_the_wrong_value_type_is_reported_as_unsupported() {
        let report = import(r#"{"tab_size": "four"}"#).unwrap();
        assert_eq!(report.imported.len(), 0);
        assert_eq!(
            report.ignored[0].reason,
            super::super::IgnoreReason::UnsupportedValue {
                expected: "integer"
            }
        );
    }

    #[test]
    fn every_key_lands_in_exactly_one_list() {
        let report =
            import(r#"{"tab_size": 4, "ignored_packages": [], "font_size": "big"}"#).unwrap();
        assert_eq!(report.imported.len() + report.ignored.len(), 3);
    }

    #[test]
    fn malformed_jsonc_is_a_real_error() {
        assert!(import("{ not valid").is_err());
    }
}
