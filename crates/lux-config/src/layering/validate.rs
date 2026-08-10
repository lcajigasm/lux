//! Validates a single JSONC settings document against
//! [`crate::schema::PROPERTIES`], reporting every problem with a 1-based
//! line and column pointing at the exact offending text — the "validación
//! con errores útiles señalando línea y columna" half of Phase 5
//! deliverable 1, deferred from the first `layering` loop this phase
//! (`docs/receipts/phase-05-luxconfig-jsonc-precedencia-capas-recarga.md`)
//! until a schema existed to validate against
//! (`docs/receipts/phase-05-panel-preferencias-o-autocompletado-esquema.md`).
//!
//! Deliberately built on `jsonc_parser::parse_to_ast`, not `completion`'s
//! own tolerant scanner: validation is only meaningful once a document is
//! syntactically complete — there's nothing useful to say about "is this
//! value the right type" for a document that isn't finished being typed
//! yet (that's what `completion` is for instead). A syntactically invalid
//! document produces exactly one diagnostic — `jsonc-parser`'s own parse
//! error, which already carries a real line and column — rather than every
//! schema rule silently failing to run against a tree that doesn't exist.

use jsonc_parser::ast::{ObjectPropName, Value as JsonValue};
use jsonc_parser::common::{Range, Ranged};
use jsonc_parser::{CollectOptions, ParseOptions};

use crate::schema::{self, PropertyKind};

/// One problem found in a settings document, with a 1-based line and a
/// 1-based, `char`-counted column — the same convention
/// `jsonc_parser::errors::ParseError`'s own `Display` already uses, so a
/// syntax-error diagnostic and a schema diagnostic from the same document
/// point at positions the same way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

/// Validates `text` as a Lux settings document. Empty when there's nothing
/// wrong with it.
pub fn validate(text: &str) -> Vec<Diagnostic> {
    let parsed =
        jsonc_parser::parse_to_ast(text, &CollectOptions::default(), &ParseOptions::default());
    let ast = match parsed {
        Ok(ast) => ast,
        Err(err) => {
            return vec![Diagnostic {
                message: err.to_string(),
                line: err.line_display(),
                column: err.column_display(),
            }];
        }
    };

    let Some(value) = ast.value else {
        return vec![Diagnostic {
            message: "empty document: expected a JSON object".to_string(),
            line: 1,
            column: 1,
        }];
    };

    let JsonValue::Object(object) = value else {
        let (line, column) = line_and_column(text, value.range().start);
        return vec![Diagnostic {
            message: "settings must be a JSON object".to_string(),
            line,
            column,
        }];
    };

    let mut diagnostics = Vec::new();
    for prop in &object.properties {
        let key = prop.name.as_str();
        let Some(schema) = schema::find(key) else {
            let (line, column) = line_and_column(text, name_range(&prop.name).start);
            diagnostics.push(Diagnostic {
                message: format!("unknown setting {key:?}"),
                line,
                column,
            });
            continue;
        };
        if let Some(message) = check_value(schema.kind, &prop.value) {
            let (line, column) = line_and_column(text, prop.value.range().start);
            diagnostics.push(Diagnostic {
                message,
                line,
                column,
            });
        }
    }
    diagnostics
}

fn name_range(name: &ObjectPropName<'_>) -> Range {
    match name {
        ObjectPropName::String(lit) => lit.range,
        ObjectPropName::Word(lit) => lit.range,
    }
}

/// `None` when `value` matches `kind`; `Some(message)` describing the
/// mismatch otherwise.
fn check_value(kind: PropertyKind, value: &JsonValue<'_>) -> Option<String> {
    match (kind, value) {
        (PropertyKind::String, JsonValue::StringLit(_)) => None,
        (PropertyKind::String, other) => Some(mismatch("a string", other)),

        (PropertyKind::Boolean, JsonValue::BooleanLit(_)) => None,
        (PropertyKind::Boolean, other) => Some(mismatch("a boolean", other)),

        (PropertyKind::Number, JsonValue::NumberLit(_)) => None,
        (PropertyKind::Number, other) => Some(mismatch("a number", other)),

        (PropertyKind::Integer, JsonValue::NumberLit(n)) => {
            if n.value.contains('.') {
                Some(format!(
                    "expected an integer, found the fractional number {}",
                    n.value
                ))
            } else {
                None
            }
        }
        (PropertyKind::Integer, other) => Some(mismatch("an integer", other)),

        (PropertyKind::Enum(values), JsonValue::StringLit(s)) => {
            if values.contains(&s.value.as_ref()) {
                None
            } else {
                Some(format!(
                    "invalid value {:?}: expected one of {}",
                    s.value,
                    values.join(", ")
                ))
            }
        }
        (PropertyKind::Enum(values), other) => {
            Some(mismatch(&format!("one of {}", values.join(", ")), other))
        }
    }
}

