//! The virtualized editor view: only visible lines are shaped, line
//! numbers, a blinking multi-cursor, multi-selection highlight rectangles,
//! indentation guides, and column rulers — Phase 2 deliverable 4. Built on
//! `lux_core` (buffer/selection data) and `lux_render` (`parley`+`fontique`
//! shaping, see `lux_render::text_layout`) — see `docs/ARCHITECTURE.md`:
//! `lux-ui` is the one crate allowed to know about both.
//!
//! This crate produces geometry for the desktop application's GPU pipeline.
//! Line access and selection geometry are bounded by the visible viewport.

// The accessibility tree still uses this historical snapshot index; frame
// construction now uses Buffer's indexed line access directly.
pub(crate) mod line_index;
mod wrap;

use std::ops::Range;
use std::time::Duration;

use accesskit::{TextPosition, TextSelection};
use lux_core::buffer::{Buffer, Indent};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_render::{LineLayout, LineStyle, TextLayoutContext};

/// Visible viewport geometry in pixels, plus current scroll offsets — what
/// a real resize/scroll event updates every frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
}

/// Editor-wide rendering knobs that don't change per keystroke.
pub struct EditorViewMetrics {
    pub line_height: f32,
    pub font_size: f32,
    pub ligatures: bool,
    pub word_wrap: bool,
    /// Character columns to draw a vertical ruler at (e.g. `[80, 100]`).
    pub column_rulers: Vec<usize>,
    pub blink_interval: Duration,
}

/// One line currently on screen: its shaped layout (glyph clusters,
/// already the visible-only work — nothing off-screen is ever shaped) and
/// the vertical indent-guide x-positions that apply to it.
pub struct VisibleLine {
    pub index: usize,
    pub visual_index: usize,
    pub is_folded: bool,
    pub is_foldable: bool,
    pub number: String,
    pub number_layout: LineLayout,
    pub layout: LineLayout,
    pub indent_guides: Vec<f32>,
    /// Pixel offset of this line from the top of the document.
    pub top: f32,
}

/// A blinking caret at a selection's head, including nonempty selections.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CursorGlyph {
    pub line: usize,
    pub visual_row: usize,
    pub x: f32,
    /// Pixel offset of the caret from the top of the document, including
    /// the soft-wrap row inside its buffer line.
    pub y: f32,
    pub visible: bool,
    pub primary: bool,
}

/// One line's worth of a (possibly multi-line) selection highlight.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SelectionRect {
    pub line: usize,
    pub visual_row: usize,
    pub x_start: f32,
    pub x_end: f32,
    /// Pixel offset of this highlight from the top of the document.
    pub y: f32,
}

/// Everything a render pass needs to draw one frame of the editor view.
pub struct EditorViewFrame {
    pub visible_line_range: Range<usize>,
    pub lines: Vec<VisibleLine>,
    pub gutter_width: f32,
    pub cursors: Vec<CursorGlyph>,
    pub selections: Vec<SelectionRect>,
    pub column_ruler_x: Vec<f32>,
    pub matching_brackets: Vec<SelectionRect>,
}

/// Owns the shaping context and font selection; long-lived, reused across
/// every `build_frame` call the same way `parley`'s own docs recommend
/// reusing `FontContext`/`LayoutContext`.
pub struct EditorView {
    text_layout: TextLayoutContext,
    family: String,
    metrics: EditorViewMetrics,
}

impl EditorView {
    /// Update physical text dimensions after an OS display-scale change.
    pub fn set_text_dimensions(&mut self, font_size: f32, line_height: f32) {
        assert!(font_size.is_finite() && font_size > 0.0);
        assert!(line_height.is_finite() && line_height > 0.0);
        self.metrics.font_size = font_size;
        self.metrics.line_height = line_height;
    }

    /// Read the active view metrics.
    pub fn metrics(&self) -> &EditorViewMetrics {
        &self.metrics
    }

    /// Mutably access the active view metrics.
    pub fn metrics_mut(&mut self) -> &mut EditorViewMetrics {
        &mut self.metrics
    }
    /// Registers `font_bytes` (see `lux_render::TextLayoutContext` — no
    /// system font discovery, by design) and returns `None` if they don't
    /// parse as a font.
    pub fn new(
        mut text_layout: TextLayoutContext,
        font_bytes: &[u8],
        metrics: EditorViewMetrics,
    ) -> Option<Self> {
        let family = text_layout.register_font(font_bytes)?;
        Some(Self {
            text_layout,
            family,
            metrics,
        })
    }

