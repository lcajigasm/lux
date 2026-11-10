//! Golden test for Phase 2 deliverable 4 ("lux-ui: vista de editor con
//! scroll virtualizado"). "Golden" here means checked against expected
//! values within a defined tolerance (`docs/PERFORMANCE.md`'s own phrase
//! for render golden tests), not a pixel-framebuffer comparison — there is
//! no `wgpu` quad/text pipeline yet to produce pixels (a later, separate
//! `lux-render` deliverable), so this validates the geometry/view-model
//! layer that pipeline will eventually consume, the same honest scoping
//! the glyph-atlas and text-layout receipts already used.
//!
//! Exercises, against a real 20-line buffer and a viewport that only fits
//! 4: virtualization (only visible lines are shaped), line numbers,
//! multi-cursor blink, multi-line selection rendering (both partial and
//! full-line spans), indentation guides, and column rulers — all six
//! sub-features this deliverable asks for.

use std::time::Duration;

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_render::TextLayoutContext;
use lux_ui::{EditorView, EditorViewMetrics, Viewport};

const AHEM_TTF: &[u8] = include_bytes!("fixtures/ahem.ttf");
/// Loose enough to absorb float rounding in real font-metric arithmetic,
/// tight enough that a wrong cluster/line lookup still fails the test.
const TOLERANCE: f32 = 0.5;

#[test]
fn mixed_line_endings_and_selected_primary_caret_use_rope_byte_offsets() {
    let buffer = Buffer::from_bytes("é\r猫\r\nabc\n".as_bytes()).unwrap();
    let selection = Selection::new(
        Position::from_char(&buffer, 6),
        Position::from_char(&buffer, 5),
    );
    let cursors = MultiCursor::from_cursors(vec![
        Selection::cursor(Position::document_end(&buffer)),
        selection,
    ]);
    let mut view = build_view();
    let frame = view.build_frame(
        &buffer,
        &cursors,
        &Viewport {
            width: 800.0,
            height: 120.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        },
        Duration::ZERO,
        None,
    );
    assert_eq!(frame.lines.len(), 4);
    let caret = frame.cursors.iter().find(|c| c.primary).unwrap();
    assert_eq!(caret.line, 2);
    assert!(caret.x.abs() < TOLERANCE);
    assert_eq!(frame.selections.len(), 1);
    assert!(frame.selections[0].x_end > frame.selections[0].x_start);
}

fn source_lines() -> Vec<&'static str> {
    vec![
        "fn main() {",                 // 0
        "    let x = 1;",              // 1
        "    if x > 0 {",              // 2
        "        return x;",           // 3
        "    }",                       // 4
        "}",                           // 5
        "",                            // 6
        "// filler line 07",           // 7
        "// filler line 08",           // 8
        "// filler line 09",           // 9
        "// filler line 10",           // 10
        "// filler line 11",           // 11
        "// filler line 12",           // 12
        "// filler line 13",           // 13
        "// off-screen cursor target", // 14
        "// filler line 15",           // 15
        "// filler line 16",           // 16
        "// filler line 17",           // 17
        "// filler line 18",           // 18
        "// filler line 19",           // 19
    ]
}

/// The char offset of `col` chars into `lines[line]` — every fixture line
/// is pure ASCII, so char count, byte count, and this test's own
/// hand-verified column arithmetic all agree.
fn char_offset(lines: &[&str], line: usize, col: usize) -> usize {
    let mut offset = 0;
    for l in &lines[..line] {
        offset += l.chars().count() + 1; // the '\n' joining it to the next line
    }
    offset + col
}

fn build_view() -> EditorView {
    let text_layout = TextLayoutContext::new();
    let metrics = EditorViewMetrics {
        line_height: 20.0,
        font_size: 16.0,
        ligatures: false,
        word_wrap: false,
        column_rulers: vec![10, 20],
        blink_interval: Duration::from_millis(500),
    };
    EditorView::new(text_layout, AHEM_TTF, metrics).expect("ahem.ttf must register")
}

#[test]
fn virtualized_scroll_shapes_only_the_visible_lines() {
    let lines = source_lines();
    let text = lines.join("\n");
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    assert_eq!(buffer.len_lines(), 20);

    let mut view = build_view();
    let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    // height=60/line_height=20 -> 3 lines fit exactly, +1 for a partial
    // line at the bottom edge = 4; scroll_y=40 -> first visible is line 2.
    let viewport = Viewport {
        width: 800.0,
        height: 60.0,
        scroll_x: 0.0,
        scroll_y: 40.0,
    };
    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);

    assert_eq!(frame.visible_line_range, 2..6);
    assert_eq!(frame.lines.len(), 4);
    assert!(
        frame.lines.len() < buffer.len_lines(),
        "virtualization must shape strictly fewer lines than the document has"
    );
    let shaped: Vec<usize> = frame.lines.iter().map(|l| l.index).collect();
    assert_eq!(shaped, vec![2, 3, 4, 5]);
}

