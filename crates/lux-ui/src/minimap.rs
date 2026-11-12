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
    folds: Option<&lux_core::folding::FoldState>,
) -> Option<MinimapFrame> {
    if !minimap.enabled {
        return None;
    }
    let lines = line_densities(buffer);
    Some(frame_from_densities(
        &lines,
        visible_line_range,
        available_rows,
        folds,
    ))
}

/// Remembers one document's row bars so a scroll does not walk the rope again.
/// `version` is the history state. `document` distinguishes two buffers that
/// both still have state 0. `edited_line` patches one line after a key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MinimapModel {
    version: u64,
    document: u64,
    available_rows: usize,
    fold_stamp: u64,
    lines: Vec<(u32, u32)>,
    rows: Vec<MinimapRow>,
}

pub struct MinimapSample<'a> {
    pub version: u64,
    pub document: u64,
    pub edited_line: Option<usize>,
    pub buffer: &'a Buffer,
    pub visible: Range<usize>,
    pub available_rows: usize,
    pub folds: Option<&'a lux_core::folding::FoldState>,
}

impl MinimapModel {
    pub fn frame(&mut self, minimap: &Minimap, sample: MinimapSample<'_>) -> Option<MinimapFrame> {
        if !minimap.enabled || sample.available_rows == 0 {
            return None;
        }
        let total_lines = sample.buffer.len_lines();
        let fold_stamp = fold_stamp(sample.folds);
        let same = self.rows_match(
            sample.version,
            sample.document,
            sample.available_rows,
            total_lines,
            fold_stamp,
        );
        if !same {
            let patched = self.try_patch(&sample, total_lines, fold_stamp);
            if !patched {
                self.lines = line_densities(sample.buffer);
                self.rows = rows_from_densities(&self.lines, sample.available_rows, sample.folds);
            }
            self.version = sample.version;
            self.document = sample.document;
            self.available_rows = sample.available_rows;
            self.fold_stamp = fold_stamp;
        }
        Some(frame_from_densities_rows(
            &self.rows,
            &self.lines,
            sample.visible,
            sample.available_rows,
            sample.folds,
        ))
    }

    fn rows_match(
        &self,
        version: u64,
        document: u64,
        available_rows: usize,
        total_lines: usize,
        fold_stamp: u64,
    ) -> bool {
        !self.rows.is_empty()
            && self.version == version
            && self.document == document
            && self.available_rows == available_rows
            && self.lines.len() == total_lines
            && self.fold_stamp == fold_stamp
    }

    fn try_patch(
        &mut self,
        sample: &MinimapSample<'_>,
        total_lines: usize,
        fold_stamp: u64,
    ) -> bool {
        let Some(line) = sample.edited_line else {
            return false;
        };
        if self.document != sample.document
            || self.available_rows != sample.available_rows
            || self.fold_stamp != fold_stamp
            || self.lines.len() != total_lines
            || line >= self.lines.len()
        {
            return false;
        }
        self.lines[line] = sample.buffer.line_columns(line).unwrap_or((0, 0));
        self.rows = rows_from_densities(&self.lines, sample.available_rows, sample.folds);
        true
    }
}

fn line_densities(buffer: &Buffer) -> Vec<(u32, u32)> {
    (0..buffer.len_lines())
        .map(|line| buffer.line_columns(line).unwrap_or((0, 0)))
        .collect()
}

fn fold_stamp(folds: Option<&lux_core::folding::FoldState>) -> u64 {
    let Some(folds) = folds else {
        return 0;
    };
    let mut stamp = folds.folded_ranges().len() as u64;
    for (start, range) in folds.folded_ranges() {
        stamp = stamp
            .wrapping_mul(31)
            .wrapping_add(*start as u64)
            .wrapping_mul(31)
            .wrapping_add(range.end_line as u64);
    }
    stamp
}