    /// The shaping context this view already owns — exposed so a caller
    /// that also needs to shape text outside `build_frame` (e.g.
    /// `crate::accessibility::build_tree`, which needs the same real
    /// per-character geometry this view uses to paint) can reuse it
    /// instead of registering the font a second time in a duplicate
    /// context.
    pub fn text_layout_mut(&mut self) -> &mut TextLayoutContext {
        &mut self.text_layout
    }

    /// The family name this view's font resolved to — see
    /// [`Self::text_layout_mut`]'s doc comment for why a caller needs both
    /// together.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// A free function, not a `&self` method: it only needs `family` and
    /// `font_size`, and taking them by value (instead of `&self`) leaves
    /// `self.text_layout` free to be borrowed mutably right after, since
    /// the borrow checker sees the two field borrows as disjoint only when
    /// there's no `&self`-taking call in between.
    fn line_style(family: &str, font_size: f32, ligatures: bool) -> LineStyle<'_> {
        LineStyle {
            family,
            font_size,
            ligatures,
        }
    }

    /// Builds one frame: only lines intersecting `viewport` are shaped, the
    /// rest of the (possibly huge) document is untouched by this step.
    pub fn build_frame(
        &mut self,
        buffer: &Buffer,
        cursors: &MultiCursor,
        viewport: &Viewport,
        elapsed: Duration,
        folds: Option<&lux_core::folding::FoldState>,
    ) -> EditorViewFrame {
        let total_lines = buffer.len_lines();
        let total_visual = folds.map_or(total_lines, |f| f.total_visual_lines(total_lines));
        let line_height = self.metrics.line_height;
        let gutter_digits = total_lines.max(1).to_string().len();
        let gutter_width = self.reference_width("0".repeat(gutter_digits).as_str());
        let column_ruler_x = self.column_ruler_x();
        let blink_visible = cursor_blink_visible(elapsed, self.metrics.blink_interval);
        let text_width =
            (viewport.width - gutter_width - self.metrics.font_size * (22.0 / 16.0)).max(50.0);
        let placed: Vec<(usize, f32)> = if self.metrics.word_wrap {
            let char_w = self.reference_width("0").max(1.0);
            let chars = (text_width / char_w).floor().max(1.0) as usize;
            wrap::visible_slots(buffer, folds, viewport, line_height, chars)
        } else {
            visible_range(viewport, line_height, total_visual)
                .map(|visual| (visual, visual as f32 * line_height))
                .collect()
        };
        let visible_line_range = placed.first().map(|(visual, _)| *visual).unwrap_or(0)
            ..placed.last().map(|(visual, _)| visual + 1).unwrap_or(0);

        let indent = buffer.indent();

        let mut lines = Vec::with_capacity(placed.len());
        for (visual_idx, top) in placed {
            let line_idx = folds.map_or(visual_idx, |f| {
                f.visual_to_buffer_line(visual_idx, total_lines)
            });
            let is_folded = folds.is_some_and(|f| f.is_line_folded(line_idx));
            let is_foldable = folds.is_some_and(|f| f.is_line_foldable(line_idx));
            let content = buffer
                .line_content(line_idx)
                .expect("visible line is in range");
            let line_text = content.as_str();
            let style =
                Self::line_style(&self.family, self.metrics.font_size, self.metrics.ligatures);
            let max_w = if self.metrics.word_wrap {
                Some(text_width)
            } else {
                None
            };
            let layout = self
                .text_layout
                .layout_line_wrapped(&style, line_text, max_w, &[]);
            let indent_guides = indent_guide_x(&layout, line_text, indent);
            let number = format!("{:>gutter_digits$}", line_idx + 1);
            let number_layout = self.text_layout.layout_line(&style, &number, &[]);
            lines.push(VisibleLine {
                index: line_idx,
                visual_index: visual_idx,
                is_folded,
                is_foldable,
                number,
                number_layout,
                layout,
                indent_guides,
                top,
            });
        }

        let mut cursor_glyphs = Vec::new();
        let mut selection_rects = Vec::new();
        let primary = cursors.primary();
        for selection in cursors.cursors() {
            let pos = selection.head;
            if let Some(line) = lines.iter().find(|l| l.index == pos.line) {
                let line_start = buffer
                    .line_start_byte(pos.line)
                    .expect("visible line is in range");
                let (x, row) = wrap::place_byte(&line.layout, pos.byte - line_start);
                cursor_glyphs.push(CursorGlyph {
                    line: pos.line,
                    visual_row: line.visual_index,
                    x,
                    y: line.top + row as f32 * line_height,
                    visible: blink_visible,
                    primary: *selection == primary,
                });
            }
            if selection.is_empty() {
                continue;
            }
            let (start, end) = selection.ordered();
            for line in &lines {
                if line.index >= start.line && line.index <= end.line {
                    let line_start = buffer
                        .line_start_byte(line.index)
                        .expect("visible line is in range");
                    let (x_start, row) = if line.index == start.line {
                        wrap::place_byte(&line.layout, start.byte - line_start)
                    } else {
                        (0.0, 0)
                    };
                    let x_end = if line.index == end.line {
                        x_for_byte(&line.layout, end.byte - line_start)
                    } else {
                        line.layout.width()
                    };
                    selection_rects.push(SelectionRect {
                        line: line.index,
                        visual_row: line.visual_index,
                        x_start,
                        x_end,
                        y: line.top + row as f32 * line_height,
                    });
                }
            }
        }

        let mut matching_brackets = Vec::new();
        let head = primary.head;
        if let Some(match_pos) = lux_core::movement::matching_bracket(buffer, head) {
            let touching_pos = if head.char < buffer.len_chars()
                && matches!(
                    buffer.char_at(head.char),
                    Some('(' | ')' | '[' | ']' | '{' | '}')
                ) {
                Position::from_char(buffer, head.char)
            } else if head.char > 0
                && matches!(
                    buffer.char_at(head.char - 1),
                    Some('(' | ')' | '[' | ']' | '{' | '}')
                )
            {
                Position::from_char(buffer, head.char - 1)
            } else {
                head
            };

            for pos in [touching_pos, match_pos] {
                if let Some(line) = lines.iter().find(|l| l.index == pos.line) {
                    let line_start = buffer
                        .line_start_byte(line.index)
                        .expect("visible line is in range");
                    let next_pos = pos.next_grapheme(buffer);
                    let (x_start, row) = wrap::place_byte(&line.layout, pos.byte - line_start);
                    let x_end = x_for_byte(&line.layout, next_pos.byte - line_start);
                    matching_brackets.push(SelectionRect {
                        line: line.index,
                        visual_row: line.visual_index,
                        x_start,
                        x_end,
                        y: line.top + row as f32 * line_height,
                    });
                }
            }
        }

        EditorViewFrame {
            visible_line_range,
            lines,
            gutter_width,
            cursors: cursor_glyphs,
            selections: selection_rects,
            column_ruler_x,
            matching_brackets,
        }
    }

    /// Maps a physical window point to the nearest valid text position.
    /// Only the pointed-at line is shaped, so mouse motion retains the
    /// editor's visible-work performance bound even for huge documents.
    pub fn hit_test(
        &mut self,
        buffer: &Buffer,
        viewport: &Viewport,
        x: f32,
        y: f32,
        left_padding: f32,
        folds: Option<&lux_core::folding::FoldState>,
    ) -> Position {
        let total_buffer_lines = buffer.len_lines();
        let total_visual_lines = folds.map_or(total_buffer_lines, |f| {
            f.total_visual_lines(total_buffer_lines)
        });
        let last_visual = total_visual_lines.saturating_sub(1);
        let visual_line = (((y.max(0.0) + viewport.scroll_y) / self.metrics.line_height.max(1.0))
            .floor() as usize)
            .min(last_visual);
        let line = folds.map_or(visual_line, |f| {
            f.visual_to_buffer_line(visual_line, total_buffer_lines)
        });
        let content = buffer.line_content(line).unwrap_or_default();
        let digits = total_buffer_lines.max(1).to_string().len();
        let gutter_width = self.reference_width(&"0".repeat(digits));
        let text_x = (x + viewport.scroll_x - gutter_width - left_padding).max(0.0);
        let style = Self::line_style(&self.family, self.metrics.font_size, self.metrics.ligatures);
        let layout = self.text_layout.layout_line(&style, &content, &[]);
        let byte_in_line = byte_for_x(&layout, text_x);
        let line_start = buffer.line_start_byte(line).unwrap_or(0);
        Position::from_byte(buffer, line_start + byte_in_line)
    }

    /// Horizontal position of a buffer position from the window's left
    /// edge, before applying horizontal scroll.
    pub fn x_for_position(
        &mut self,
        buffer: &Buffer,
        position: Position,
        left_padding: f32,
    ) -> f32 {
        let content = buffer.line_content(position.line).unwrap_or_default();
        let digits = buffer.len_lines().max(1).to_string().len();
        let gutter_width = self.reference_width(&"0".repeat(digits));
        let style = Self::line_style(&self.family, self.metrics.font_size, self.metrics.ligatures);
        let layout = self.text_layout.layout_line(&style, &content, &[]);
        let line_start = buffer.line_start_byte(position.line).unwrap_or(0);
        gutter_width + left_padding + x_for_byte(&layout, position.byte - line_start)
    }

    /// Converts AccessKit's line-node/cluster-index text model back to a
    /// core selection so assistive technology can move the real caret.
    pub fn selection_from_accessibility(
        &mut self,
        buffer: &Buffer,
        selection: TextSelection,
    ) -> Option<lux_core::selection::Selection> {
        let anchor = self.position_from_accessibility(buffer, selection.anchor)?;
        let focus = self.position_from_accessibility(buffer, selection.focus)?;
        Some(lux_core::selection::Selection::new(anchor, focus))
    }

    fn position_from_accessibility(
        &mut self,
        buffer: &Buffer,
        position: TextPosition,
    ) -> Option<Position> {
        let line = usize::try_from(position.node.0).ok()?.checked_sub(1)?;
        let content = buffer.line_content(line)?;
        let style = Self::line_style(&self.family, self.metrics.font_size, false);
        let layout = self.text_layout.layout_line(&style, &content, &[]);
        let clusters = layout.clusters();
        let byte_in_line = clusters
            .get(position.character_index)
            .map_or(content.len(), |cluster| cluster.byte_range.start);
        Some(Position::from_byte(
            buffer,
            buffer.line_start_byte(line)? + byte_in_line,
        ))
    }

    /// The width of laying out `text` in this view's font/size, ligatures
    /// off — used for the gutter and column rulers, neither of which is
    /// program text that should ever ligate.
    pub fn reference_width(&mut self, text: &str) -> f32 {
        let style = Self::line_style(&self.family, self.metrics.font_size, false);
        self.text_layout.layout_line(&style, text, &[]).width()
    }

    fn column_ruler_x(&mut self) -> Vec<f32> {
        let Some(&max_col) = self.metrics.column_rulers.iter().max() else {
            return Vec::new();
        };
        let reference = "0".repeat(max_col);
        let style = Self::line_style(&self.family, self.metrics.font_size, false);
        let layout = self.text_layout.layout_line(&style, &reference, &[]);
        let clusters = layout.clusters();
        self.metrics
            .column_rulers
            .iter()
            .map(|&col| match col {
                0 => 0.0,
                col if col <= clusters.len() => {
                    let c = &clusters[col - 1];
                    c.x + c.advance
                }
                _ => layout.width(),
            })
            .collect()
    }
}

