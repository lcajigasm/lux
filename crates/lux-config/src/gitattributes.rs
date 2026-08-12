//! `.gitattributes` support for line-ending normalization (Phase 5
//! deliverable 3, `.gitattributes` half — narrowed to what the deliverable
//! text actually asks for, "para normalización de finales de línea", not
//! the full `.gitattributes` feature set: diff/merge drivers, filters,
//! `export-ignore`, and `linguist-*` hints are out of scope). Pattern
//! matching and precedence are delegated to `gix-attributes`, the same
//! gitoxide-family crate `lux-git` will use for real Git integration in
//! Phase 6 (ADR 0010) — real gitignore-style glob matching and real
//! attribute-assignment precedence (a later-declared pattern overrides an
//! earlier one for the same attribute, matching Git's own behavior), not a
//! hand-rolled reimplementation of either.

use std::borrow::Cow;

use bstr::ByteSlice;
use gix_attributes::glob::pattern::Case;
use gix_attributes::search::{MetadataCollection, Outcome};
use gix_attributes::state::ValueRef;
use gix_attributes::{Search, StateRef};

/// How a path's line endings should be handled, resolved from its `text`/
/// `eol` `.gitattributes` state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EolNormalization {
    /// `eol=lf`/`eol=crlf`, or a plain `text` with no explicit `eol` (Git
    /// always normalizes line endings to LF in the repository itself
    /// regardless of platform, converting only on checkout).
    Normalize(LineEnding),
    /// `text=auto`: normalize only if Git's own text/binary heuristic
    /// considers the content text. This crate doesn't run that heuristic —
    /// resolving `Auto` further is a caller's job.
    Auto,
    /// `-text`, or the built-in `binary` macro (which expands to `-text
    /// -diff -merge`, see [`GitAttributes::new`]): never touch line
    /// endings for this path.
    Disabled,
    /// No `.gitattributes` rule addresses this path's line endings at all.
    Unspecified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

/// A set of parsed `.gitattributes` files, queryable for a given path's
/// line-ending policy.
pub struct GitAttributes {
    search: Search,
    collection: MetadataCollection,
}

impl Default for GitAttributes {
    fn default() -> Self {
        Self::new()
    }
}

impl GitAttributes {
    /// Starts with Git's own built-in `binary` macro pre-registered — the
    /// same `[attr]binary -diff -merge -text` Git itself always defines —
    /// so `binary` in a `.gitattributes` file behaves the way Git users
    /// actually expect without every file having to spell out `-text`
    /// explicitly.
    pub fn new() -> Self {
        let mut collection = MetadataCollection::default();
        let mut search = Search::default();
        search.add_patterns_buffer(
            b"[attr]binary -diff -merge -text",
            "[builtin]".into(),
            None,
            &mut collection,
            true,
        );
        GitAttributes { search, collection }
    }

    /// Adds one `.gitattributes` file's content. Call in root-to-leaf order
    /// (the repository root's file first, then progressively deeper
    /// directories): later calls take precedence over earlier ones for
    /// patterns matching the same attribute, mirroring Git's own "a
    /// `.gitattributes` closer to the file wins" behavior. This module
    /// doesn't discover files on disk itself — same as `layering` leaving
    /// "which files apply" to its caller.
    pub fn add_file(&mut self, content: &str, source_label: &str) {
        self.search.add_patterns_buffer(
            content.as_bytes(),
            source_label.into(),
            None,
            &mut self.collection,
            true,
        );
    }

