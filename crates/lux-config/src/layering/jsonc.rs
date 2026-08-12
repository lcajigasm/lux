//! Thin wrapper around `jsonc-parser` 0.33.1: parses JSONC (comments,
//! trailing commas, single-quoted strings — see `ParseOptions::default()`,
//! which allows all of jsonc-parser's leniency options) straight into a
//! `serde_json::Value` via `parse_to_serde_value`, so the rest of
//! `layering` works with plain JSON values and never has to know JSONC was
//! involved.

use jsonc_parser::ParseOptions;
use jsonc_parser::errors::ParseError;
use serde_json::Value;

/// A JSONC document that failed to parse. `Display` includes the 1-based
/// line and column jsonc-parser already computes internally, so a caller
/// can point at the mistake without this crate building its own
/// line/column tracker.
#[derive(Debug)]
pub struct JsoncError(ParseError);

impl std::fmt::Display for JsoncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (line {}, column {})",
            self.0,
            self.0.line_display(),
            self.0.column_display()
        )
    }
}

impl std::error::Error for JsoncError {}

/// Parses `text` as JSONC. Empty or whitespace-only input parses as
/// `Value::Null` (jsonc-parser's own behavior for empty documents) rather
/// than an error — an empty settings file is a valid "no overrides" layer.
pub fn parse(text: &str) -> Result<Value, JsoncError> {
    jsonc_parser::parse_to_serde_value(text, &ParseOptions::default()).map_err(JsoncError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_and_trailing_commas() {
        let text = r#"{
            // line comment
            "a": 1, /* block comment */
            "b": 2,
        }"#;
        let value = parse(text).unwrap();
        assert_eq!(value["a"], 1);
        assert_eq!(value["b"], 2);
    }

    #[test]
    fn empty_input_parses_as_null() {
        assert_eq!(parse("").unwrap(), Value::Null);
        assert_eq!(parse("   \n  ").unwrap(), Value::Null);
    }

    #[test]
    fn malformed_json_is_a_real_error_with_line_and_column() {
        let err = parse("{\n  \"a\": ,\n}").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("line"), "message was: {message}");
        assert!(message.contains("column"), "message was: {message}");
    }

    #[test]
    fn nested_objects_and_arrays_round_trip() {
        let value = parse(r#"{ "a": { "b": [1, 2, 3] } }"#).unwrap();
        assert_eq!(value["a"]["b"][1], 2);
    }
}
