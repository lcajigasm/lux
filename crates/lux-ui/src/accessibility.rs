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
pub const ROOT_ID: NodeId = NodeId(0);
pub const WINDOW_ID: NodeId = NodeId(u64::MAX);
pub const TAB_BAR_ID: NodeId = NodeId(u64::MAX - 1);
pub const STATUS_BAR_ID: NodeId = NodeId(u64::MAX - 2);
pub const ANNOUNCER_ID: NodeId = NodeId(u64::MAX - 3);
pub const FILE_TREE_ID: NodeId = NodeId(u64::MAX - 4);
pub const SEARCH_BAR_ID: NodeId = NodeId(u64::MAX - 5);
pub const TERMINAL_ID: NodeId = NodeId(u64::MAX - 6);
pub const SEARCH_INPUT_ID: NodeId = NodeId(u64::MAX - 7);
pub const SEARCH_MATCHES_ID: NodeId = NodeId(u64::MAX - 8);
pub const STATUS_TEXT_ID: NodeId = NodeId(u64::MAX - 9);
pub const TAB_ITEM_BASE_ID: u64 = u64::MAX - 1000;
pub const TREE_ITEM_BASE_ID: u64 = u64::MAX - 2000;

#[derive(Debug, Clone, Default)]
pub struct AccessibilityContext<'a> {
    pub window_title: &'a str,
    pub tabs: Vec<(&'a str, bool)>,
    pub status_telemetry: Option<String>,
    pub sidebar_items: Option<Vec<(&'a str, bool)>>,
    pub search_query: Option<String>,
    pub search_matches: Option<String>,
    pub terminal_title: Option<String>,
    pub announcement: Option<String>,
    pub focused_target: crate::focus::FocusTarget,
}

fn line_node_id(index: usize) -> NodeId {
    NodeId(index as u64 + 1)
}

fn build_editor_nodes(
    text_layout: &mut TextLayoutContext,
    family: &str,
    font_size: f32,
    buffer: &Buffer,
    cursors: &MultiCursor,
    nodes: &mut Vec<(NodeId, Node)>,
) -> Vec<NodeId> {
    let text = buffer.text();
    let index = LineIndex::build(&text);
    let line_count = index.len();
    let primary = cursors.primary();

    let mut child_ids = Vec::with_capacity(line_count);
    let mut anchor_position = None;
    let mut focus_position = None;

    for line_idx in 0..line_count {
        let line_start = index.line_byte_range(&text, line_idx).start;
        let line_text = &text[index.line_byte_range(&text, line_idx)];
        let id = line_node_id(line_idx);
        // Shaping every line is done on the UI thread, before the run loop
        // can answer a probe. A 400 by 420 file never returned from that
        // walk. Short lines keep real cluster geometry; a long line exposes
        // its text and a scalar index instead.
        let clusters = if line_text.len() <= LONG_LINE {
            let style = LineStyle {
                family,
                font_size,
                ligatures: false,
            };
            Some(text_layout.layout_line(&style, line_text, &[]).clusters())
        } else {
            None
        };
        let index_for = |byte| match &clusters {
            Some(clusters) => character_index_for_byte(clusters, byte),
            None => character_index_in_line(line_text, byte),
        };

        if primary.anchor.line == line_idx {
            anchor_position = Some(TextPosition {
                node: id,
                character_index: index_for(primary.anchor.byte - line_start),
            });
        }
        if primary.head.line == line_idx {
            focus_position = Some(TextPosition {
                node: id,
                character_index: index_for(primary.head.byte - line_start),
            });
        }

        let mut node = Node::new(Role::TextRun);
        node.set_text_direction(TextDirection::LeftToRight);
        node.set_value(line_text.to_string());
        if let Some(clusters) = &clusters {
            let (lengths, positions, widths) = cluster_arrays(clusters);
            node.set_character_lengths(lengths);
            node.set_character_positions(positions);
            node.set_character_widths(widths);
        }

        nodes.push((id, node));
        child_ids.push(id);
    }

    let mut root = Node::new(Role::MultilineTextInput);
    root.add_action(Action::SetTextSelection);
    // macOS drops TextRun nodes from the platform tree, so the text area's
    // own value is the string a keystroke probe and VoiceOver can read.
    root.set_value(text);
    root.set_children(child_ids.clone());
    if let (Some(anchor), Some(focus)) = (anchor_position, focus_position) {
        root.set_text_selection(TextSelection { anchor, focus });
    }
    nodes.push((ROOT_ID, root));

    child_ids
}

/// Builds an accessibility tree exposing the multiline text editor.
pub fn build_tree(
    text_layout: &mut TextLayoutContext,
    family: &str,
    font_size: f32,
    buffer: &Buffer,
    cursors: &MultiCursor,
) -> TreeUpdate {
    let mut nodes = Vec::new();
    let _ = build_editor_nodes(text_layout, family, font_size, buffer, cursors, &mut nodes);
    TreeUpdate {
        nodes,
        tree: Some(Tree::new(ROOT_ID)),
        tree_id: TreeId::ROOT,
        focus: ROOT_ID,
    }
}

/// Builds the complete AccessKit node hierarchy covering all application surfaces:
/// tabs, sidebar file tree, editor lines/cursors, search results, terminal, status bar,
/// and live region announcements.
pub fn build_full_window_tree(
    text_layout: &mut TextLayoutContext,
    family: &str,
    font_size: f32,
    buffer: &Buffer,
    cursors: &MultiCursor,
    ctx: &AccessibilityContext,
) -> TreeUpdate {
    let mut nodes = Vec::new();
    let _ = build_editor_nodes(text_layout, family, font_size, buffer, cursors, &mut nodes);

    let mut window = Node::new(Role::Window);
    window.set_label(ctx.window_title);
    let mut window_children = Vec::new();

    // 1. Tab Bar
    if !ctx.tabs.is_empty() {
        let mut tab_bar = Node::new(Role::TabList);
        let mut tab_children = Vec::new();
        for (i, &(title, is_active)) in ctx.tabs.iter().enumerate() {
            let tab_id = NodeId(TAB_ITEM_BASE_ID - i as u64);
            let mut tab_node = Node::new(Role::Tab);
            tab_node.set_label(title);
            tab_node.set_selected(is_active);
            nodes.push((tab_id, tab_node));
            tab_children.push(tab_id);
        }
        tab_bar.set_children(tab_children);
        nodes.push((TAB_BAR_ID, tab_bar));
        window_children.push(TAB_BAR_ID);
    }

    // 2. Sidebar File Tree
    if let Some(items) = &ctx.sidebar_items {
        let mut tree_node = Node::new(Role::Tree);
        tree_node.set_label("Project Files");
        let mut tree_children = Vec::new();
        for (i, &(name, is_dir)) in items.iter().enumerate() {
            let item_id = NodeId(TREE_ITEM_BASE_ID - i as u64);
            let mut item_node = Node::new(Role::TreeItem);
            item_node.set_label(name);
            if is_dir {
                item_node.set_expanded(true);
            }
            nodes.push((item_id, item_node));
            tree_children.push(item_id);
        }
        tree_node.set_children(tree_children);
        nodes.push((FILE_TREE_ID, tree_node));
        window_children.push(FILE_TREE_ID);
    }

    // 3. Multiline Editor
    window_children.push(ROOT_ID);

    // 4. In-Buffer Search Bar
    if let Some(query) = &ctx.search_query {
        let mut search_node = Node::new(Role::Search);
        search_node.set_label("Find in Buffer");
        let mut search_children = Vec::new();

        let mut input_node = Node::new(Role::TextInput);
        input_node.set_label("Search Query");
        input_node.set_value(query.clone());
        nodes.push((SEARCH_INPUT_ID, input_node));
        search_children.push(SEARCH_INPUT_ID);

        if let Some(matches_text) = &ctx.search_matches {
            let mut match_node = Node::new(Role::Status);
            match_node.set_label(matches_text.clone());
            nodes.push((SEARCH_MATCHES_ID, match_node));
            search_children.push(SEARCH_MATCHES_ID);
        }
        search_node.set_children(search_children);
        nodes.push((SEARCH_BAR_ID, search_node));
        window_children.push(SEARCH_BAR_ID);
    }

    // 5. Terminal Panel
    if let Some(term_title) = &ctx.terminal_title {
        let mut term_node = Node::new(Role::Terminal);
        term_node.set_label(term_title.clone());
        nodes.push((TERMINAL_ID, term_node));
        window_children.push(TERMINAL_ID);
    }

    // 6. Status Bar
    if let Some(status_text) = &ctx.status_telemetry {
        let mut status_node = Node::new(Role::Status);
        status_node.set_label("Status Bar");
        let mut label_node = Node::new(Role::Status);
        label_node.set_label(status_text.clone());
        nodes.push((STATUS_TEXT_ID, label_node));
        status_node.set_children(vec![STATUS_TEXT_ID]);
        nodes.push((STATUS_BAR_ID, status_node));
        window_children.push(STATUS_BAR_ID);
    }

    // 7. Live Announcer
    if let Some(announcement) = &ctx.announcement {
        let mut announcer = Node::new(Role::Status);
        announcer.set_live(accesskit::Live::Polite);
        announcer.set_label(announcement.clone());
        nodes.push((ANNOUNCER_ID, announcer));
        window_children.push(ANNOUNCER_ID);
    }

    window.set_children(window_children);
    nodes.push((WINDOW_ID, window));

    let focus_node = match ctx.focused_target {
        crate::focus::FocusTarget::Terminal => TERMINAL_ID,
        crate::focus::FocusTarget::FileTree => FILE_TREE_ID,
        crate::focus::FocusTarget::Search => SEARCH_INPUT_ID,
        _ => ROOT_ID,
    };

    TreeUpdate {
        nodes,
        tree: Some(Tree::new(WINDOW_ID)),
        tree_id: TreeId::ROOT,
        focus: focus_node,
    }
}

/// Lines longer than this are not shaped for the accessibility tree.
/// The tree is built on the UI thread.
const LONG_LINE: usize = 256;

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
fn character_index_in_line(line: &str, byte_in_line: usize) -> usize {
    let mut byte = byte_in_line.min(line.len());
    while byte > 0 && !line.is_char_boundary(byte) {
        byte -= 1;
    }
    line[..byte].chars().count()
}

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
        assert_eq!(root.value(), Some("one\ntwo\nthree"));
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
    fn a_long_line_keeps_a_scalar_caret_index_and_skips_cluster_arrays() {
        let (mut cx, family) = context_with_ahem();
        let mut text = "M".repeat(420);
        text.push('\n');
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let pos = Position::from_char(&buffer, 400);
        let cursors = MultiCursor::new(Selection::cursor(pos));

        let update = build_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors);

        let line = node(&update, line_node_id(0));
        assert_eq!(line.value().unwrap().chars().count(), 420);
        assert!(line.character_lengths().is_empty());
        let selection = node(&update, ROOT_ID).text_selection().unwrap();
        assert_eq!(selection.focus.character_index, 400);
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

    #[test]
    fn full_window_tree_hierarchy_and_announcements() {
        let (mut cx, family) = context_with_ahem();
        let buffer = Buffer::from_bytes(b"fn main() {}\n").unwrap();
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        let ctx = AccessibilityContext {
            window_title: "main.rs — Lux",
            tabs: vec![("main.rs", true), ("lib.rs", false)],
            status_telemetry: Some("Ln 1, Col 1  Rust  UTF-8  LF  Spaces: 4".into()),
            sidebar_items: Some(vec![("src", true), ("main.rs", false)]),
            search_query: Some("main".into()),
            search_matches: Some("1 of 1 matches".into()),
            terminal_title: Some("Terminal 1".into()),
            announcement: Some("Ln 1, Col 1".into()),
            focused_target: crate::focus::FocusTarget::Editor,
        };

        let update = build_full_window_tree(&mut cx, &family, FONT_SIZE, &buffer, &cursors, &ctx);

        assert_eq!(update.tree, Some(Tree::new(WINDOW_ID)));
        assert_eq!(update.focus, ROOT_ID);

        let window = node(&update, WINDOW_ID);
        assert_eq!(window.role(), Role::Window);
        assert_eq!(window.label(), Some("main.rs — Lux"));

        // Tab list
        let tab_bar = node(&update, TAB_BAR_ID);
        assert_eq!(tab_bar.role(), Role::TabList);
        assert_eq!(tab_bar.children().len(), 2);

        // Sidebar Tree
        let tree = node(&update, FILE_TREE_ID);
        assert_eq!(tree.role(), Role::Tree);
        assert_eq!(tree.children().len(), 2);

        // Editor
        let editor = node(&update, ROOT_ID);
        assert_eq!(editor.role(), Role::MultilineTextInput);

        // Search
        let search = node(&update, SEARCH_BAR_ID);
        assert_eq!(search.role(), Role::Search);

        // Terminal
        let term = node(&update, TERMINAL_ID);
        assert_eq!(term.role(), Role::Terminal);

        // Status bar
        let status = node(&update, STATUS_BAR_ID);
        assert_eq!(status.role(), Role::Status);

        // Announcer
        let announcer = node(&update, ANNOUNCER_ID);
        assert_eq!(announcer.role(), Role::Status);
        assert_eq!(announcer.live(), Some(accesskit::Live::Polite));
        assert_eq!(announcer.label(), Some("Ln 1, Col 1"));
    }
}
