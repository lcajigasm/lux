//! `.editorconfig` support (Phase 5 deliverable 3, `.editorconfig` half) via
//! `ec4rs`, a dedicated, spec-compliant EditorConfig implementation rather
//! than a hand-rolled one: EditorConfig's glob dialect (character classes,
//! `{a,b}` brace alternation, `{1..10}` numeric ranges) is its own grammar,
//! different enough from gitignore-style globs that reusing `ignore`/
//! `globset` here wouldn't actually cover it, and re-deriving that grammar
//! by hand would duplicate real, already-solved, spec-tested parsing work.
//!
//! This module is a thin, typed wrapper: `ec4rs::properties_of` already
//! does the real work this deliverable asks for — walking from a file's
//! directory up toward the filesystem root (or a `root = true`
//! `.editorconfig`), matching each file's `[glob]` sections against the
//! target path, and merging with closer files and later-declared sections
//! overriding earlier ones. What this module adds is exposing that as a
//! typed [`EditorConfig`] instead of `ec4rs`'s stringly-keyed
//! [`ec4rs::Properties`] map.

use std::path::Path;

pub use ec4rs::property::{Charset, EndOfLine, IndentSize, IndentStyle, MaxLineLen};

/// The standard EditorConfig properties this crate resolves. Each field is
/// `None` when no matching section set it — that's not the same as
/// "malformed", it just means this property is unconfigured for this path,
/// and a caller should fall back to its own default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditorConfig {
    pub indent_style: Option<IndentStyle>,
    pub indent_size: Option<IndentSize>,
    pub tab_width: Option<usize>,
    pub end_of_line: Option<EndOfLine>,
    pub charset: Option<Charset>,
    pub trim_trailing_whitespace: Option<bool>,
    pub insert_final_newline: Option<bool>,
    pub max_line_length: Option<MaxLineLen>,
}

