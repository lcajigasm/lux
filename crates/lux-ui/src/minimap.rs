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
//! averaging it away with its blank neighbors. That line is drawn as word
//! strokes in its syntax colors (`ink`), taken from the highlight tokens the
//! editor already holds, so the strip reads as miniature text.

mod ink;

pub use ink::{ColorRun, LineColors, MAX_INK_COLUMNS, MinimapSpan};
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
    /// Every row's strokes, back to back; see [`MinimapFrame::row_spans`].
    pub spans: Vec<MinimapSpan>,
    span_starts: Vec<u32>,
    pub viewport: ViewportIndicator,
}

impl MinimapFrame {
    /// The strokes of row `row`, left to right.
    pub fn row_spans(&self, row: usize) -> &[MinimapSpan] {
        let (Some(start), Some(end)) = (self.span_starts.get(row), self.span_starts.get(row + 1))
        else {
            return &[];
        };
        &self.spans[*start as usize..*end as usize]
    }
}

/// Builds one minimap frame for `buffer`, or `None` if the minimap is off
/// — matching golden rule #1: a disabled minimap draws nothing, not an
/// empty one. `visible_line_range` is the main editor's current visible
/// range (`EditorView`'s own `visible_line_range`); `available_rows` is
/// how many pixel rows the minimap widget has, e.g. `height / row_height`.
/// Strokes take the plain foreground; [`MinimapModel`] adds syntax colors.
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
    if buffer.is_huge_file_mode() {
        return Some(sampled_frame(
            buffer,
            visible_line_range,
            available_rows,
            folds,
        ));
    }
    let lines = line_densities(buffer);
    let rows = rows_from_densities(&lines, available_rows, folds);
    let spans = plain_spans(buffer, &rows);
    Some(frame_from_rows(
        &rows,
        &spans,
        lines.len(),
        visible_line_range,
        available_rows,
        folds,
    ))
}

/// Remembers one document's row bars and strokes so a scroll does not walk
/// the rope again. `revision` is [`Buffer::revision`], which changes on every
/// mutation, including replacements that bypass undo history. `document` is
/// the tab. Strokes are cached per row, for the one line each row shows, so
/// their cost follows the minimap's height, not the document's length.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MinimapModel {
    revision: u64,
    document: u64,
    available_rows: usize,
    fold_stamp: u64,
    ink_stamp: u64,
    lines: Vec<(u32, u32)>,
    rows: Vec<MinimapRow>,
    /// Strokes per row; `None` until computed or after that row changed.
    strokes: Vec<Option<Vec<MinimapSpan>>>,
    stats: MinimapStats,
}

/// Work counters, cumulative since the model was created. They make the
/// incremental claim testable: a scroll measures and inks nothing, a typed
/// character measures and inks one line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MinimapStats {
    /// Lines whose indent and length were read from the rope.
    pub lines_measured: u64,
    /// Lines turned into colored strokes.
    pub lines_inked: u64,
}

/// One edit that changed only `line`, taking the buffer from revision
/// `before` straight to `after`. The model patches that bar only when its
/// cached rows are exactly at `before` and the buffer is exactly at `after`;
/// anything else, such as further edits while the minimap was hidden,
/// rebuilds every bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineEdit {
    pub line: usize,
    pub before: u64,
    pub after: u64,
}

pub struct MinimapSample<'a> {
    pub revision: u64,
    pub document: u64,
    pub edit: Option<LineEdit>,
    pub buffer: &'a Buffer,
    pub visible: Range<usize>,
    pub available_rows: usize,
    pub folds: Option<&'a lux_core::folding::FoldState>,
    /// The editor's syntax colors, if the document has a language.
    pub colors: Option<&'a dyn LineColors>,
}

impl MinimapModel {
    pub fn stats(&self) -> MinimapStats {
        self.stats
    }

    pub fn frame(&mut self, minimap: &Minimap, sample: MinimapSample<'_>) -> Option<MinimapFrame> {
        if !minimap.enabled || sample.available_rows == 0 {
            return None;
        }
        if sample.buffer.is_huge_file_mode() {
            return Some(sampled_frame(
                sample.buffer,
                sample.visible,
                sample.available_rows,
                sample.folds,
            ));
        }
        let total_lines = sample.buffer.len_lines();
        let fold_stamp = fold_stamp(sample.folds);
        let same = self.rows_match(
            sample.revision,
            sample.document,
            sample.available_rows,
            total_lines,
            fold_stamp,
        );
        if !same {
            let patched = self.try_patch(&sample, total_lines, fold_stamp);
            if !patched {
                self.lines = line_densities(sample.buffer);
                self.stats.lines_measured += self.lines.len() as u64;
                self.rows = rows_from_densities(&self.lines, sample.available_rows, sample.folds);
                self.strokes = vec![None; self.rows.len()];
            }
            self.revision = sample.revision;
            self.document = sample.document;
            self.available_rows = sample.available_rows;
            self.fold_stamp = fold_stamp;
        }
        let ink_stamp = sample.colors.map_or(0, |colors| colors.stamp());
        if ink_stamp != self.ink_stamp {
            self.strokes.iter_mut().for_each(|row| *row = None);
            self.ink_stamp = ink_stamp;
        }
        self.ink_rows(sample.buffer, sample.colors);
        Some(frame_from_rows(
            &self.rows,
            &self.strokes,
            total_lines,
            sample.visible,
            sample.available_rows,
            sample.folds,
        ))
    }