#[test]
fn line_numbers_are_right_aligned_to_the_documents_digit_width() {
    let lines = source_lines();
    let text = lines.join("\n");
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    // 20 lines -> "20" is 2 digits -> every gutter number is 2 chars wide.
    let mut view = build_view();
    let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    let viewport = Viewport {
        width: 800.0,
        height: 60.0,
        scroll_x: 0.0,
        scroll_y: 40.0,
    };
    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);

    let numbers: Vec<&str> = frame.lines.iter().map(|l| l.number.as_str()).collect();
    assert_eq!(numbers, vec![" 3", " 4", " 5", " 6"]);
    assert!(frame.gutter_width > 0.0);
}

#[test]
fn multi_cursor_blinks_and_ignores_carets_scrolled_off_screen() {
    let lines = source_lines();
    let text = lines.join("\n");
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();

    let on_screen = Selection::cursor(Position::from_char(
        &buffer,
        char_offset(&lines, 3, 15), // right at the 'x' in "return x;"
    ));
    let off_screen = Selection::cursor(Position::from_char(
        &buffer,
        char_offset(&lines, 14, 0), // line 14 is nowhere near the viewport
    ));
    let cursors = MultiCursor::from_cursors(vec![on_screen, off_screen]);

    let mut view = build_view();
    let viewport = Viewport {
        width: 800.0,
        height: 60.0,
        scroll_x: 0.0,
        scroll_y: 40.0,
    };

    let blink_on = view.build_frame(
        &buffer,
        &cursors,
        &viewport,
        Duration::from_millis(200),
        None,
    );
    assert_eq!(
        blink_on.cursors.len(),
        1,
        "the off-screen cursor must not appear in the frame at all"
    );
    assert_eq!(blink_on.cursors[0].line, 3);
    assert!(blink_on.cursors[0].visible);
    assert!(blink_on.cursors[0].x > 0.0);

    let blink_off = view.build_frame(
        &buffer,
        &cursors,
        &viewport,
        Duration::from_millis(700),
        None,
    );
    assert_eq!(blink_off.cursors.len(), 1);
    assert!(
        !blink_off.cursors[0].visible,
        "700ms is past one 500ms blink interval, caret must be in its 'off' phase"
    );
}

#[test]
fn multi_line_selection_renders_partial_start_full_middle_partial_end() {
    let lines = source_lines();
    let text = lines.join("\n");
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();

    // Selects from column 4 of "    if x > 0 {" (line 2, right after the
    // indent) through column 4 of "    }" (line 4, right before the "}").
    let selection = Selection::new(
        Position::from_char(&buffer, char_offset(&lines, 2, 4)),
        Position::from_char(&buffer, char_offset(&lines, 4, 4)),
    );
    let cursors = MultiCursor::new(selection);

    let mut view = build_view();
    let viewport = Viewport {
        width: 800.0,
        height: 60.0,
        scroll_x: 0.0,
        scroll_y: 40.0,
    };
    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);

    assert_eq!(frame.selections.len(), 3, "must cover lines 2, 3 and 4");
    let find_line = |idx: usize| frame.lines.iter().find(|l| l.index == idx).unwrap();
    let find_rect = |idx: usize| frame.selections.iter().find(|s| s.line == idx).unwrap();

    let start_rect = find_rect(2);
    let line2 = find_line(2);
    assert!(
        start_rect.x_start > TOLERANCE,
        "selection starts after column 0 on the first line, x_start must be > 0"
    );
    assert!((start_rect.x_end - line2.layout.width()).abs() < TOLERANCE);

    let mid_rect = find_rect(3);
    let line3 = find_line(3);
    assert!(
        mid_rect.x_start.abs() < TOLERANCE,
        "a fully-selected middle line starts at 0"
    );
    assert!((mid_rect.x_end - line3.layout.width()).abs() < TOLERANCE);

    let end_rect = find_rect(4);
    let line4 = find_line(4);
    assert!(end_rect.x_start.abs() < TOLERANCE);
    assert!(
        end_rect.x_end < line4.layout.width() - TOLERANCE,
        "selection ends before the last column on the last line, x_end must be < full width"
    );
    assert!(end_rect.x_end > TOLERANCE);
}