/// Resolves the effective EditorConfig properties for `path`. Returns
/// `Ok(EditorConfig::default())` (every field `None`) when no
/// `.editorconfig` file in the walk covers `path` at all — that's a normal,
/// unconfigured outcome, not an error. Only real I/O failure or a genuinely
/// malformed `.editorconfig` file (reported by `ec4rs` with the offending
/// file and line) returns `Err`; a *property* with a value `ec4rs` can't
/// parse degrades to `None` for that one field rather than failing the
/// whole resolution, since one unrecognized value in an otherwise-good file
/// shouldn't make every other property in it unusable.
pub fn resolve(path: impl AsRef<Path>) -> Result<EditorConfig, ec4rs::Error> {
    let props = ec4rs::properties_of(path)?;
    Ok(EditorConfig {
        indent_style: props.get().ok(),
        indent_size: props.get().ok(),
        tab_width: match props.get::<ec4rs::property::TabWidth>() {
            Ok(ec4rs::property::TabWidth::Value(width)) => Some(width),
            Err(_) => None,
        },
        end_of_line: props.get().ok(),
        charset: props.get().ok(),
        trim_trailing_whitespace: match props.get::<ec4rs::property::TrimTrailingWs>() {
            Ok(ec4rs::property::TrimTrailingWs::Value(v)) => Some(v),
            Err(_) => None,
        },
        insert_final_newline: match props.get::<ec4rs::property::FinalNewline>() {
            Ok(ec4rs::property::FinalNewline::Value(v)) => Some(v),
            Err(_) => None,
        },
        max_line_length: props.get().ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-config-editorconfig-test-{label}-{:?}-{}",
            thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(&dir).unwrap()
    }

    #[test]
    fn resolves_properties_from_a_matching_glob_section() {
        let dir = temp_dir("basic");
        fs::write(
            dir.join(".editorconfig"),
            "root = true\n[*.rs]\nindent_style = space\nindent_size = 4\n",
        )
        .unwrap();
        let file = dir.join("main.rs");
        fs::write(&file, "fn main() {}").unwrap();

        let resolved = resolve(&file).unwrap();
        assert_eq!(resolved.indent_style, Some(IndentStyle::Spaces));
        assert_eq!(resolved.indent_size, Some(IndentSize::Value(4)));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_section_that_does_not_match_the_path_contributes_nothing() {
        let dir = temp_dir("unmatched");
        fs::write(
            dir.join(".editorconfig"),
            "root = true\n[*.py]\nindent_style = tab\n",
        )
        .unwrap();
        let file = dir.join("main.rs");
        fs::write(&file, "fn main() {}").unwrap();

        let resolved = resolve(&file).unwrap();
        assert_eq!(resolved, EditorConfig::default());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_closer_editorconfig_overrides_one_property_while_inheriting_others() {
        let outer = temp_dir("inherit-outer");
        let inner = outer.join("inner");
        fs::create_dir_all(&inner).unwrap();
        fs::write(
            outer.join(".editorconfig"),
            "root = true\n[*]\nindent_style = space\nindent_size = 2\n",
        )
        .unwrap();
        fs::write(inner.join(".editorconfig"), "[*]\nindent_size = 4\n").unwrap();
        let file = inner.join("main.rs");
        fs::write(&file, "fn main() {}").unwrap();

        let resolved = resolve(&file).unwrap();
        // Overridden by the closer file...
        assert_eq!(resolved.indent_size, Some(IndentSize::Value(4)));
        // ...but the outer file's property the inner one never mentions
        // still applies — real inheritance, not a full replacement.
        assert_eq!(resolved.indent_style, Some(IndentStyle::Spaces));

        fs::remove_dir_all(&outer).ok();
    }

    #[test]
    fn root_true_stops_the_upward_walk() {
        let outer = temp_dir("root-outer");
        let inner = outer.join("inner");
        fs::create_dir_all(&inner).unwrap();
        // If `root = true` below were ignored, this outer file's
        // `indent_style = tab` would incorrectly reach `inner`.
        fs::write(outer.join(".editorconfig"), "[*]\nindent_style = tab\n").unwrap();
        fs::write(
            inner.join(".editorconfig"),
            "root = true\n[*]\nindent_style = space\n",
        )
        .unwrap();
        let file = inner.join("main.rs");
        fs::write(&file, "fn main() {}").unwrap();

        let resolved = resolve(&file).unwrap();
        assert_eq!(resolved.indent_style, Some(IndentStyle::Spaces));

        fs::remove_dir_all(&outer).ok();
    }

    #[test]
    fn brace_alternation_globs_match_every_listed_extension() {
        let dir = temp_dir("braces");
        fs::write(
            dir.join(".editorconfig"),
            "root = true\n[*.{js,ts}]\nindent_size = 2\n",
        )
        .unwrap();
        fs::write(dir.join("a.js"), "").unwrap();
        fs::write(dir.join("a.ts"), "").unwrap();
        fs::write(dir.join("a.py"), "").unwrap();

        assert_eq!(
            resolve(dir.join("a.js")).unwrap().indent_size,
            Some(IndentSize::Value(2))
        );
        assert_eq!(
            resolve(dir.join("a.ts")).unwrap().indent_size,
            Some(IndentSize::Value(2))
        );
        assert_eq!(resolve(dir.join("a.py")).unwrap().indent_size, None);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn indent_size_of_tab_is_a_distinct_value_from_a_numeric_size() {
        let dir = temp_dir("tab-size");
        fs::write(
            dir.join(".editorconfig"),
            "root = true\n[*]\nindent_size = tab\ntab_width = 8\n",
        )
        .unwrap();
        let file = dir.join("main.rs");
        fs::write(&file, "").unwrap();

        let resolved = resolve(&file).unwrap();
        assert_eq!(resolved.indent_size, Some(IndentSize::UseTabWidth));
        assert_eq!(resolved.tab_width, Some(8));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn boolean_properties_parse_to_real_bools() {
        let dir = temp_dir("bools");
        fs::write(
            dir.join(".editorconfig"),
            "root = true\n[*]\ntrim_trailing_whitespace = true\ninsert_final_newline = false\n",
        )
        .unwrap();
        let file = dir.join("main.rs");
        fs::write(&file, "").unwrap();

        let resolved = resolve(&file).unwrap();
        assert_eq!(resolved.trim_trailing_whitespace, Some(true));
        assert_eq!(resolved.insert_final_newline, Some(false));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_malformed_editorconfig_file_is_a_real_actionable_error() {
        let dir = temp_dir("malformed");
        // A key with no value, inside an already-valid section, is not
        // valid EditorConfig syntax — unlike a bad line in the file's
        // preamble (before its first section header), which `ec4rs`
        // treats as "this file doesn't apply" rather than an error, a bad
        // line *inside* a section is a real, surfaced parse failure.
        fs::write(
            dir.join(".editorconfig"),
            "root = true\n[*.rs]\nindent_style = \n",
        )
        .unwrap();
        let file = dir.join("main.rs");
        fs::write(&file, "").unwrap();

        let err = resolve(&file).unwrap_err();
        let message = err.to_string();
        assert!(message.contains(".editorconfig"), "message was: {message}");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_editorconfig_anywhere_resolves_to_every_field_unset() {
        let dir = temp_dir("none");
        let file = dir.join("main.rs");
        fs::write(&file, "").unwrap();

        // No `.editorconfig` was written anywhere in `dir`'s chain, and
        // this dev environment has none above its temp directory either.
        let resolved = resolve(&file).unwrap();
        assert_eq!(resolved, EditorConfig::default());

        fs::remove_dir_all(&dir).ok();
    }
}