/// The first and last (exclusive) line indices intersecting `viewport`,
/// clamped to `total_lines` — the whole point of this deliverable: nothing
/// outside this range gets shaped.
fn visible_range(viewport: &Viewport, line_height: f32, total_lines: usize) -> Range<usize> {
    if total_lines == 0 || viewport.height <= 0.0 {
        return 0..0;
    }
    let line_height = line_height.max(1.0);
    let first = ((viewport.scroll_y.max(0.0) / line_height).floor() as usize).min(total_lines - 1);
    // +1 so a line only partially scrolled into view at the bottom edge is
    // still shaped (better than a visible gap while scrolling).
    let visible_count = (viewport.height / line_height).ceil() as usize + 1;
    let last = first.saturating_add(visible_count).min(total_lines);
    first..last
}

/// A caret's x position within its line's shaped layout, given its byte
/// offset from the line's start. Interpolates linearly across a cluster
/// when `byte_in_line` lands inside one instead of exactly at its start
/// (possible for a ligature, whose cluster spans more than one source
/// byte) — an approximation, not exact sub-glyph caret placement, which is
/// a deeper problem left for when a real ligature-bearing font is in use
/// (see the receipt).
fn x_for_byte(layout: &LineLayout, byte_in_line: usize) -> f32 {
    wrap::place_byte(layout, byte_in_line).0
}