#[test]
fn indent_guides_match_leading_whitespace_and_detected_indent_width() {
    let lines = source_lines();
    let text = lines.join("\n");
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    let indent_width = buffer.indent().width as usize;

    let mut view = build_view();
    let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    let viewport = Viewport {
        width: 800.0,
        height: 60.0,
        scroll_x: 0.0,
        scroll_y: 40.0,
    };
    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);

    let line2 = frame.lines.iter().find(|l| l.index == 2).unwrap(); // 4 leading spaces
    let line3 = frame.lines.iter().find(|l| l.index == 3).unwrap(); // 8 leading spaces
    assert_eq!(line2.indent_guides.len(), 4 / indent_width);
    assert_eq!(line3.indent_guides.len(), 8 / indent_width);
    assert!(
        line3.indent_guides.len() > line2.indent_guides.len(),
        "the more deeply indented line must have more guides"
    );
    for pair in line3.indent_guides.windows(2) {
        assert!(
            pair[1] > pair[0],
            "indent guides must be strictly left-to-right"
        );
    }
}

#[test]
fn no_leading_whitespace_means_no_indent_guides() {
    let lines = source_lines();
    let text = lines.join("\n");
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();

    let mut view = build_view();
    let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    // Scroll so line 0 ("fn main() {", no indent) is visible.
    let viewport = Viewport {
        width: 800.0,
        height: 60.0,
        scroll_x: 0.0,
        scroll_y: 0.0,
    };
    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);
    let line0 = frame.lines.iter().find(|l| l.index == 0).unwrap();
    assert!(line0.indent_guides.is_empty());
}

#[test]
fn column_rulers_are_positioned_in_increasing_column_order() {
    let lines = source_lines();
    let text = lines.join("\n");
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();

    let mut view = build_view();
    let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    let viewport = Viewport {
        width: 800.0,
        height: 60.0,
        scroll_x: 0.0,
        scroll_y: 40.0,
    };
    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, None);

    // build_view() configures rulers at columns [10, 20].
    assert_eq!(frame.column_ruler_x.len(), 2);
    assert!(frame.column_ruler_x[0] > 0.0);
    assert!(frame.column_ruler_x[1] > frame.column_ruler_x[0]);
    // Monospace: column 20 must sit at roughly twice column 10's offset.
    let ratio = frame.column_ruler_x[1] / frame.column_ruler_x[0];
    assert!((ratio - 2.0).abs() < 0.05, "expected ~2x, got {ratio}");
}

#[test]
fn build_frame_with_folds_skips_collapsed_lines() {
    let text = "fn foo() {\n    line 1\n    line 2\n    line 3\n}\nline 5\n";
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    let mut view = build_view();
    let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    let viewport = Viewport {
        width: 800.0,
        height: 200.0,
        scroll_x: 0.0,
        scroll_y: 0.0,
    };

    let mut folds = lux_core::folding::FoldState::new();
    folds.set_foldable_ranges(vec![lux_core::folding::FoldRange::new(0, 4)]);
    folds.fold(0);

    let frame = view.build_frame(&buffer, &cursors, &viewport, Duration::ZERO, Some(&folds));
    let indices: Vec<usize> = frame.lines.iter().map(|l| l.index).collect();
    // Lines 1..=4 are folded away; line 0 is folded header, followed by line 5
    assert_eq!(indices, vec![0, 5, 6]);
    assert!(frame.lines[0].is_folded);
    assert_eq!(frame.lines[0].visual_index, 0);
    assert_eq!(frame.lines[1].visual_index, 1);
}

#[test]
fn an_unwrapped_long_line_shapes_only_the_visible_columns() {
    let line = "M".repeat(420);
    let buffer = Buffer::from_bytes(format!("{line}\n").as_bytes()).unwrap();
    let mut view = build_view();
    let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    let frame = view.build_frame(
        &buffer,
        &cursors,
        &Viewport {
            width: 800.0,
            height: 40.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        },
        Duration::ZERO,
        None,
    );
    let clusters = frame.lines[0].layout.clusters();
    assert!(
        clusters.len() < 200,
        "shaped {} columns of a 420-column line",
        clusters.len()
    );
    assert!(clusters.len() > 10, "shaped {}", clusters.len());
    assert_eq!(clusters[0].byte_range.start, 0);
}
