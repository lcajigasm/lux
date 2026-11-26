//! `EditorViewFrame::range_rects` is the geometry selections are drawn
//! with, so a find match over the same range lands on the same pixels.

use std::time::Duration;

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_render::TextLayoutContext;
use lux_ui::{EditorView, EditorViewMetrics, Viewport};

const AHEM_TTF: &[u8] = include_bytes!("fixtures/ahem.ttf");

fn view(word_wrap: bool) -> EditorView {
    let metrics = EditorViewMetrics {
        line_height: 20.0,
        font_size: 16.0,
        ligatures: false,
        word_wrap,
        column_rulers: Vec::new(),
        blink_interval: Duration::from_millis(500),
    };
    EditorView::new(TextLayoutContext::new(), AHEM_TTF, metrics).expect("ahem registers")
}

fn viewport(width: f32, scroll_y: f32) -> Viewport {
    Viewport {
        width,
        height: 200.0,
        scroll_x: 0.0,
        scroll_y,
    }
}

/// Selecting `start..end` and asking the frame for the range's rects must
/// give the same rectangles.
fn assert_same_as_selection(text: &str, start: usize, end: usize, word_wrap: bool, width: f32) {
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    let (a, b) = (
        Position::from_char(&buffer, start),
        Position::from_char(&buffer, end),
    );
    let cursors = MultiCursor::new(Selection::new(a, b));
    let mut view = view(word_wrap);
    let frame = view.build_frame(
        &buffer,
        &cursors,
        &viewport(width, 0.0),
        Duration::ZERO,
        None,
    );
    let ranges = frame.range_rects(&buffer, a, b, 20.0);
    assert!(!ranges.is_empty());
    assert_eq!(ranges, frame.selections);
}

#[test]
fn a_range_after_a_tab_matches_the_selection_rect() {
    assert_same_as_selection("\tlet counts = 1;\n", 5, 11, false, 800.0);
}

#[test]
fn a_range_after_wide_characters_matches_the_selection_rect() {
    assert_same_as_selection("日本語 counts\n", 4, 10, false, 800.0);
}

#[test]
fn a_range_across_a_soft_wrap_row_is_split_per_row() {
    let text = "aaaa bbbb cccc dddd eeee ffff gggg hhhh\n";
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    let (a, b) = (
        Position::from_char(&buffer, 2),
        Position::from_char(&buffer, 37),
    );
    let mut view = view(true);
    let frame = view.build_frame(
        &buffer,
        &MultiCursor::new(Selection::new(a, b)),
        &viewport(260.0, 0.0),
        Duration::ZERO,
        None,
    );
    let rects = frame.range_rects(&buffer, a, b, 20.0);
    assert!(rects.len() >= 2, "the line wraps, so the range spans rows");
    for pair in rects.windows(2) {
        assert!(pair[1].y > pair[0].y, "one rect per row, top to bottom");
    }
    assert!(rects[1..].iter().all(|r| r.x_start == 0.0));
    assert_eq!(rects, frame.selections);
}

#[test]
fn a_range_on_a_line_scrolled_off_screen_has_no_rects() {
    let text = (0..40).map(|i| format!("line {i}\n")).collect::<String>();
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    let mut view = view(false);
    let frame = view.build_frame(
        &buffer,
        &MultiCursor::new(Selection::cursor(Position::from_char(&buffer, 0))),
        &viewport(800.0, 400.0),
        Duration::ZERO,
        None,
    );
    let first = Position::from_char(&buffer, 0);
    assert!(frame.range_rects(&buffer, first, first, 20.0).is_empty());
    let visible = frame.lines[0].index;
    let start = Position::from_byte(&buffer, buffer.line_start_byte(visible).unwrap());
    let rects = frame.range_rects(&buffer, start, start.next_grapheme(&buffer), 20.0);
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].y, frame.lines[0].top);
}