/// Chooses the source boundary nearest a horizontal point. Cluster
/// boundaries keep the result out of the middle of a shaped grapheme or
/// programming ligature.
fn byte_for_x(layout: &LineLayout, x: f32) -> usize {
    let clusters = layout.clusters();
    for cluster in &clusters {
        if x < cluster.x + cluster.advance / 2.0 {
            return cluster.byte_range.start;
        }
        if x < cluster.x + cluster.advance {
            return cluster.byte_range.end;
        }
    }
    clusters.last().map_or(0, |cluster| cluster.byte_range.end)
}

/// Indent-guide x-positions for one line: one guide per full indent level
/// of leading whitespace, using `Buffer::indent()`'s detected width.
///
/// ponytail: treats every leading space or tab as one column (matching
/// `lux_core::Position::column`'s current grapheme-count semantics, which
/// don't expand tabs either) rather than expanding a tab to its visual
/// width. Upgrade to real tab-expansion once `lux-config` defines a visual
/// tab-width setting; until then this is consistent with the rest of the
/// codebase, not a shortcut only taken here.
fn indent_guide_x(layout: &LineLayout, line_text: &str, indent: Indent) -> Vec<f32> {
    let width = indent.width.max(1) as usize;
    let leading = line_text
        .bytes()
        .take_while(|&b| b == b' ' || b == b'\t')
        .count();
    let levels = leading / width;
    (1..=levels)
        .map(|level| x_for_byte(layout, level * width))
        .collect()
}

