//! Auto-indent per language (Phase 3 deliverable 5): the indent level for a
//! line is derived from the real parse tree's nesting, never from the
//! `{([:` character-counting heuristic v0 used (`LUX_PROMPT_MAESTRO.md` is
//! explicit that this is the mistake this replaces). A text scan can't tell
//! a brace inside a string or comment from a real one, and it has no notion
//! of "this grammar's body starts on a different row than its header" —
//! both of which the tree already knows.
//!
//! Each language exposes the set of node kinds that represent one indented
//! body (a `block`, a `compound_statement`, an `object` literal, ...) —
//! established once per grammar by inspecting its real parse tree, the same
//! way `queries/<lang>/highlights.scm` establishes each grammar's capture
//! names. [`indent_level`] then just counts how many of those kinds enclose
//! the target line. Wrapper/header nodes (`if_expression`, `elif_clause`,
//! `else_clause`, ...) are never in the list, so they never add a level on
//! their own — which is also what makes an `else`/`elif` line dedent
//! correctly without any keyword-specific handling.

use crate::Document;
use tree_sitter::{Node, Point};

/// The node kinds that represent one indented body in `language`'s grammar.
/// Not exhaustive for every construct in every grammar — only what real
/// source samples from each vendored grammar were checked against — but
/// safe to extend: an unrecognized kind here simply never matches, so
/// indent falls back to one level less than it ideally would, never to
/// wrong/negative output.
fn block_kinds(language: crate::LanguageId) -> &'static [&'static str] {
    use crate::LanguageId::*;
    match language {
        Rust | Go => &["block"],
        Python => &["block"],
        C | Cpp => &["compound_statement"],
        Java => &["block", "class_body", "interface_body"],
        JavaScript | TypeScript | Tsx => &["statement_block", "class_body", "object", "array"],
        Kotlin => &["function_body", "control_structure_body", "class_body"],
        Swift => &["statements", "class_body"],
        Html => &["element"],
        Css => &["block"],
        Json => &["object", "array"],
        Yaml => &["block_mapping", "block_sequence"],
        Markdown => &["list"],
        // tree-sitter-bash gives `if`/`while`/`for` no separate body node for
        // their primary branch, so `if_statement` itself must double as
        // "one level for its direct statement children" — verified against
        // `if ... then ... fi` and `for ... do ... done`. `elif`/`else`
        // clauses are siblings of that same `if_statement`, not nested
        // inside another countable kind, so their own line correctly stays
        // un-indented (nothing new to count), but re-dedenting *to* them
        // (rather than to the loop's `do`/`done` or `fi`) isn't derived
        // specially here.
        // ponytail: elif/else branch lines re-indent via the same
        // `if_statement` count as the loop body instead of their own
        // dedented level; add `elif_clause`/`else_clause` handling if that
        // proves wrong in practice.
        Bash => &["if_statement", "do_group"],
        Toml => &["array"],
        // New grammars indent from the parse tree once a body kind has been
        // checked against a real sample. Until then they stay flat rather
        // than guessing from braces.
        _ => &[],
    }
}

/// The suggested indent level (0 = no indent) for `row` in `document`'s
/// current tree — the number of the language's [`block_kinds`] that enclose
/// it, with one adjustment: if `row`'s first non-blank token is itself the
/// *last* token of the innermost enclosing body (a closing `}`, a `fi`, a
/// `done`, ...), that body doesn't count — the line closing a body sits at
/// the same level as the line that opened it, not one level deeper.
///
/// `source` must be the text `document` was last parsed from. Returns 0
/// before any [`Document::set_text`] call, or if `row` doesn't exist in
/// `source`.
#[must_use]
pub fn indent_level(document: &Document, source: &str, row: usize) -> usize {
    let Some(tree) = document.tree() else {
        return 0;
    };
    let Some(line) = source.lines().nth(row) else {
        return 0;
    };
    let column = line
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map_or(0, |(byte, _)| byte);
    let point = Point { row, column };
    let Some(anchor) = tree.root_node().descendant_for_point_range(point, point) else {
        return 0;
    };

    let language = document.language();
    let kinds = block_kinds(language);
    let depth = block_depth(anchor, kinds).saturating_sub(root_offset(language));

    let closes_its_own_body = anchor.child_count() == 0
        && anchor
            .parent()
            .is_some_and(|p| kinds.contains(&p.kind()) && p.end_byte() == anchor.end_byte());
    if closes_its_own_body {
        depth.saturating_sub(1)
    } else {
        depth
    }
}

