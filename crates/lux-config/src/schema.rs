//! Lux's own settings schema — the canonical list of settings keys this
//! crate knows about, each one's value shape, and a short description.
//! This is `completion`'s data source (see that module for why "file
//! editing with schema-aware completion" was chosen over a preferences UI
//! panel for Phase 5 deliverable 6).
//!
//! Every key here already exists elsewhere in this crate under the exact
//! same name: `font_settings`'s four `editor.font*`/`editor.lineHeight`
//! keys, and the indent/whitespace keys `import::sublime`/`import::vscode`/
//! `import::jetbrains` already map onto. This module doesn't introduce a
//! new vocabulary — it's the first place that vocabulary is written down
//! as one list instead of scattered across each consumer's own match
//! statement. Unifying those consumers to read from this list instead of
//! their own literals is real future cleanup, deliberately not done here:
//! this loop's scope is deliverable 6, not refactoring deliverables 2-5's
//! already-closed, already-tested code.

/// The shape of value a property accepts. `Enum`/`Boolean` have a fixed,
/// enumerable set of valid values — `completion` can suggest all of them.
/// `String`/`Integer`/`Number` are open-ended — there's nothing honest to
/// suggest beyond the property existing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyKind {
    String,
    Integer,
    /// Distinct from `Integer` for documentation purposes (`editor.fontSize`/
    /// `editor.lineHeight` are routinely fractional) even though the two
    /// behave identically in `completion` today — both are open-ended, no
    /// discrete values to suggest either way.
    Number,
    Boolean,
    Enum(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertySchema {
    pub key: &'static str,
    pub kind: PropertyKind,
    pub description: &'static str,
}

pub const PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        key: "editor.fontFamily",
        kind: PropertyKind::String,
        description: "Typeface used for editing.",
    },
    PropertySchema {
        key: "editor.fontSize",
        kind: PropertyKind::Number,
        description: "Font size in points.",
    },
    PropertySchema {
        key: "editor.lineHeight",
        kind: PropertyKind::Number,
        description: "Line height as a multiplier of the font size.",
    },
    PropertySchema {
        key: "editor.fontLigatures",
        kind: PropertyKind::Boolean,
        description: "Whether to render programming ligatures.",
    },
    PropertySchema {
        key: "editor.tabWidth",
        kind: PropertyKind::Integer,
        description: "Number of columns a tab character occupies.",
    },
    PropertySchema {
        key: "editor.indentSize",
        kind: PropertyKind::Integer,
        description: "Number of columns one level of indentation occupies.",
    },
    PropertySchema {
        key: "editor.indentStyle",
        kind: PropertyKind::Enum(&["tab", "space"]),
        description: "Whether new indentation uses tabs or spaces.",
    },
    PropertySchema {
        key: "editor.trimTrailingWhitespace",
        kind: PropertyKind::Boolean,
        description: "Remove trailing whitespace from each line on save.",
    },
    PropertySchema {
        key: "editor.trimTrailingWhitespacePreserveCursor",
        kind: PropertyKind::Boolean,
        description: "Keep trailing whitespace on lines with active cursors on save.",
    },
    PropertySchema {
        key: "editor.insertFinalNewline",
        kind: PropertyKind::Boolean,
        description: "Ensure the file ends with a single newline on save.",
    },
    PropertySchema {
        key: "editor.wordWrap",
        kind: PropertyKind::Boolean,
        description: "Wrap lines that exceed the viewport width.",
    },
    PropertySchema {
        key: "editor.distractionFreeColumnWidth",
        kind: PropertyKind::Integer,
        description: "Column width in characters for distraction free mode.",
    },
];

pub fn find(key: &str) -> Option<&'static PropertySchema> {
    PROPERTIES.iter().find(|p| p.key == key)
}

/// The "esquema publicado" half of Phase 5 deliverable 1: `PROPERTIES`
/// exported as a real [JSON Schema](https://json-schema.org/) document
/// (draft-07), the standard format external tooling — a `$schema`
/// reference in a real editor, a future language server, `jsonc-parser`-
/// unrelated JSON Schema validators — already knows how to consume,
/// instead of a Lux-specific format only this crate could read.
/// `layering::validate` is this crate's *own* validator and doesn't
/// depend on this function; the two independently agree because both
/// read from the same [`PROPERTIES`] list.
pub fn to_json_schema() -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    for property in PROPERTIES {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "description".to_string(),
            serde_json::Value::String(property.description.to_string()),
        );
        match property.kind {
            PropertyKind::String => {
                entry.insert("type".to_string(), serde_json::json!("string"));
            }
            PropertyKind::Integer => {
                entry.insert("type".to_string(), serde_json::json!("integer"));
            }
            PropertyKind::Number => {
                entry.insert("type".to_string(), serde_json::json!("number"));
            }
            PropertyKind::Boolean => {
                entry.insert("type".to_string(), serde_json::json!("boolean"));
            }
            PropertyKind::Enum(values) => {
                entry.insert("type".to_string(), serde_json::json!("string"));
                entry.insert("enum".to_string(), serde_json::json!(values));
            }
        }
        properties.insert(property.key.to_string(), serde_json::Value::Object(entry));
    }
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Lux settings",
        "type": "object",
        "properties": serde_json::Value::Object(properties),
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_property_key_is_unique() {
        let mut keys: Vec<&str> = PROPERTIES.iter().map(|p| p.key).collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), before, "duplicate key in PROPERTIES");
    }

    #[test]
    fn find_returns_the_matching_property() {
        let found = find("editor.indentStyle").unwrap();
        assert_eq!(found.kind, PropertyKind::Enum(&["tab", "space"]));
    }

    #[test]
    fn find_returns_none_for_an_unknown_key() {
        assert!(find("editor.doesNotExist").is_none());
    }

    #[test]
    fn json_schema_has_an_entry_for_every_property() {
        let schema = to_json_schema();
        let properties = schema["properties"].as_object().unwrap();
        assert_eq!(properties.len(), PROPERTIES.len());
        for property in PROPERTIES {
            assert!(
                properties.contains_key(property.key),
                "missing {} in generated JSON Schema",
                property.key
            );
        }
    }

    #[test]
    fn json_schema_represents_an_enum_property_with_type_and_enum() {
        let schema = to_json_schema();
        let indent_style = &schema["properties"]["editor.indentStyle"];
        assert_eq!(indent_style["type"], "string");
        assert_eq!(indent_style["enum"], serde_json::json!(["tab", "space"]));
    }

    #[test]
    fn json_schema_rejects_additional_properties() {
        let schema = to_json_schema();
        assert_eq!(schema["additionalProperties"], false);
    }
}