    fn ink_rows(&mut self, buffer: &Buffer, colors: Option<&dyn LineColors>) {
        let mut runs = Vec::new();
        for (row, strokes) in self.rows.iter().zip(self.strokes.iter_mut()) {
            if strokes.is_some() {
                continue;
            }
            let mut out = Vec::new();
            if row.content_columns > 0 {
                ink::line_spans(buffer, row.line, colors, &mut runs, &mut out);
                self.stats.lines_inked += 1;
            }
            *strokes = Some(out);
        }
    }

    fn rows_match(
        &self,
        revision: u64,
        document: u64,
        available_rows: usize,
        total_lines: usize,
        fold_stamp: u64,
    ) -> bool {
        !self.rows.is_empty()
            && self.revision == revision
            && self.document == document
            && self.available_rows == available_rows
            && self.lines.len() == total_lines
            && self.fold_stamp == fold_stamp
    }

    /// Re-measures the edited line and rebuilds only the row that shows it.
    fn try_patch(
        &mut self,
        sample: &MinimapSample<'_>,
        total_lines: usize,
        fold_stamp: u64,
    ) -> bool {
        let Some(edit) = sample.edit else {
            return false;
        };
        let line = edit.line;
        if self.rows.is_empty()
            || self.revision != edit.before
            || sample.revision != edit.after
            || self.document != sample.document
            || self.available_rows != sample.available_rows
            || self.fold_stamp != fold_stamp
            || self.lines.len() != total_lines
            || line >= self.lines.len()
        {
            return false;
        }
        self.lines[line] = sample.buffer.line_columns(line).unwrap_or((0, 0));
        self.stats.lines_measured += 1;
        let folds = sample.folds;
        let total_visual = folds.map_or(total_lines, |f| f.total_visual_lines(total_lines));
        let per_row = lines_per_row(total_visual, sample.available_rows);
        let row = row_for_buffer_line(total_lines, sample.available_rows, line, folds);
        if let Some(row) = row.filter(|row| *row < self.rows.len()) {
            self.rows[row] =
                densest_row(&self.lines, row, per_row, total_visual, total_lines, folds);
            self.strokes[row] = None;
        }
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

fn plain_spans(buffer: &Buffer, rows: &[MinimapRow]) -> Vec<Option<Vec<MinimapSpan>>> {
    let mut runs = Vec::new();
    rows.iter()
        .map(|row| {
            let mut out = Vec::new();
            ink::line_spans(buffer, row.line, None, &mut runs, &mut out);
            Some(out)
        })
        .collect()
}

fn frame_from_rows(
    rows: &[MinimapRow],
    strokes: &[Option<Vec<MinimapSpan>>],
    total_lines: usize,
    visible_line_range: Range<usize>,
    available_rows: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> MinimapFrame {
    let total_visual = folds.map_or(total_lines, |folds| folds.total_visual_lines(total_lines));
    let mut spans = Vec::new();
    let mut span_starts = Vec::with_capacity(rows.len() + 1);
    for row in strokes {
        span_starts.push(spans.len() as u32);
        spans.extend_from_slice(row.as_deref().unwrap_or(&[]));
    }
    span_starts.push(spans.len() as u32);
    MinimapFrame {
        rows: rows.to_vec(),
        spans,
        span_starts,
        viewport: viewport_rows(total_visual, visible_line_range, available_rows),
    }
}

fn viewport_rows(
    total_visual: usize,
    visible_line_range: Range<usize>,
    available_rows: usize,
) -> ViewportIndicator {
    let rows = if visible_line_range.is_empty() {
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
    ViewportIndicator { rows }
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
pub fn lines_per_row(total_lines: usize, available_rows: usize) -> usize {
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

/// One line per minimap row, read from the rope. A huge file is millions
/// of lines; building a byte range for every line is a second copy of the
/// file and the slow part of the first frame. A huge file has no syntax
/// highlighting, so its strokes take the plain foreground.
fn sampled_frame(
    buffer: &Buffer,
    visible_line_range: Range<usize>,
    available_rows: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> MinimapFrame {
    let total_lines = buffer.len_lines();
    let total_visual = folds.map_or(total_lines, |f| f.total_visual_lines(total_lines));
    let per_row = lines_per_row(total_visual, available_rows);
    let rows: Vec<MinimapRow> = (0..row_count(total_visual, available_rows))
        .map(|row| {
            let start_v = (row * per_row).min(total_visual.saturating_sub(1));
            let line = folds.map_or(start_v, |fold| {
                fold.visual_to_buffer_line(start_v, total_lines)
            });
            let line = line.min(total_lines.saturating_sub(1));
            let (indent_columns, content_columns) = line_columns(buffer, line);
            MinimapRow {
                line,
                indent_columns,
                content_columns,
            }
        })
        .collect();
    let spans = plain_spans(buffer, &rows);
    frame_from_rows(
        &rows,
        &spans,
        total_lines,
        visible_line_range,
        available_rows,
        folds,
    )
}

fn line_columns(buffer: &Buffer, line: usize) -> (usize, usize) {
    if line >= buffer.len_lines() {
        return (0, 0);
    }
    let mut indent = 0usize;
    let mut content = 0usize;
    let mut body = false;
    for ch in buffer.rope().line(line).chars() {
        if ch == '\n' || ch == '\r' {
            break;
        }
        if !body && (ch == ' ' || ch == '\t') {
            indent += 1;
        } else {
            body = true;
            content += 1;
        }
    }
    (indent, content)
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
mod tests;

#[cfg(test)]
mod model_tests;
