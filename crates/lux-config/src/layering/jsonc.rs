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

/// Why [`set_top_level`] left a document alone.
#[derive(Debug)]
pub enum EditError {
    /// The text is not valid JSONC.
    Parse(JsoncError),
    /// The document parses, but its root is not an object.
    NotAnObject,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(f),
            Self::NotAnObject => f.write_str("the top level is not a JSON object"),
        }
    }
}

impl std::error::Error for EditError {}

/// Sets each top-level `key` of the JSONC object in `text` to its value and
/// returns the edited text. Only the value spans change: comments, trailing
/// commas, key order and formatting elsewhere survive. Missing keys are
/// appended. Empty input is treated as an empty object. A document that
/// does not parse, or whose root is not an object, is an error, never
/// replaced.
pub fn set_top_level(text: &str, pairs: &[(String, Value)]) -> Result<String, EditError> {
    use jsonc_parser::cst::CstRootNode;
    let root = CstRootNode::parse(text, &ParseOptions::default())
        .map_err(|error| EditError::Parse(JsoncError(error)))?;
    let object = match root.value() {
        None => root.object_value_or_set(),
        Some(_) => root.object_value().ok_or(EditError::NotAnObject)?,
    };
    for (key, value) in pairs {
        match object.get(key) {
            Some(prop) => prop.set_value(cst_value(value)),
            None => {
                object.append(key, cst_value(value));
            }
        }
    }
    Ok(root.to_string())
}

fn cst_value(value: &Value) -> jsonc_parser::cst::CstInputValue {
    use jsonc_parser::cst::CstInputValue;
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(cst_value).collect()),
        Value::Object(map) => CstInputValue::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), cst_value(value)))
                .collect(),
        ),
    }
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

    fn pairs(items: &[(&str, Value)]) -> Vec<(String, Value)> {
        items
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn set_top_level_keeps_comments_and_trailing_commas() {
        let text = "{\n  // font\n  \"a\": 1, /* keep */\n  \"b\": \"x\",\n}";
        let edited = set_top_level(
            text,
            &pairs(&[("a", serde_json::json!(2)), ("c", serde_json::json!(true))]),
        )
        .unwrap();
        assert!(edited.contains("// font"), "{edited}");
        assert!(edited.contains("/* keep */"), "{edited}");
        let value = parse(&edited).unwrap();
        assert_eq!(value["a"], 2);
        assert_eq!(value["b"], "x");
        assert_eq!(value["c"], true);
    }

    #[test]
    fn set_top_level_refuses_broken_or_non_object_documents() {
        let one = pairs(&[("a", serde_json::json!(1))]);
        assert!(matches!(
            set_top_level("{ \"a\": ,", &one),
            Err(EditError::Parse(_))
        ));
        assert!(matches!(
            set_top_level("[1, 2]", &one),
            Err(EditError::NotAnObject)
        ));
        let empty = set_top_level("", &one).unwrap();
        assert_eq!(parse(&empty).unwrap()["a"], 1);
    }

    #[test]
    fn nested_objects_and_arrays_round_trip() {
        let value = parse(r#"{ "a": { "b": [1, 2, 3] } }"#).unwrap();
        assert_eq!(value["a"]["b"][1], 2);
    }
}
