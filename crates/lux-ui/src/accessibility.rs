//! Builds an `accesskit` tree exposing the editor as a real multiline text
//! input: real content, real cursor position, and real selection — Phase 2
//! deliverable 6 (`docs/adr/0005-accessibility-from-phase-2.md`).
//!
//! The tree shape follows `accesskit`'s own text model, the same one
//! `parley`'s own (unused here — see below) `LayoutAccessibility` builds:
//! one `Role::TextRun` child node per line, each carrying its text plus
//! per-character byte lengths/x-positions/advances, and a
//! `Role::MultilineTextInput` root referencing those children plus a
//! `TextSelection` naming which line and character index the primary
//! cursor sits at. This is what lets a screen reader read line-by-line,
//! report caret position, and read back a selection.
//!
//! Deliberately not `parley::layout::accessibility::LayoutAccessibility`:
//! that API needs the underlying `parley::Layout`, which
//! `lux_render::LineLayout` doesn't expose (and adding that export would be
//! a change to `lux-render`, a different, already-closed deliverable, out
//! of this one's scope). This module instead builds the same shape
//! directly from `lux_render::text_layout::ClusterGeometry`, which is
//! already public and already exactly "one entry per smallest selectable
//! unit" — the same definition `accesskit` uses for a "character."
//!
//! Reuses `crate::editor_view::line_index::LineIndex` (bumped from private
//! to `pub(crate)` for this) rather than re-deriving line byte ranges from
//! `Buffer::text()` a second, separately-maintained way.

use accesskit::{
    Action, Node, NodeId, Role, TextDirection, TextPosition, TextSelection, Tree, TreeId,
    TreeUpdate,
};

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_render::{ClusterGeometry, LineStyle, TextLayoutContext};

use crate::editor_view::line_index::LineIndex;

/// The editor's own node id: the one `Role::MultilineTextInput` control
/// this whole tree exposes. Stable for the process's lifetime — there is
/// only ever one editor view per tree in this deliverable's scope.
pub const ROOT_ID: NodeId = NodeId(0);

/// Line `index`'s `Role::TextRun` node id. Deterministic and index-based,
/// not identity-tracked across edits (unlike `parley`'s own span-reuse
/// bookkeeping) — simple and correct for a first wiring pass; only
/// rebuilding the full tree every call, rather than diffing which lines
/// actually changed, is the real cost of that simplicity, worth revisiting
/// once a large-document performance budget applies here.
fn line_node_id(index: usize) -> NodeId {
    NodeId(index as u64 + 1)
}

