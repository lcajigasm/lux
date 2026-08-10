//! Minimap (`LUX_PROMPT_MAESTRO.md` Phase 3 deliverable 10, optional): a
//! condensed, whole-document overview with a "you are here" viewport
//! indicator and click-to-scroll. Like `EditorView`, this produces the
//! geometry a render pass would draw, not pixels — the quad pipeline
//! doesn't exist yet (`docs/ARCHITECTURE.md`). Unlike `EditorView`, it
//! never shapes glyphs: a minimap showing a real glyph per character
//! across a 50k-line document would shape the entire document every
//! frame, defeating Phase 2 deliverable 4's whole point (virtualized,
//! visible-range-only shaping). Each source line instead reduces to a
//! density bar — how far its content starts (indent) and how long it
//! runs — the same real technique production minimaps use (a blurred
//! approximation of text, not the text itself). When the document has
//! more lines than the minimap has pixel rows for, several source lines
//! collapse into one row, represented by the densest line in that group
//! (the one with the most content), so a long line is never hidden by
//! averaging it away with its blank neighbors.

use crate::editor_view::line_index::LineIndex;
use lux_core::buffer::Buffer;
use std::ops::Range;

/// Whether the minimap is shown — the "opcional" in the deliverable name.
/// On by default, same as every editor that has one; the user can turn it
/// off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Minimap {
    pub enabled: bool,
}

impl Default for Minimap {
    fn default() -> Self {
        Minimap { enabled: true }
    }
}

impl Minimap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

/// One minimap row: the densest source line among the (possibly several)
/// document lines it represents, as an indent offset and a content length,
/// both in columns (characters, not bytes; tabs count as one column each,
/// same simplification `editor_view::indent_guide_x` already documents).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimapRow {
    pub line: usize,
    pub indent_columns: usize,
    pub content_columns: usize,
}

/// The main editor's visible line range, translated into minimap rows —
/// the "you are here" overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportIndicator {
    pub rows: Range<usize>,
}

/// Everything a minimap render pass needs for one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapFrame {
    pub rows: Vec<MinimapRow>,
    pub viewport: ViewportIndicator,
}

/// Builds one minimap frame for `buffer`, or `None` if the minimap is off
/// — matching golden rule #1: a disabled minimap draws nothing, not an
/// empty one. `visible_line_range` is the main editor's current visible
/// range (`EditorView`'s own `visible_line_range`); `available_rows` is
/// how many pixel rows the minimap widget has, e.g. `height / row_height`.
pub fn build_frame(
    minimap: &Minimap,
    buffer: &Buffer,
    visible_line_range: Range<usize>,
    available_rows: usize,
) -> Option<MinimapFrame> {
    if !minimap.enabled {
        return None;
    }
    let text = buffer.text();
    let index = LineIndex::build(&text);
    let total_lines = index.len();
    let per_row = lines_per_row(total_lines, available_rows);

    let rows = (0..row_count(total_lines, available_rows))
        .map(|row| densest_row(&index, &text, row, per_row))
        .collect();

    let viewport_rows = if visible_line_range.is_empty() {
        0..0
    } else {
        let first_row = row_for_line(total_lines, available_rows, visible_line_range.start);
        let last_row = row_for_line(total_lines, available_rows, visible_line_range.end - 1);
        first_row..last_row + 1
    };

    Some(MinimapFrame {
        rows,
        viewport: ViewportIndicator {
            rows: viewport_rows,
        },
    })
}

/// Maps a click at minimap row `row` to the document line it represents —
/// the navigation a minimap's whole interactive point is. Clamped to the
/// last real line for an out-of-range row (e.g. a click below the last
/// populated row, in the widget's unused trailing space).
pub fn line_for_row(total_lines: usize, available_rows: usize, row: usize) -> usize {
    if total_lines == 0 {
        return 0;
    }
    line_range_for_row(total_lines, available_rows, row)
        .start
        .min(total_lines - 1)
}

/// How many source lines each minimap row represents: 1 when the document
/// fits (one row per line), more when it's downsampled. Never 0, so every
/// row is guaranteed to cover at least one line.
fn lines_per_row(total_lines: usize, available_rows: usize) -> usize {
    if available_rows == 0 {
        return total_lines.max(1);
    }
    total_lines.div_ceil(available_rows).max(1)
}

/// How many rows the minimap actually needs — `<= available_rows`, since a
/// document that doesn't divide evenly by `lines_per_row` simply uses
/// fewer, fully-populated rows rather than padding out to `available_rows`
/// with empty ones.
fn row_count(total_lines: usize, available_rows: usize) -> usize {
    total_lines.div_ceil(lines_per_row(total_lines, available_rows))
}

fn line_range_for_row(total_lines: usize, available_rows: usize, row: usize) -> Range<usize> {
    let per_row = lines_per_row(total_lines, available_rows);
    let start = (row * per_row).min(total_lines);
    let end = (start + per_row).min(total_lines);
    start..end
}

fn row_for_line(total_lines: usize, available_rows: usize, line: usize) -> usize {
    line / lines_per_row(total_lines, available_rows)
}