/// Counts how many of `node` and its ancestors (root included) have a kind
/// in `kinds`.
fn block_depth(node: Node, kinds: &[&str]) -> usize {
    let mut depth = 0;
    let mut current = Some(node);
    while let Some(n) = current {
        if kinds.contains(&n.kind()) {
            depth += 1;
        }
        current = n.parent();
    }
    depth
}

/// How many of the outermost matches in [`block_kinds`] are the document's
/// own top-level container, not a real nesting step, and so must not count.
///
/// YAML and Markdown wrap a file's top-level content in the very same node
/// kind (`block_mapping`/`block_sequence`, `list`) used for content nested
/// *inside* a value — even though that outermost occurrence has no opening
/// delimiter and its direct children sit at column 0, same as the file
/// itself. Contrast JSON/HTML, where the outermost `object`/`element` has a
/// real `{`/`<div>` and its content is genuinely indented one level; there,
/// nothing needs subtracting. Every language's list was built from real
/// parse trees (see the module docs), so this offset is the one thing that
/// can't be read off a single tree — it only shows up by comparing a
/// top-level line against a nested one.
fn root_offset(language: crate::LanguageId) -> usize {
    matches!(
        language,
        crate::LanguageId::Yaml | crate::LanguageId::Markdown
    ) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, LanguageId};

    fn level(lang: LanguageId, source: &str, row: usize) -> usize {
        let mut doc = Document::new(lang);
        doc.set_text(source);
        indent_level(&doc, source, row)
    }

    #[test]
    fn top_level_code_has_no_indent() {
        assert_eq!(level(LanguageId::Rust, "fn main() {\n}\n", 0), 0);
    }

    #[test]
    fn a_line_inside_one_block_is_one_level_deep() {
        let source = "fn main() {\n    let x = 1;\n}\n";
        assert_eq!(level(LanguageId::Rust, source, 1), 1);
    }

    #[test]
    fn nesting_adds_a_level_per_block_even_when_header_and_brace_share_a_row() {
        // `if true {` opens both `if_expression` and `block` on the same
        // row — a naive "one level per ancestor row" count would conflate
        // them; `block_kinds` only lists `block`, so it doesn't.
        let source = "fn main() {\n    if true {\n        let x = 1;\n    }\n}\n";
        assert_eq!(level(LanguageId::Rust, source, 2), 2);
    }

    #[test]
    fn a_lone_closing_brace_dedents_to_the_line_that_opened_it() {
        let source = "fn main() {\n    if true {\n        let x = 1;\n    }\n}\n";
        assert_eq!(level(LanguageId::Rust, source, 3), 1);
        assert_eq!(level(LanguageId::Rust, source, 4), 0);
    }

    #[test]
    fn a_blank_line_takes_the_indent_of_the_body_it_sits_in() {
        let source = "fn main() {\n    if true {\n\n    }\n}\n";
        assert_eq!(level(LanguageId::Rust, source, 2), 2);
    }

    #[test]
    fn ignores_braces_written_inside_a_string_literal() {
        // A text-scanning heuristic would see the `{` in the string and
        // think it opened a new level; the tree knows it's just characters
        // inside a `string_literal` node, not a real brace.
        let source = "fn main() {\n    let s = \"{unbalanced\";\n    let y = 2;\n}\n";
        assert_eq!(level(LanguageId::Rust, source, 2), 1);
    }

    #[test]
    fn python_block_indents_without_any_brace_at_all() {
        let source = "if True:\n    x = 1\n    y = 2\n";
        assert_eq!(level(LanguageId::Python, source, 1), 1);
        assert_eq!(level(LanguageId::Python, source, 2), 1);
    }

    #[test]
    fn python_else_dedents_to_match_its_if_with_no_keyword_special_casing() {
        let source = "if True:\n    x = 1\nelse:\n    y = 2\n";
        assert_eq!(level(LanguageId::Python, source, 2), 0);
        assert_eq!(level(LanguageId::Python, source, 3), 1);
    }

    #[test]
    fn c_family_uses_compound_statement_not_block() {
        let source = "int main() {\n    if (1) {\n        int x = 1;\n    }\n}\n";
        assert_eq!(level(LanguageId::C, source, 2), 2);
        assert_eq!(level(LanguageId::C, source, 3), 1);
    }

    #[test]
    fn json_indents_nested_objects_and_arrays() {
        let source = "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n";
        assert_eq!(level(LanguageId::Json, source, 1), 1);
        assert_eq!(level(LanguageId::Json, source, 2), 2);
        assert_eq!(level(LanguageId::Json, source, 4), 1);
    }

    #[test]
    fn yaml_indents_nested_sequences() {
        let source = "a:\n  b: 1\n  c:\n    - 1\n    - 2\n";
        assert_eq!(level(LanguageId::Yaml, source, 1), 1);
        assert_eq!(level(LanguageId::Yaml, source, 3), 2);
    }

    #[test]
    fn bash_if_then_fi_indents_and_dedents() {
        let source = "if true; then\n  echo a\nfi\n";
        assert_eq!(level(LanguageId::Bash, source, 1), 1);
        assert_eq!(level(LanguageId::Bash, source, 2), 0);
    }

    #[test]
    fn bash_for_do_done_indents_and_dedents() {
        let source = "for x in 1 2; do\n  echo $x\ndone\n";
        assert_eq!(level(LanguageId::Bash, source, 1), 1);
        assert_eq!(level(LanguageId::Bash, source, 2), 0);
    }

    #[test]
    fn kotlin_and_swift_class_and_function_bodies_nest() {
        let kt = "class A {\n    fun f() {\n        val x = 1\n    }\n}\n";
        assert_eq!(level(LanguageId::Kotlin, kt, 2), 2);
        let swift = "class A {\n    func f() {\n        let x = 1\n    }\n}\n";
        assert_eq!(level(LanguageId::Swift, swift, 2), 2);
    }

    #[test]
    fn html_indents_nested_elements() {
        let source = "<div>\n  <p>\n    hi\n  </p>\n</div>\n";
        assert_eq!(level(LanguageId::Html, source, 2), 2);
    }

    #[test]
    fn yaml_top_level_keys_stay_unindented_despite_the_grammars_root_wrapper() {
        // `a:` and (were there a second top-level key) its siblings sit at
        // column 0 even though tree-sitter-yaml wraps the whole file in a
        // `block_mapping` node — the same kind used for genuinely nested
        // mapping values. Without `root_offset` this would over-count by 1
        // for every line.
        assert_eq!(level(LanguageId::Yaml, "a: 1\n", 0), 0);
    }

    #[test]
    fn markdown_nested_list_items_indent_one_level_not_two() {
        let source = "- a\n- b\n  - c\n  - d\n";
        assert_eq!(level(LanguageId::Markdown, source, 0), 0);
        assert_eq!(level(LanguageId::Markdown, source, 2), 1);
    }

    #[test]
    fn returns_zero_before_any_parse() {
        let doc = Document::new(LanguageId::Rust);
        assert_eq!(indent_level(&doc, "", 0), 0);
    }
}