fn mismatch(expected: &str, found: &JsonValue<'_>) -> String {
    format!("expected {expected}, found {}", kind_name(found))
}

fn kind_name(value: &JsonValue<'_>) -> &'static str {
    match value {
        JsonValue::StringLit(_) => "a string",
        JsonValue::NumberLit(_) => "a number",
        JsonValue::BooleanLit(_) => "a boolean",
        JsonValue::Object(_) => "an object",
        JsonValue::Array(_) => "an array",
        JsonValue::NullKeyword(_) => "null",
    }
}

/// 1-based line, 1-based `char`-counted column for the byte offset
/// `pos` — the same algorithm `jsonc_parser::errors::ParseError` uses
/// internally for its own line/column (not exposed publicly, so this is a
/// small, independent, equally-simple reimplementation rather than a
/// dependency on private crate internals).
fn line_and_column(text: &str, pos: usize) -> (usize, usize) {
    let mut line = 0;
    let mut column = 0;
    for c in text[..pos].chars() {
        if c == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line + 1, column + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_document_has_no_diagnostics() {
        let text = r#"{
            "editor.fontFamily": "Menlo",
            "editor.fontSize": 13,
            "editor.indentStyle": "space"
        }"#;
        assert!(validate(text).is_empty());
    }

    #[test]
    fn an_unknown_setting_is_reported_at_the_keys_position() {
        let text = "{\n  \"editor.doesNotExist\": 1\n}";
        let diagnostics = validate(text);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("editor.doesNotExist"));
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].column, 3);
    }

    #[test]
    fn a_type_mismatch_is_reported_at_the_values_position() {
        let text = "{\n  \"editor.fontSize\": \"big\"\n}";
        let diagnostics = validate(text);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("expected a number"));
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].column, 22);
    }

    #[test]
    fn a_fractional_value_for_an_integer_key_is_reported() {
        let text = r#"{"editor.tabWidth": 2.5}"#;
        let diagnostics = validate(text);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("fractional"));
    }

    #[test]
    fn an_integer_value_for_an_integer_key_is_fine() {
        let text = r#"{"editor.tabWidth": 2}"#;
        assert!(validate(text).is_empty());
    }

    #[test]
    fn an_invalid_enum_value_lists_the_valid_ones() {
        let text = r#"{"editor.indentStyle": "spaces"}"#;
        let diagnostics = validate(text);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("tab"));
        assert!(diagnostics[0].message.contains("space"));
    }

    #[test]
    fn a_valid_enum_value_is_fine() {
        let text = r#"{"editor.indentStyle": "tab"}"#;
        assert!(validate(text).is_empty());
    }

    #[test]
    fn multiple_problems_are_all_reported_not_just_the_first() {
        let text = r#"{"editor.fontSize": "big", "editor.doesNotExist": 1}"#;
        assert_eq!(validate(text).len(), 2);
    }

    #[test]
    fn malformed_jsonc_syntax_is_one_diagnostic_with_a_real_line_and_column() {
        let text = "{\n  \"editor.fontSize\": ,\n}";
        let diagnostics = validate(text);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert!(diagnostics[0].column > 1);
    }

    #[test]
    fn an_empty_document_is_a_single_clear_diagnostic() {
        let diagnostics = validate("");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 1));
    }

    #[test]
    fn a_top_level_array_is_rejected() {
        let diagnostics = validate("[1, 2, 3]");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("object"));
    }

    #[test]
    fn comments_before_the_offending_value_do_not_throw_off_the_position() {
        let text = "{\n  // a comment\n  \"editor.fontSize\": \"big\"\n}";
        let diagnostics = validate(text);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
    }
}