/// Builds a full accessibility tree for `buffer`'s current content and
/// `cursors`' primary selection, shaping every line with `text_layout`
/// (the same shaping `lux_render::text_layout` already does for rendering,
/// just applied to the whole document here instead of a virtualized
/// visible slice — a screen reader needs to navigate the full document,
/// not only what's currently scrolled into view).
///
/// Only the *primary* cursor/selection is exposed: `accesskit::TextSelection`
/// models exactly one anchor/focus pair, matching how every mainstream
/// screen reader's text-editing model works today — multi-cursor has no
/// accessibility-API equivalent to expose the other cursors as, so they are
/// silently not represented here, not lost from the editor itself.
pub fn build_tree(
    text_layout: &mut TextLayoutContext,
    family: &str,
    font_size: f32,
    buffer: &Buffer,
    cursors: &MultiCursor,
) -> TreeUpdate {
    let text = buffer.text();
    let index = LineIndex::build(&text);
    let line_count = index.len();
    let primary = cursors.primary();

    let mut nodes = Vec::with_capacity(line_count + 1);
    let mut child_ids = Vec::with_capacity(line_count);
    let mut anchor_position = None;
    let mut focus_position = None;

    for line_idx in 0..line_count {
        let line_start = index.line_byte_range(&text, line_idx).start;
        let line_text = &text[index.line_byte_range(&text, line_idx)];
        let style = LineStyle {
            family,
            font_size,
            ligatures: false,
        };
        let layout = text_layout.layout_line(&style, line_text, &[]);
        let clusters = layout.clusters();
        let id = line_node_id(line_idx);

        if primary.anchor.line == line_idx {
            anchor_position = Some(TextPosition {
                node: id,
                character_index: character_index_for_byte(
                    &clusters,
                    primary.anchor.byte - line_start,
                ),
            });
        }
        if primary.head.line == line_idx {
            focus_position = Some(TextPosition {
                node: id,
                character_index: character_index_for_byte(
                    &clusters,
                    primary.head.byte - line_start,
                ),
            });
        }

        let mut node = Node::new(Role::TextRun);
        node.set_text_direction(TextDirection::LeftToRight);
        node.set_value(line_text.to_string());
        let (lengths, positions, widths) = cluster_arrays(&clusters);
        node.set_character_lengths(lengths);
        node.set_character_positions(positions);
        node.set_character_widths(widths);

        nodes.push((id, node));
        child_ids.push(id);
    }

    let mut root = Node::new(Role::MultilineTextInput);
    root.add_action(Action::SetTextSelection);
    root.set_children(child_ids);
    if let (Some(anchor), Some(focus)) = (anchor_position, focus_position) {
        root.set_text_selection(TextSelection { anchor, focus });
    }
    nodes.push((ROOT_ID, root));

    TreeUpdate {
        nodes,
        tree: Some(Tree::new(ROOT_ID)),
        tree_id: TreeId::ROOT,
        // This editor is the only focusable control in this deliverable's
        // scope, so it's always the answer to "what has focus when the
        // window does" — never a placeholder, just a real one-widget tree.
        focus: ROOT_ID,
    }
}

/// `character_lengths`/`character_positions`/`character_widths`, one entry
/// per cluster — `ClusterGeometry` is already exactly accesskit's
/// definition of a "character": the smallest selectable unit.
fn cluster_arrays(clusters: &[ClusterGeometry]) -> (Vec<u8>, Vec<f32>, Vec<f32>) {
    let lengths = clusters
        .iter()
        .map(|c| (c.byte_range.end - c.byte_range.start) as u8)
        .collect();
    let positions = clusters.iter().map(|c| c.x).collect();
    let widths = clusters.iter().map(|c| c.advance).collect();
    (lengths, positions, widths)
}