fn frame_from_densities(
    lines: &[(u32, u32)],
    visible_line_range: Range<usize>,
    available_rows: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> MinimapFrame {
    let rows = rows_from_densities(lines, available_rows, folds);
    frame_from_rows(&rows, lines, visible_line_range, available_rows, folds)
}

fn frame_from_densities_rows(
    rows: &[MinimapRow],
    lines: &[(u32, u32)],
    visible_line_range: Range<usize>,
    available_rows: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> MinimapFrame {
    frame_from_rows(rows, lines, visible_line_range, available_rows, folds)
}

fn frame_from_rows(
    rows: &[MinimapRow],
    lines: &[(u32, u32)],
    visible_line_range: Range<usize>,
    available_rows: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> MinimapFrame {
    let total_lines = lines.len();
    let total_visual = folds.map_or(total_lines, |folds| folds.total_visual_lines(total_lines));
    let viewport_rows = if visible_line_range.is_empty() {
        0..0
    } else {
        let first_row = row_for_line(total_visual, available_rows, visible_line_range.start);
        let last_line = visible_line_range
            .end
            .saturating_sub(1)
            .min(total_visual.saturating_sub(1));
        let last_row = row_for_line(total_visual, available_rows, last_line);
        first_row..last_row + 1
    };
    MinimapFrame {
        rows: rows.to_vec(),
        viewport: ViewportIndicator {
            rows: viewport_rows,
        },
    }
}

fn rows_from_densities(
    lines: &[(u32, u32)],
    available_rows: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> Vec<MinimapRow> {
    let total_lines = lines.len();
    let total_visual = folds.map_or(total_lines, |folds| folds.total_visual_lines(total_lines));
    let per_row = lines_per_row(total_visual, available_rows);
    (0..row_count(total_visual, available_rows))
        .map(|row| densest_row(lines, row, per_row, total_visual, total_lines, folds))
        .collect()
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

pub fn line_for_row_with_folds(
    total_lines: usize,
    available_rows: usize,
    row: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> usize {
    let total_visual = folds.map_or(total_lines, |f| f.total_visual_lines(total_lines));
    let visual_line = line_for_row(total_visual, available_rows, row);
    folds.map_or(visual_line, |f| {
        f.visual_to_buffer_line(visual_line, total_lines)
    })
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

pub fn row_for_line(total_lines: usize, available_rows: usize, line: usize) -> usize {
    line / lines_per_row(total_lines, available_rows)
}

pub fn row_for_buffer_line(
    total_lines: usize,
    available_rows: usize,
    buffer_line: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> Option<usize> {
    let total_visual = folds.map_or(total_lines, |f| f.total_visual_lines(total_lines));
    let visual_line = match folds {
        Some(f) => f.buffer_to_visual_line(buffer_line)?,
        None => buffer_line,
    };
    Some(row_for_line(total_visual, available_rows, visual_line))
}

fn densest_row(
    lines: &[(u32, u32)],
    row: usize,
    per_row: usize,
    total_visual: usize,
    total_lines: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> MinimapRow {
    let start_v = row * per_row;
    let end_v = (start_v + per_row).min(total_visual);
    let first_buf = folds.map_or(start_v, |folds| {
        folds.visual_to_buffer_line(start_v, total_lines)
    });
    let mut best = MinimapRow {
        line: first_buf,
        indent_columns: 0,
        content_columns: 0,
    };
    for visual in start_v..end_v {
        let line = folds.map_or(visual, |folds| {
            folds.visual_to_buffer_line(visual, total_lines)
        });
        let (indent, content) = lines.get(line).copied().unwrap_or((0, 0));
        if content as usize > best.content_columns {
            best = MinimapRow {
                line,
                indent_columns: indent as usize,
                content_columns: content as usize,
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
        assert_eq!(build_frame(&minimap, &buffer, 0..2, 100, None), None);
    }

    #[test]
    fn build_frame_is_one_to_one_when_the_document_fits() {
        let buffer = Buffer::from_bytes(b"one\n  two\nthree\n").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 0..4, 100, None).unwrap();
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
        let frame = build_frame(&minimap, &buffer, 0..1, 3, None).unwrap();
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
        let frame = build_frame(&minimap, &buffer, 0..1, 3, None).unwrap();
        assert_eq!(frame.rows.len(), 3);

        // And when the document is smaller than the rows on offer, only as
        // many rows as there are lines are produced — never padded to 100.
        // ("1\n2\n" is 3 lines, not 2: a trailing newline always yields one
        // more, empty final line — see `LineIndex`'s own doc comment.)
        let small = Buffer::from_bytes(b"1\n2\n").unwrap();
        let frame = build_frame(&minimap, &small, 0..1, 100, None).unwrap();
        assert_eq!(frame.rows.len(), 3);
    }

    #[test]
    fn viewport_indicator_matches_the_one_to_one_visible_range() {
        let buffer = Buffer::from_bytes(b"1\n2\n3\n4\n5").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 1..3, 100, None).unwrap();
        assert_eq!(frame.viewport.rows, 1..3);
    }

    #[test]
    fn viewport_indicator_tracks_the_visible_range_through_downsampling() {
        // 10 lines, 5 rows -> 2 lines/row. Visible lines 4..6 (0-indexed)
        // fall in rows 2 and 3.
        let buffer = Buffer::from_bytes(b"0\n1\n2\n3\n4\n5\n6\n7\n8\n9").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 4..6, 5, None).unwrap();
        assert_eq!(frame.viewport.rows, 2..3);
    }

    #[test]
    fn viewport_indicator_is_empty_for_an_empty_visible_range() {
        let buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
        let minimap = Minimap::new();
        let frame = build_frame(&minimap, &buffer, 0..0, 100, None).unwrap();
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
        let frame = build_frame(&minimap, &buffer, 0..1, 100, None).unwrap();
        assert_eq!(frame.rows.len(), 1);
        assert_eq!(frame.rows[0].content_columns, 0);
    }

    #[test]
    fn build_frame_skips_folded_lines() {
        let buffer = Buffer::from_bytes(b"header\nhidden 1\nhidden 2\ntail\n").unwrap();
        let minimap = Minimap::new();
        let mut folds = lux_core::folding::FoldState::new();
        folds.set_foldable_ranges(vec![lux_core::folding::FoldRange::new(0, 2)]);
        folds.fold(0);

        let frame = build_frame(&minimap, &buffer, 0..2, 100, Some(&folds)).unwrap();
        // 5 total lines originally, lines 1..=2 hidden -> 3 visual lines: header, tail, trailing empty
        assert_eq!(frame.rows.len(), 3);
        assert_eq!(frame.rows[0].line, 0);
        assert_eq!(frame.rows[1].line, 3);
    }

    fn sample<'a>(
        buffer: &'a Buffer,
        version: u64,
        edited_line: Option<usize>,
        visible: Range<usize>,
        available_rows: usize,
    ) -> MinimapSample<'a> {
        MinimapSample {
            version,
            document: 7,
            edited_line,
            buffer,
            visible,
            available_rows,
            folds: None,
        }
    }

    #[test]
    fn scrolling_reuses_the_row_bars() {
        let mut text = String::new();
        for i in 0..200 {
            text.push_str(&format!("line {i}\n"));
        }
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let minimap = Minimap::new();
        let mut model = MinimapModel::default();
        let top = model
            .frame(&minimap, sample(&buffer, 1, None, 0..10, 40))
            .unwrap();
        let scrolled = model
            .frame(&minimap, sample(&buffer, 1, None, 80..100, 40))
            .unwrap();
        assert_eq!(top.rows, scrolled.rows);
        assert_ne!(top.viewport.rows, scrolled.viewport.rows);
    }

    #[test]
    fn one_typed_line_patches_that_bar() {
        let mut buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
        let minimap = Minimap::new();
        let mut model = MinimapModel::default();
        let before = model
            .frame(&minimap, sample(&buffer, 1, None, 0..2, 100))
            .unwrap();
        buffer.insert(0, "zzz");
        let after = model
            .frame(&minimap, sample(&buffer, 2, Some(0), 0..2, 100))
            .unwrap();
        assert_eq!(before.rows[0].content_columns, 1);
        assert_eq!(after.rows[0].content_columns, 4);
        assert_eq!(after.rows[1], before.rows[1]);
    }
}
