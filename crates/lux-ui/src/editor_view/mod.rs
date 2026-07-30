//! The virtualized editor view: only visible lines are shaped, line
//! numbers, a blinking multi-cursor, multi-selection highlight rectangles,
//! indentation guides, and column rulers — Phase 2 deliverable 4. Built on
//! `lux_core` (buffer/selection data) and `lux_render` (`parley`+`fontique`
//! shaping, see `lux_render::text_layout`) — see `docs/ARCHITECTURE.md`:
//! `lux-ui` is the one crate allowed to know about both.
//!
//! This produces the geometry a render pass would draw, not pixels: the
//! quad/text `wgpu` pipeline doesn't exist yet (a separate, later slice of
//! `lux-render`'s own deliverable), so there is nothing to paint onto a
//! framebuffer yet. What's real and tested here is the layer that pipeline
//! will consume — the same reasoning the glyph-atlas and text-layout
//! receipts already used for their own "not on screen yet" scope notes.

mod line_index;

use std::ops::Range;
use std::time::Duration;

use lux_core::buffer::{Buffer, Indent};
use lux_core::multicursor::MultiCursor;
use lux_render::{LineLayout, LineStyle, TextLayoutContext};

use line_index::LineIndex;

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
    /// Character columns to draw a vertical ruler at (e.g. `[80, 100]`).
    pub column_rulers: Vec<usize>,
    pub blink_interval: Duration,
}

/// One line currently on screen: its shaped layout (glyph clusters,
/// already the visible-only work — nothing off-screen is ever shaped) and
/// the vertical indent-guide x-positions that apply to it.
pub struct VisibleLine {
    pub index: usize,
    pub number: String,
    pub layout: LineLayout,
    pub indent_guides: Vec<f32>,
}

/// A single blinking caret (an empty selection) at a point on one line.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CursorGlyph {
    pub line: usize,
    pub x: f32,
    pub visible: bool,
}

/// One line's worth of a (possibly multi-line) selection highlight.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SelectionRect {
    pub line: usize,
    pub x_start: f32,
    pub x_end: f32,
}

/// Everything a render pass needs to draw one frame of the editor view.
pub struct EditorViewFrame {
    pub visible_line_range: Range<usize>,
    pub lines: Vec<VisibleLine>,
    pub gutter_width: f32,
    pub cursors: Vec<CursorGlyph>,
    pub selections: Vec<SelectionRect>,
    pub column_ruler_x: Vec<f32>,
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
    ) -> EditorViewFrame {
        let total_lines = buffer.len_lines();
        let visible_line_range = visible_range(viewport, self.metrics.line_height, total_lines);
        let gutter_digits = total_lines.max(1).to_string().len();
        let gutter_width = self.reference_width("0".repeat(gutter_digits).as_str());
        let column_ruler_x = self.column_ruler_x();
        let blink_visible = cursor_blink_visible(elapsed, self.metrics.blink_interval);

        // `Buffer` has no public per-line accessor (see `line_index`'s doc
        // comment): this is the one place per frame this view materializes
        // the whole document, specifically so the expensive per-character
        // shaping work below stays limited to `visible_line_range`.
        let text = buffer.text();
        let index = LineIndex::build(&text);
        debug_assert_eq!(
            index.len(),
            total_lines,
            "LineIndex's own line-splitting must agree with Buffer::len_lines"
        );
        let indent = buffer.indent();

        let mut lines = Vec::with_capacity(visible_line_range.len());
        for line_idx in visible_line_range.clone() {
            let byte_range = index.line_byte_range(&text, line_idx);
            let line_text = &text[byte_range];
            let style =
                Self::line_style(&self.family, self.metrics.font_size, self.metrics.ligatures);
            let layout = self.text_layout.layout_line(&style, line_text, &[]);
            let indent_guides = indent_guide_x(&layout, line_text, indent);
            lines.push(VisibleLine {
                index: line_idx,
                number: format!("{:>gutter_digits$}", line_idx + 1),
                layout,
                indent_guides,
            });
        }

        let mut cursor_glyphs = Vec::new();
        let mut selection_rects = Vec::new();
        for selection in cursors.cursors() {
            if selection.is_empty() {
                let pos = selection.head;
                if let Some(line) = visible_line(&lines, &visible_line_range, pos.line) {
                    let line_start = index.line_byte_range(&text, pos.line).start;
                    let x = x_for_byte(&line.layout, pos.byte - line_start);
                    cursor_glyphs.push(CursorGlyph {
                        line: pos.line,
                        x,
                        visible: blink_visible,
                    });
                }
                continue;
            }
            let (start, end) = selection.ordered();
            for line_idx in start.line..=end.line {
                let Some(line) = visible_line(&lines, &visible_line_range, line_idx) else {
                    continue;
                };
                let line_start = index.line_byte_range(&text, line_idx).start;
                let x_start = if line_idx == start.line {
                    x_for_byte(&line.layout, start.byte - line_start)
                } else {
                    0.0
                };
                let x_end = if line_idx == end.line {
                    x_for_byte(&line.layout, end.byte - line_start)
                } else {
                    line.layout.width()
                };
                selection_rects.push(SelectionRect {
                    line: line_idx,
                    x_start,
                    x_end,
                });
            }
        }

        EditorViewFrame {
            visible_line_range,
            lines,
            gutter_width,
            cursors: cursor_glyphs,
            selections: selection_rects,
            column_ruler_x,
        }
    }

    /// The width of laying out `text` in this view's font/size, ligatures
    /// off — used for the gutter and column rulers, neither of which is
    /// program text that should ever ligate.
    fn reference_width(&mut self, text: &str) -> f32 {
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

fn visible_line<'a>(
    lines: &'a [VisibleLine],
    visible_line_range: &Range<usize>,
    line_idx: usize,
) -> Option<&'a VisibleLine> {
    if !visible_line_range.contains(&line_idx) {
        return None;
    }
    lines.get(line_idx - visible_line_range.start)
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
    let clusters = layout.clusters();
    for cluster in &clusters {
        if cluster.byte_range.contains(&byte_in_line) {
            let span = (cluster.byte_range.end - cluster.byte_range.start).max(1) as f32;
            let offset = (byte_in_line - cluster.byte_range.start) as f32;
            return cluster.x + cluster.advance * (offset / span);
        }
    }
    layout.width()
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