fn densest_row(index: &LineIndex, text: &str, row: usize, per_row: usize) -> MinimapRow {
    let start = row * per_row;
    let end = (start + per_row).min(index.len());
    let mut best = MinimapRow {
        line: start,
        indent_columns: 0,
        content_columns: 0,
    };
    for line in start..end {
        let line_text = &text[index.line_byte_range(text, line)];
        let leading = line_text
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .count();
        let content_columns = line_text.chars().count() - leading;
        if content_columns > best.content_columns {
            best = MinimapRow {
                line,
                indent_columns: leading,
                content_columns,
            };
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_defaults_to_enabled() {
        assert!(Minimap::new().enabled);
    }

    #[test]
    fn toggle_flips_enabled() {
        let mut minimap = Minimap::new();
        minimap.toggle();
        assert!(!minimap.enabled);
        minimap.toggle();
        assert!(minimap.enabled);
    }

    #[test]
    fn build_frame_returns_none_when_disabled() {
        let buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
        let minimap = Minimap { enabled: false };
        assert_eq!(build_frame(&minimap, &buffer, 0..2, 100), None);
    }

    #[test]
    fn build_frame_is_one_to_one_when_the_document_fits() {
        let buffer = Buffer::from_bytes(b"one\n  two\nthree\n").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 0..4, 100).unwrap();
        // "one\n  two\nthree\n" is 4 lines (ropey counts the trailing
        // newline as one more, empty line — same convention as LineIndex).
        assert_eq!(frame.rows.len(), 4);
        assert_eq!(
            frame.rows[0],
            MinimapRow {
                line: 0,
                indent_columns: 0,
                content_columns: 3
            }
        );
        assert_eq!(
            frame.rows[1],
            MinimapRow {
                line: 1,
                indent_columns: 2,
                content_columns: 3
            }
        );
        assert_eq!(
            frame.rows[3],
            MinimapRow {
                line: 3,
                indent_columns: 0,
                content_columns: 0
            }
        );
    }

    #[test]
    fn build_frame_downsamples_and_picks_the_densest_line_per_row() {
        // 6 lines, 3 rows available -> 2 lines per row. Row 0 covers lines
        // 0-1 ("a" and "much longer line"); the longer one should win.
        let buffer = Buffer::from_bytes(b"a\nmuch longer line\nb\nc\nd\ne\n").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 0..1, 3).unwrap();
        assert_eq!(frame.rows.len(), 3);
        assert_eq!(frame.rows[0].line, 1);
        assert_eq!(frame.rows[0].content_columns, "much longer line".len());
    }

    #[test]
    fn build_frame_never_pads_with_empty_trailing_rows() {
        // 5 lines into 3 available rows: 2 lines/row needs ceil(5/2) = 3
        // rows exactly (2, 2, 1) — no row is ever empty.
        let buffer = Buffer::from_bytes(b"1\n2\n3\n4\n5").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 0..1, 3).unwrap();
        assert_eq!(frame.rows.len(), 3);

        // And when the document is smaller than the rows on offer, only as
        // many rows as there are lines are produced — never padded to 100.
        // ("1\n2\n" is 3 lines, not 2: a trailing newline always yields one
        // more, empty final line — see `LineIndex`'s own doc comment.)
        let small = Buffer::from_bytes(b"1\n2\n").unwrap();
        let frame = build_frame(&minimap, &small, 0..1, 100).unwrap();
        assert_eq!(frame.rows.len(), 3);
    }

    #[test]
    fn viewport_indicator_matches_the_one_to_one_visible_range() {
        let buffer = Buffer::from_bytes(b"1\n2\n3\n4\n5").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 1..3, 100).unwrap();
        assert_eq!(frame.viewport.rows, 1..3);
    }

    #[test]
    fn viewport_indicator_tracks_the_visible_range_through_downsampling() {
        // 10 lines, 5 rows -> 2 lines/row. Visible lines 4..6 (0-indexed)
        // fall in rows 2 and 3.
        let buffer = Buffer::from_bytes(b"0\n1\n2\n3\n4\n5\n6\n7\n8\n9").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 4..6, 5).unwrap();
        assert_eq!(frame.viewport.rows, 2..3);
    }

    #[test]
    fn viewport_indicator_is_empty_for_an_empty_visible_range() {
        let buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 0..0, 100).unwrap();
        assert_eq!(frame.viewport.rows, 0..0);
    }

    #[test]
    fn line_for_row_maps_a_click_back_to_the_right_source_line() {
        // 10 lines, 5 rows -> row 2 covers source lines 4-5.
        assert_eq!(line_for_row(10, 5, 2), 4);
    }

    #[test]
    fn line_for_row_clamps_an_out_of_range_row_to_the_last_line() {
        assert_eq!(line_for_row(4, 100, 99), 3);
    }

    #[test]
    fn empty_document_is_a_single_empty_row() {
        let buffer = Buffer::from_bytes(b"").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 0..1, 100).unwrap();
        assert_eq!(frame.rows.len(), 1);
        assert_eq!(frame.rows[0].content_columns, 0);
    }
}