    /// The line-ending policy for `relative_path` (relative to wherever
    /// `add_file`'s patterns are rooted — typically the repository root).
    pub fn eol_normalization(&self, relative_path: &str) -> EolNormalization {
        let mut outcome = Outcome::default();
        outcome.initialize_with_selection(&self.collection, ["text", "eol"]);
        self.search.pattern_matching_relative_path(
            relative_path.as_bytes().as_bstr(),
            Case::Sensitive,
            None,
            &mut outcome,
        );

        let mut text = StateRef::Unspecified;
        let mut eol = StateRef::Unspecified;
        for matched in outcome.iter_selected() {
            match matched.assignment.name.as_str() {
                "text" => text = matched.assignment.state,
                "eol" => eol = matched.assignment.state,
                _ => {}
            }
        }

        // `-text` (explicit or via the `binary` macro) wins outright: no
        // eol setting can re-enable normalization once text is unset.
        if let StateRef::Unset = text {
            return EolNormalization::Disabled;
        }
        if let StateRef::Value(value) = eol {
            return match value_str(value).as_ref() {
                "lf" => EolNormalization::Normalize(LineEnding::Lf),
                "crlf" => EolNormalization::Normalize(LineEnding::CrLf),
                _ => EolNormalization::Unspecified,
            };
        }
        match text {
            StateRef::Value(value) if value_str(value).as_ref() == "auto" => EolNormalization::Auto,
            StateRef::Set => EolNormalization::Normalize(LineEnding::Lf),
            _ => EolNormalization::Unspecified,
        }
    }
}

fn value_str(value: ValueRef<'_>) -> Cow<'_, str> {
    value.as_bstr().to_str_lossy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eol_lf_normalizes_to_lf() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.sh text eol=lf\n", "<root>");
        assert_eq!(
            attrs.eol_normalization("script.sh"),
            EolNormalization::Normalize(LineEnding::Lf)
        );
    }

    #[test]
    fn eol_crlf_normalizes_to_crlf() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.bat text eol=crlf\n", "<root>");
        assert_eq!(
            attrs.eol_normalization("run.bat"),
            EolNormalization::Normalize(LineEnding::CrLf)
        );
    }

    #[test]
    fn plain_text_with_no_eol_normalizes_to_lf() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.txt text\n", "<root>");
        assert_eq!(
            attrs.eol_normalization("readme.txt"),
            EolNormalization::Normalize(LineEnding::Lf)
        );
    }

    #[test]
    fn text_auto_resolves_to_auto() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.md text=auto\n", "<root>");
        assert_eq!(attrs.eol_normalization("readme.md"), EolNormalization::Auto);
    }

    #[test]
    fn explicit_minus_text_disables_normalization() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.png -text\n", "<root>");
        assert_eq!(
            attrs.eol_normalization("logo.png"),
            EolNormalization::Disabled
        );
    }

    #[test]
    fn the_binary_macro_disables_normalization_without_spelling_out_minus_text() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.exe binary\n", "<root>");
        assert_eq!(
            attrs.eol_normalization("tool.exe"),
            EolNormalization::Disabled
        );
    }

    #[test]
    fn eol_is_ignored_once_text_is_explicitly_unset() {
        let mut attrs = GitAttributes::new();
        // A contradictory but real-world-possible line: -text should win.
        attrs.add_file("*.dat -text eol=lf\n", "<root>");
        assert_eq!(
            attrs.eol_normalization("blob.dat"),
            EolNormalization::Disabled
        );
    }

    #[test]
    fn an_unmatched_path_is_unspecified() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.sh text eol=lf\n", "<root>");
        assert_eq!(
            attrs.eol_normalization("main.rs"),
            EolNormalization::Unspecified
        );
    }

    #[test]
    fn a_later_added_file_overrides_an_earlier_one_for_the_same_pattern() {
        let mut attrs = GitAttributes::new();
        attrs.add_file("*.txt text eol=crlf\n", "<root>");
        // Simulates a deeper directory's own `.gitattributes` overriding
        // the root's — added later, wins, per Git's real precedence.
        attrs.add_file("*.txt text eol=lf\n", "<subdir>");
        assert_eq!(
            attrs.eol_normalization("notes.txt"),
            EolNormalization::Normalize(LineEnding::Lf)
        );
    }
}
