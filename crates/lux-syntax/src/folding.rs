//! Syntax-derived code folding (Phase 3 deliverable 4): foldable regions
//! come from the real parse tree's node spans, never from an indentation
//! heuristic (`LUX_PROMPT_MAESTRO.md` is explicit that v0's `{([:`
//! indentation heuristic is the mistake this replaces). Any node that
//! spans more than one source line — a function body, a struct, an array
//! literal, a multi-line comment, an HTML element — is foldable; nesting
//! them all naively over-reports (a `fn foo() { ... }` line starts both the
//! function item and its block, so both nodes begin on the same row), so
//! this keeps only the widest range per starting line.

use crate::Document;

/// One foldable region: the 0-indexed row it starts and ends on, plus the
/// exact byte range of the node it was derived from (for callers that want
/// the underlying text, e.g. to render a "N lines" collapsed placeholder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldRange {
    pub start_row: usize,
    pub end_row: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Every foldable range in `document`'s current tree, sorted by starting
/// row, at most one per starting row (the widest node that starts there —
/// see the module docs for why that dedup is necessary). The root node
/// itself (the whole file) is never included: folding "everything" isn't a
/// useful affordance.
#[must_use]
pub fn folding_ranges(document: &Document) -> Vec<FoldRange> {
    let Some(tree) = document.tree() else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut cursor = tree.root_node().walk();
    loop {
        let node = cursor.node();
        let start = node.start_position();
        let end = node.end_position();
        if end.row > start.row && node.parent().is_some() {
            ranges.push(FoldRange {
                start_row: start.row,
                end_row: end.row,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                ranges.sort_by_key(|r| (r.start_row, std::cmp::Reverse(r.end_row)));
                ranges.dedup_by_key(|r| r.start_row);
                return ranges;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, LanguageId};

    #[test]
    fn a_single_line_file_has_no_foldable_ranges() {
        let mut doc = Document::new(LanguageId::Json);
        doc.set_text(r#"{"a": 1}"#);
        assert!(folding_ranges(&doc).is_empty());
    }

    #[test]
    fn folds_a_multi_line_function_body_exactly_once_per_start_row() {
        let mut doc = Document::new(LanguageId::Rust);
        let source = "fn main() {\n    let x = 1;\n    println!(\"{x}\");\n}\n";
        doc.set_text(source);
        let ranges = folding_ranges(&doc);

        // "fn main() {" (row 0) starts both `function_item` and its
        // `block` — only the widest (the whole function) survives.
        let starting_at_zero: Vec<_> = ranges.iter().filter(|r| r.start_row == 0).collect();
        assert_eq!(starting_at_zero.len(), 1, "{ranges:?}");
        assert_eq!(starting_at_zero[0].end_row, 3);
    }

    #[test]
    fn folds_nested_regions_independently() {
        let mut doc = Document::new(LanguageId::Json);
        let source = "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n";
        doc.set_text(source);
        let ranges = folding_ranges(&doc);

        // The outer object (rows 0..5) and the inner array (rows 1..4)
        // both fold, independently, each starting on its own row.
        assert!(
            ranges.iter().any(|r| r.start_row == 0 && r.end_row == 5),
            "{ranges:?}"
        );
        assert!(
            ranges.iter().any(|r| r.start_row == 1 && r.end_row == 4),
            "{ranges:?}"
        );
    }

    #[test]
    fn never_includes_the_whole_file_as_its_own_fold() {
        let mut doc = Document::new(LanguageId::Rust);
        let source = "fn a() {\n}\nfn b() {\n}\n";
        doc.set_text(source);
        let ranges = folding_ranges(&doc);
        // The root `source_file` node spans rows 0..4 too, but must not
        // appear — only its two function children should.
        assert_eq!(ranges.len(), 2, "{ranges:?}");
    }

    #[test]
    fn ranges_are_sorted_by_starting_row_with_at_most_one_per_row() {
        let mut doc = Document::new(LanguageId::Rust);
        let source =
            "struct S {\n    a: i32,\n}\n\nfn f() {\n    let v = vec![\n        1,\n    ];\n}\n";
        doc.set_text(source);
        let ranges = folding_ranges(&doc);

        let rows: Vec<usize> = ranges.iter().map(|r| r.start_row).collect();
        let mut sorted_unique = rows.clone();
        sorted_unique.sort_unstable();
        sorted_unique.dedup();
        assert_eq!(
            rows, sorted_unique,
            "ranges must be sorted with unique starting rows"
        );
    }

    #[test]
    fn derives_from_the_tree_not_from_brace_counting() {
        // A heuristic that counts `{`/`[` on a line would treat this
        // single-line struct literal as non-foldable (it never spans
        // multiple lines) and would also get confused by the brace inside
        // the string. The tree-derived version handles both correctly:
        // no fold for the one-liner, and the string's brace isn't a node
        // this function even looks at.
        let mut doc = Document::new(LanguageId::Rust);
        doc.set_text("fn one_liner() { let s = \"{not a fold}\"; }\n");
        assert!(folding_ranges(&doc).is_empty());
    }
}
