//! The line-number gutter: geometry shared by paint and hit testing, and
//! the opaque gutter pass.
//!
//! Left to right, in points: 8 padding, the digits, 6, a 3-wide git
//! change bar, 2, and a 10-wide fold column. Fold markers show only while
//! the pointer is over the gutter, or on a line that is folded.

use lux_render::{GlyphInstance, QuadInstance};
use lux_ui::Theme;
use lux_ui::editor_view::{EditorViewFrame, Viewport};
use swash::FontRef;

use super::super::clip;
use super::super::paint::{color, push_glyph};
use super::push_quad;
use crate::app::AtlasState;

const NUMBER_PAD: f32 = 8.0;
const DIGIT_GAP: f32 = 6.0;
const MARKER_WIDTH: f32 = 3.0;
const MARKER_GAP: f32 = 2.0;
const FOLD_WIDTH: f32 = 10.0;

/// Left margin of the digit column, in physical pixels.
pub(crate) fn gutter_number_pad(scale: f32) -> f32 {
    (NUMBER_PAD * scale).max(1.0)
}

/// Padding, digits, change bar, and fold column. Zero when there are no
/// digits (distraction-free mode).
pub(crate) fn gutter_total_width(digit_width: f32, scale: f32) -> f32 {
    if digit_width <= 0.0 {
        0.0
    } else {
        gutter_marker_x(0.0, digit_width, scale) + (MARKER_WIDTH + MARKER_GAP + FOLD_WIDTH) * scale
    }
}

/// Left edge of the git change bar.
pub(crate) fn gutter_marker_x(sidebar: f32, digit_width: f32, scale: f32) -> f32 {
    sidebar + gutter_number_pad(scale) + digit_width + DIGIT_GAP * scale
}

/// Width of the git change bar.
pub(crate) fn gutter_marker_width(scale: f32) -> f32 {
    MARKER_WIDTH * scale
}

/// Left edge of the fold column.
pub(crate) fn gutter_fold_x(sidebar: f32, digit_width: f32, scale: f32) -> f32 {
    gutter_marker_x(sidebar, digit_width, scale) + (MARKER_WIDTH + MARKER_GAP) * scale
}

/// Width of the fold column.
pub(crate) fn gutter_fold_width(scale: f32) -> f32 {
    FOLD_WIDTH * scale
}

/// A folded line always shows its marker; a foldable one only while the
/// pointer is over this gutter.
pub(crate) fn fold_marker_visible(folded: bool, foldable: bool, hovered: bool) -> bool {
    folded || (foldable && hovered)
}

/// What the gutter draws besides the numbers.
pub(crate) struct GutterMarks<'a> {
    pub(crate) bookmarks: Option<&'a lux_core::bookmarks::BookmarkState>,
    /// The pointer is over this gutter.
    pub(crate) hovered: bool,
}

impl<'a> GutterMarks<'a> {
    /// `hover` is the top-left corner of the pane whose gutter is under the
    /// pointer, if any (see `MouseState::gutter_hover`); this gutter's pane
    /// starts at `origin`.
    pub(crate) fn new(
        bookmarks: Option<&'a lux_core::bookmarks::BookmarkState>,
        hover: Option<[f32; 2]>,
        origin: [f32; 2],
    ) -> Self {
        let same = |[x, y]: [f32; 2]| (x - origin[0]).abs() < 0.5 && (y - origin[1]).abs() < 0.5;
        Self {
            bookmarks,
            hovered: hover.is_some_and(same),
        }
    }
}

/// Opaque gutter. Drawn after the text so a horizontal scroll cannot cover
/// the line numbers. It does not move with `scroll_x`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_gutter(
    frame: &EditorViewFrame,
    marks: GutterMarks<'_>,
    viewport: &Viewport,
    editor_top: f32,
    line_height: f32,
    sidebar_width: f32,
    gutter_width: f32,
    stroke: f32,
    theme: &Theme,
    atlas_state: &mut AtlasState,
    font: FontRef,
    font_size: f32,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    if gutter_width <= 0.0 {
        return;
    }
    let gutter_bounds = [sidebar_width, editor_top, gutter_width, viewport.height];
    quad_instances.push(QuadInstance::solid(
        gutter_bounds,
        color(theme.chrome.gutter),
    ));
    let foreground = color(theme.foreground);
    let muted = color(theme.muted);
    for line in &frame.lines {
        let y = editor_top + line.top - viewport.scroll_y;
        if marks.bookmarks.is_some_and(|b| b.contains(line.index)) {
            push_bookmark(
                quad_instances,
                [sidebar_width, y, line_height, stroke],
                gutter_bounds,
            );
        }

        let line_pad_y = (line_height - font_size) * 0.5;
        let is_current = frame.cursors.iter().any(|cursor| cursor.line == line.index);
        let num_color = if is_current { foreground } else { muted };
        for cluster in line.number_layout.clusters() {
            let start = glyph_instances.len();
            push_glyph(
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                font,
                font_size,
                &cluster,
                sidebar_width + gutter_number_pad(stroke),
                y + line_pad_y + line.number_layout.baseline(),
                num_color,
                glyph_instances,
            );
            clip::clip_glyphs(glyph_instances, start, gutter_bounds);
        }

        if fold_marker_visible(line.is_folded, line.is_foldable, marks.hovered) {
            let icon_color = if line.is_folded { foreground } else { muted };
            let tri_h = (line_height * 0.40).clamp(5.0 * stroke, 9.0 * stroke);
            let column = gutter_fold_x(sidebar_width, frame.gutter_width, stroke);
            let icon_x = column + ((gutter_fold_width(stroke) - tri_h) / 2.0).max(0.0);
            let tri_y = y + (line_height - tri_h) / 2.0;
            push_fold_triangle(
                quad_instances,
                [icon_x, tri_y, tri_h],
                line.is_folded,
                stroke,
                icon_color,
                gutter_bounds,
            );
        }
    }
}

