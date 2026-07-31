//! Bracket pair colorization and cursor-pair highlighting (Phase 3
//! deliverable 3). Pairs are found by walking the real parse tree's leaf
//! tokens, not by scanning source text for `(`/`)` characters — a plain
//! text scan would pair brackets that appear inside a string or comment
//! (e.g. `"(unbalanced"`), which the tree already knows aren't brackets at
//! all (they're children of a `string_literal`/`comment` node, not
//! anonymous bracket tokens).

use crate::Document;
use std::ops::Range;

/// The three ASCII bracket kinds tracked for pairing/coloring. Every
/// vendored grammar represents `(`/`)`/`[`/`]`/`{`/`}` as anonymous leaf
/// nodes whose `kind()` is the literal character itself — this list is
/// therefore language-agnostic, no per-grammar configuration needed.
const KINDS: [(&str, &str); 3] = [("(", ")"), ("[", "]"), ("{", "}")];

fn kind_index(node_kind: &str) -> Option<(usize, bool)> {
    KINDS.iter().enumerate().find_map(|(i, (open, close))| {
        if node_kind == *open {
            Some((i, true))
        } else if node_kind == *close {
            Some((i, false))
        } else {
            None
        }
    })
}

/// One matched bracket pair: the byte ranges of its opening and closing
/// tokens, and its nesting depth (0 = outermost).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPair {
    pub open: Range<usize>,
    pub close: Range<usize>,
    pub depth: usize,
}

/// Finds every matched bracket pair in `document`'s current tree, in
/// document order, by walking every leaf token and matching openers to
/// closers of the same kind with a stack — exactly how a text editor
/// tracks nesting depth while typing. An opener left on the stack at the
/// end (unbalanced/incomplete code) contributes no pair, which is the
/// correct, safe behavior: nothing to color yet.
#[must_use]
pub fn bracket_pairs(document: &Document) -> Vec<BracketPair> {
    let Some(tree) = document.tree() else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    let mut stack: Vec<(usize, Range<usize>)> = Vec::new();
    let mut cursor = tree.root_node().walk();
    loop {
        let node = cursor.node();
        // `is_missing()` excludes tree-sitter's own error-recovery nodes: on
        // unbalanced input it synthesizes a zero-width "MISSING )" (etc.)
        // leaf with the right `kind()` so the tree stays well-formed for
        // the rest of the parse — treating that as a real closing token
        // would fabricate a pair for a bracket that was never actually
        // typed.
        if node.child_count() == 0
            && !node.is_missing()
            && let Some((idx, is_open)) = kind_index(node.kind())
        {
            let range = node.byte_range();
            if is_open {
                stack.push((idx, range));
            } else if stack.last().is_some_and(|(top, _)| *top == idx) {
                let (_, open) = stack.pop().expect("checked by is_some_and above");
                pairs.push(BracketPair {
                    open,
                    close: range,
                    depth: stack.len(),
                });
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                pairs.sort_by_key(|p| p.open.start);
                return pairs;
            }
        }
    }
}

/// The innermost pair whose opening or closing token contains
/// `byte_offset` — "the pair under the cursor" to highlight. `pairs` must
/// be sorted by `open.start` (what [`bracket_pairs`] returns); when
/// several pairs' delimiters both contain the offset (impossible for the
/// delimiters themselves, since they don't overlap) this simply returns
/// the first match.
#[must_use]
pub fn pair_at(pairs: &[BracketPair], byte_offset: usize) -> Option<&BracketPair> {
    pairs
        .iter()
        .find(|p| p.open.contains(&byte_offset) || p.close.contains(&byte_offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, LanguageId};

    #[test]
    fn pairs_nested_brackets_of_the_same_kind() {
        let mut doc = Document::new(LanguageId::Json);
        let source = "[[1, 2], [3]]";
        doc.set_text(source);
        let pairs = bracket_pairs(&doc);

        assert_eq!(pairs.len(), 3, "{pairs:?}");
        let depths: Vec<usize> = pairs.iter().map(|p| p.depth).collect();
        assert!(
            depths.contains(&0),
            "outermost pair must be depth 0: {depths:?}"
        );
        assert!(
            depths.contains(&1),
            "the two inner pairs must be depth 1: {depths:?}"
        );
    }

    #[test]
    fn distinguishes_bracket_kinds_so_mismatched_types_never_pair() {
        let mut doc = Document::new(LanguageId::Rust);
        let source = "fn f(a: [i32; 3]) { a[0] }";
        doc.set_text(source);
        let pairs = bracket_pairs(&doc);

        for pair in &pairs {
            let opener = &source[pair.open.clone()];
            let closer = &source[pair.close.clone()];
            let expected_closer = KINDS.iter().find(|(o, _)| *o == opener).map(|(_, c)| *c);
            assert_eq!(
                Some(closer),
                expected_closer,
                "opener {opener:?} paired with {closer:?}"
            );
        }
    }

    #[test]
    fn ignores_brackets_that_are_only_characters_inside_a_string() {
        let mut doc = Document::new(LanguageId::Rust);
        // The only *real* bracket pair here is the function's own `()`;
        // the parenthesis inside the string literal is not a bracket node.
        let source = r#"fn f() { let s = "(unbalanced"; }"#;
        doc.set_text(source);
        let pairs = bracket_pairs(&doc);

        assert_eq!(pairs.len(), 2, "{pairs:?}"); // f( ) and the block { }
        for pair in &pairs {
            assert!(!source[pair.open.clone()].is_empty());
        }
    }

    #[test]
    fn an_unbalanced_opener_contributes_no_pair() {
        let mut doc = Document::new(LanguageId::Json);
        doc.set_text("[1, 2");
        assert!(bracket_pairs(&doc).is_empty());
    }

    #[test]
    fn pair_at_finds_the_pair_under_the_cursor() {
        let mut doc = Document::new(LanguageId::Json);
        let source = "[1, [2]]";
        doc.set_text(source);
        let pairs = bracket_pairs(&doc);

        let outer_open = source.find('[').unwrap();
        let found = pair_at(&pairs, outer_open).expect("cursor is on the outer '['");
        assert_eq!(found.depth, 0);

        let inner_open = source.rfind('[').unwrap();
        let found = pair_at(&pairs, inner_open).expect("cursor is on the inner '['");
        assert_eq!(found.depth, 1);
    }

    #[test]
    fn pair_at_is_none_between_brackets() {
        let mut doc = Document::new(LanguageId::Json);
        let source = "[1, 2]";
        doc.set_text(source);
        let pairs = bracket_pairs(&doc);
        assert!(pair_at(&pairs, source.find('1').unwrap()).is_none());
    }

    #[test]
    fn theme_bracket_colors_cycle_by_depth() {
        let theme = &crate::theme::LUX_DARK;
        let first = theme.color_for_bracket_depth(0);
        let cycled = theme.color_for_bracket_depth(6); // wraps for a 6-color palette
        assert_eq!(first, cycled);
    }
}
