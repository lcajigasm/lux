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
        key: "editor.insertFinalNewline",
        kind: PropertyKind::Boolean,
        description: "Ensure the file ends with a single newline on save.",
    },
    PropertySchema {
        key: "editor.wordWrap",
        kind: PropertyKind::Boolean,
        description: "Wrap lines that exceed the viewport width.",
    },
];

pub fn find(key: &str) -> Option<&'static PropertySchema> {
    PROPERTIES.iter().find(|p| p.key == key)
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
}