/// Whether a blinking caret is in its "on" phase at `elapsed` — a pure
/// function of time, not a timer: the actual blink redraw is driven by a
/// future event/timer in the real render loop (golden rule #5, no
/// polling), not by this function, which only answers "on or off right
/// now" for a given instant.
fn cursor_blink_visible(elapsed: Duration, interval: Duration) -> bool {
    if interval.is_zero() {
        return true;
    }
    let interval_ms = interval.as_millis().max(1);
    (elapsed.as_millis() / interval_ms).is_multiple_of(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_range_clamps_to_document_length() {
        let viewport = Viewport {
            width: 800.0,
            height: 40.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        assert_eq!(visible_range(&viewport, 20.0, 3), 0..3);
    }

    #[test]
    fn visible_range_scrolls_the_window_forward() {
        let viewport = Viewport {
            width: 800.0,
            height: 40.0,
            scroll_x: 0.0,
            scroll_y: 100.0,
        };
        // 100px / 20px line height = line 5 onward, 40px/20px + 1 = 3 lines.
        assert_eq!(visible_range(&viewport, 20.0, 1000), 5..8);
    }

    #[test]
    fn visible_range_of_an_empty_document_is_empty() {
        let viewport = Viewport {
            width: 800.0,
            height: 40.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        assert_eq!(visible_range(&viewport, 20.0, 0), 0..0);
    }

    #[test]
    fn blink_alternates_every_interval() {
        let interval = Duration::from_millis(500);
        assert!(cursor_blink_visible(Duration::from_millis(0), interval));
        assert!(cursor_blink_visible(Duration::from_millis(499), interval));
        assert!(!cursor_blink_visible(Duration::from_millis(500), interval));
        assert!(!cursor_blink_visible(Duration::from_millis(999), interval));
        assert!(cursor_blink_visible(Duration::from_millis(1000), interval));
    }

    #[test]
    fn blink_is_always_visible_when_interval_is_zero() {
        assert!(cursor_blink_visible(
            Duration::from_millis(12345),
            Duration::ZERO
        ));
    }
}