/// Maps a byte offset within one line to an index into that line's cluster
/// list — `accesskit::TextPosition::character_index`'s own definition: an
/// index into `Node::character_lengths`, not a byte offset. Returns
/// `clusters.len()` (one past the last character) for a position at or
/// past the end of the line, matching `TextPosition`'s documented meaning
/// for "the length of that slice."
fn character_index_for_byte(clusters: &[ClusterGeometry], byte_in_line: usize) -> usize {
    clusters
        .iter()
        .position(|c| c.byte_range.start == byte_in_line)
        .unwrap_or(clusters.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_core::position::Position;
    use lux_core::selection::Selection;

    const AHEM_TTF: &[u8] = include_bytes!("../tests/fixtures/ahem.ttf");
    const FONT_SIZE: f32 = 16.0;

    fn context_with_ahem() -> (TextLayoutContext, String) {
        let mut cx = TextLayoutContext::new();
        let family = cx.register_font(AHEM_TTF).expect("ahem.ttf must register");
        (cx, family)
    }

    fn node(update: &TreeUpdate, id: NodeId) -> &Node {
        &update
            .nodes
            .iter()
            .find(|(node_id, _)| *node_id == id)
            .unwrap()
            .1
    }

    #[test]
    fn builds_one_text_run_per_line_matching_the_roots_children_in_order() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"one\ntwo\nthree").unwrap();
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        assert_eq!(buffer.len_lines(), 3);
        assert_eq!(update.nodes.len(), 4, "3 lines + 1 root");
        let root = node(&update, ROOT_ID);
        assert_eq!(root.role(), Role::MultilineTextInput);
        assert_eq!(
            root.children(),
            &[line_node_id(0), line_node_id(1), line_node_id(2)]
        );
        for id in [line_node_id(0), line_node_id(1), line_node_id(2)] {
            assert_eq!(node(&update, id).role(), Role::TextRun);
        }
    }

    #[test]
    fn text_run_value_and_character_arrays_describe_the_lines_real_content() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"hello\nworld").unwrap();
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        let line0 = node(&update, line_node_id(0));
        assert_eq!(line0.value(), Some("hello"));
        let lengths = line0.character_lengths(); // `&[u8]`, not optional
        assert_eq!(lengths.len(), 5, "one entry per character in \"hello\"");
        assert_eq!(
            lengths.iter().map(|&l| l as usize).sum::<usize>(),
            "hello".len(),
            "character_lengths must sum to the value's byte length"
        );
        assert_eq!(line0.character_positions().unwrap().len(), 5);
        assert_eq!(line0.character_widths().unwrap().len(), 5);
    }

    #[test]
    fn empty_line_produces_an_empty_but_valid_text_run() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"a\n\nb").unwrap();
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        let blank = node(&update, line_node_id(1));
        assert_eq!(blank.value(), Some(""));
        assert_eq!(blank.character_lengths().len(), 0);
    }

    #[test]
    fn a_collapsed_cursor_maps_to_the_right_line_and_character_index() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"abcde\nfg").unwrap();
        // Position the caret right before 'c' (char index 2) on line 0.
        let pos = Position::from_char(&buffer, 2);
        let cursors = MultiCursor::new(Selection::cursor(pos));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        let root = node(&update, ROOT_ID);
        let selection = root.text_selection().unwrap();
        assert_eq!(
            selection.anchor, selection.focus,
            "a collapsed selection is just a caret"
        );
        assert_eq!(selection.focus.node, line_node_id(0));
        assert_eq!(selection.focus.character_index, 2);
    }

    #[test]
    fn caret_at_end_of_line_uses_the_character_count_as_its_index() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"abc\ndef").unwrap();
        let pos = Position::from_char(&buffer, 3); // right after 'c', end of line 0
        let cursors = MultiCursor::new(Selection::cursor(pos));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        let selection = node(&update, ROOT_ID).text_selection().unwrap();
        assert_eq!(selection.focus.node, line_node_id(0));
        assert_eq!(
            selection.focus.character_index, 3,
            "3 characters on line 0 (\"abc\")"
        );
    }

    #[test]
    fn a_multi_line_selection_has_anchor_and_focus_on_different_lines() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"abc\ndef\nghi").unwrap();
        let anchor = Position::from_char(&buffer, 1); // line 0, char 1
        let head = Position::from_char(&buffer, 9); // line 2, char 1 ("ghi" starts at char 8)
        let cursors = MultiCursor::new(Selection::new(anchor, head));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        let selection = node(&update, ROOT_ID).text_selection().unwrap();
        assert_eq!(selection.anchor.node, line_node_id(0));
        assert_eq!(selection.anchor.character_index, 1);
        assert_eq!(selection.focus.node, line_node_id(2));
        assert_eq!(selection.focus.character_index, 1);
    }

    #[test]
    fn only_the_primary_selection_is_exposed_when_multiple_cursors_exist() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"abcdef\nghijkl").unwrap();
        let first = Selection::cursor(Position::from_char(&buffer, 1));
        let second = Selection::cursor(Position::from_char(&buffer, 9)); // last-added -> primary
        let cursors = MultiCursor::from_cursors(vec![first, second]);

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        let selection = node(&update, ROOT_ID).text_selection().unwrap();
        assert_eq!(
            cursors.count(),
            2,
            "both cursors still exist in the editor's own state"
        );
        assert_eq!(
            selection.focus,
            TextPosition {
                node: line_node_id(1),
                character_index: 2,
            },
            "only the primary cursor's position is exposed to accessibility"
        );
    }

    #[test]
    fn the_root_is_always_the_focused_node_of_its_own_tree() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"x").unwrap();
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        assert_eq!(update.focus, ROOT_ID);
        assert_eq!(update.tree, Some(Tree::new(ROOT_ID)));
        assert_eq!(update.tree_id, TreeId::ROOT);
    }
}