/// `at` is the gutter's left edge, the line top, line height, and stroke.
fn push_bookmark(out: &mut Vec<QuadInstance>, at: [f32; 4], bounds: [f32; 4]) {
    let [sidebar, y, line_h, stroke] = at;
    let bm_x = sidebar + 2.0 * stroke;
    let bm_w = 4.0 * stroke;
    let bm_h = (line_h * 0.60).clamp(8.0 * stroke, 14.0 * stroke);
    let bm_y = y + (line_h - bm_h) / 2.0;
    let body = [bm_x, bm_y, bm_w, bm_h - 2.0 * stroke];
    push_quad(out, body, [0.25, 0.70, 0.95, 0.95], bounds);
    let tail = [
        bm_x + stroke,
        bm_y + bm_h - 2.0 * stroke,
        bm_w - 2.0 * stroke,
        stroke,
    ];
    push_quad(out, tail, [0.20, 0.60, 0.85, 0.95], bounds);
}

/// A right-pointing triangle for a folded region, a down-pointing one for
/// an open region. `at` is the box's left, top, and side.
fn push_fold_triangle(
    out: &mut Vec<QuadInstance>,
    at: [f32; 3],
    folded: bool,
    stroke: f32,
    ink: [f32; 4],
    bounds: [f32; 4],
) {
    let [x, y, side] = at;
    for step in 0..4 {
        let offset = step as f32 * stroke;
        let inset = offset * 0.85;
        let span = (side - 2.0 * inset).max(stroke);
        let rect = if folded {
            [x + offset, y + inset, stroke, span]
        } else {
            [x + inset, y + offset, span, stroke]
        };
        push_quad(out, rect, ink, bounds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_columns_follow_the_review_spacing_at_both_scales() {
        for scale in [1.0_f32, 2.0] {
            let digits = 14.0 * scale;
            assert_eq!(gutter_number_pad(scale), 8.0 * scale);
            let digit_right = gutter_number_pad(scale) + digits;
            let marker = gutter_marker_x(0.0, digits, scale);
            assert_eq!(marker - digit_right, 6.0 * scale, "digits to change bar");
            assert_eq!(gutter_marker_width(scale), 3.0 * scale);
            let fold = gutter_fold_x(0.0, digits, scale);
            assert_eq!(
                fold - (marker + gutter_marker_width(scale)),
                2.0 * scale,
                "change bar to fold column"
            );
            let total = gutter_total_width(digits, scale);
            assert_eq!(total - fold, gutter_fold_width(scale));
            assert_eq!(gutter_fold_width(scale), 10.0 * scale);
            assert_eq!(total, digits + 29.0 * scale);
            // The sidebar shifts every column by the same amount.
            assert_eq!(gutter_fold_x(100.0, digits, scale), fold + 100.0);
        }
        assert_eq!(gutter_total_width(0.0, 2.0), 0.0);
    }

    #[test]
    fn fold_markers_show_on_hover_or_when_folded() {
        assert!(!fold_marker_visible(false, true, false));
        assert!(fold_marker_visible(false, true, true));
        assert!(fold_marker_visible(true, true, false));
        assert!(!fold_marker_visible(false, false, true));
    }

    #[test]
    fn a_gutter_is_hovered_only_by_a_pointer_over_its_own_pane() {
        let pane = [200.0, 32.0];
        assert!(GutterMarks::new(None, Some(pane), pane).hovered);
        assert!(!GutterMarks::new(None, Some([640.0, 32.0]), pane).hovered);
        // 2 Rows: the lower pane's gutter shares the left edge.
        assert!(!GutterMarks::new(None, Some([200.0, 400.0]), pane).hovered);
        assert!(!GutterMarks::new(None, None, pane).hovered);
    }
}
