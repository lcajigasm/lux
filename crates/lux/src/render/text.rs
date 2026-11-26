//! Editor text lines and the opaque line-number gutter.
//!
//! Text, selections, rulers, and indent guides are clipped to the text
//! viewport. The gutter is painted after that text: an opaque theme fill
//! that does not scroll horizontally, then the numbers, folds, and
//! bookmarks on top of it. Vertical placement uses the same line origin
//! as the text, so the numbers stay on their lines.

use lux_core::buffer::Buffer;
use lux_render::{GlyphInstance, QuadInstance};
use lux_ui::Theme;
use lux_ui::editor_view::{EditorViewFrame, Viewport};
use swash::FontRef;

use super::clip::{self, clip_rect};
use super::paint::{color, faint_rule, push_glyph, syntax_color};
use crate::app::{AtlasState, SyntaxState};

mod gutter;

pub(crate) use gutter::{
    GutterMarks, gutter_marker_width, gutter_marker_x, gutter_total_width, render_gutter,
};

/// Indent guide ink: the foreground at 12%, and 30% for the guide of the
/// block that holds the caret. Neither reads as syntax color.
pub(crate) const GUIDE_ALPHA: f32 = 0.12;
pub(crate) const ACTIVE_GUIDE_ALPHA: f32 = 0.30;

/// One indent guide's rect and ink: a single device pixel wide, snapped
/// to a pixel column so it never straddles two and blurs.
pub(crate) fn guide_quad(
    x: f32,
    y: f32,
    height: f32,
    active: bool,
    theme: &Theme,
) -> ([f32; 4], [f32; 4]) {
    let ink = color(theme.foreground);
    let alpha = if active {
        ACTIVE_GUIDE_ALPHA
    } else {
        GUIDE_ALPHA
    };
    ([x.round(), y, 1.0, height], [ink[0], ink[1], ink[2], alpha])
}

pub(crate) mod find_decor;

/// The text area quads are clipped to: right of the gutter, inside the
/// editor viewport.
pub(crate) fn text_bounds(
    viewport: &Viewport,
    editor_top: f32,
    sidebar_width: f32,
    text_left: f32,
) -> [f32; 4] {
    [
        text_left,
        editor_top,
        (sidebar_width + viewport.width - text_left).max(0.0),
        viewport.height,
    ]
}

/// Window rectangle of one highlight row. Selections and find matches both
/// go through here, so they line up pixel for pixel.
pub(crate) fn selection_quad(
    rect: &lux_ui::SelectionRect,
    viewport: &Viewport,
    editor_top: f32,
    text_left: f32,
    line_height: f32,
) -> [f32; 4] {
    [
        text_left + rect.x_start - viewport.scroll_x,
        editor_top + rect.y - viewport.scroll_y,
        (rect.x_end - rect.x_start).max(1.0),
        line_height,
    ]
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_text_lines(
    frame: &EditorViewFrame,
    buffer: &Buffer,
    syntax: Option<&SyntaxState>,
    find_decor: Option<&find_decor::FindDecor>,
    viewport: &Viewport,
    editor_top: f32,
    line_height: f32,
    sidebar_width: f32,
    text_left: f32,
    stroke: f32,
    theme: &Theme,
    atlas_state: &mut AtlasState,
    font: FontRef,
    font_size: f32,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let foreground = color(theme.foreground);
    let selection_color = color(theme.selection);
    let text_bounds = text_bounds(viewport, editor_top, sidebar_width, text_left);

    for ruler_x in &frame.column_ruler_x {
        push_quad(
            quad_instances,
            [
                text_left + ruler_x - viewport.scroll_x,
                editor_top,
                stroke,
                viewport.height,
            ],
            faint_rule(theme),
            text_bounds,
        );
    }

    for line in &frame.lines {
        let y = editor_top + line.top - viewport.scroll_y;
        for (level, guide_x) in line.indent_guides.iter().enumerate() {
            let x = text_left + guide_x - viewport.scroll_x;
            let active = line.active_guide == Some(level);
            let (rect, ink) = guide_quad(x, y, line_height, active, theme);
            push_quad(quad_instances, rect, ink, text_bounds);
        }
    }
    for selection in &frame.selections {
        push_quad(
            quad_instances,
            selection_quad(selection, viewport, editor_top, text_left, line_height),
            selection_color,
            text_bounds,
        );
    }

    for b_rect in &frame.matching_brackets {
        let y = editor_top + b_rect.y - viewport.scroll_y;
        let width = (b_rect.x_end - b_rect.x_start).max(1.0);
        let x = text_left + b_rect.x_start - viewport.scroll_x;
        push_quad(
            quad_instances,
            [x, y + line_height - 2.0 * stroke, width, 2.0 * stroke],
            [0.95, 0.75, 0.20, 0.85],
            text_bounds,
        );
    }

    for line in &frame.lines {
        let y = editor_top + line.top - viewport.scroll_y;
        if line.is_folded {
            let pill_x = text_left + line.layout.width() - viewport.scroll_x + 6.0 * stroke;
            let pill_h = (line_height * 0.65).clamp(10.0 * stroke, 16.0 * stroke);
            let pill_y = y + (line_height - pill_h) / 2.0;
            let pill_w = 20.0 * stroke;
            push_quad(
                quad_instances,
                [pill_x, pill_y, pill_w, pill_h],
                color(theme.chrome.panel_input),
                text_bounds,
            );
            push_quad(
                quad_instances,
                [pill_x, pill_y, pill_w, stroke],
                color(theme.chrome.border),
                text_bounds,
            );
            push_quad(
                quad_instances,
                [pill_x, pill_y + pill_h - stroke, pill_w, stroke],
                color(theme.chrome.border),
                text_bounds,
            );
            let dot_size = 2.0 * stroke;
            let dot_y = y + (line_height - dot_size) / 2.0;
            for dot_i in 0..3 {
                let dot_x = pill_x + 4.0 * stroke + dot_i as f32 * (5.0 * stroke);
                push_quad(
                    quad_instances,
                    [dot_x, dot_y, dot_size, dot_size],
                    foreground,
                    text_bounds,
                );
            }
        }

        let line_start = buffer.line_start_byte(line.index).unwrap_or(0);
        let line_len = buffer.line_content(line.index).map_or(0, |c| c.len());
        let line_end = line_start + line_len;
        let line_tokens: &[lux_syntax::Token] = if let Some(syntax) = syntax {
            let start_idx = syntax.tokens.partition_point(|t| t.end_byte <= line_start);
            let end_idx =
                syntax.tokens[start_idx..].partition_point(|t| t.start_byte < line_end) + start_idx;
            &syntax.tokens[start_idx..end_idx]
        } else {
            &[]
        };

        let decor = find_decor.map(|decor| (decor, decor.line(buffer, line.index)));
        if let Some((decor, line_decor)) = &decor {
            for rect in decor.hit_rects(frame, buffer, line.index, line_decor, line_height) {
                push_quad(
                    quad_instances,
                    selection_quad(&rect, viewport, editor_top, text_left, line_height),
                    decor.hit,
                    text_bounds,
                );
            }
        }
        let line_pad_y = (line_height - font_size) * 0.5;
        let mut cached_face: Option<(u64, FontRef<'_>)> = None;
        for cluster in line.layout.clusters() {
            let face = match cached_face {
                Some((id, face)) if id == cluster.font_id => face,
                _ => {
                    let face = line.layout.face(cluster.font_id).unwrap_or(font);
                    cached_face = Some((cluster.font_id, face));
                    face
                }
            };
            let abs_byte = line_start + cluster.byte_range.start;
            let decor_ink = decor.as_ref().and_then(|(decor, line_decor)| {
                decor.ink(line_decor.role_at(cluster.byte_range.start))
            });
            let glyph_color = if let Some(ink) = decor_ink {
                ink
            } else if let Some(syntax) = syntax
                && let Some(token) = line_tokens
                    .iter()
                    .find(|t| t.start_byte <= abs_byte && abs_byte < t.end_byte)
            {
                syntax_color(syntax.theme.color_for_token(token))
            } else {
                foreground
            };
            let row_y = y + cluster.row as f32 * line_height;
            let start = glyph_instances.len();
            push_glyph(
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                face,
                font_size,
                &cluster,
                text_left - viewport.scroll_x,
                row_y + line_pad_y + line.layout.baseline(),
                glyph_color,
                glyph_instances,
            );
            clip::clip_glyphs(glyph_instances, start, text_bounds);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_carets(
    frame: &EditorViewFrame,
    viewport: &Viewport,
    editor_top: f32,
    text_left: f32,
    sidebar_width: f32,
    line_pad_y: f32,
    stroke: f32,
    caret_h: f32,
    color: [f32; 4],
    quad_instances: &mut Vec<QuadInstance>,
) -> Option<(f32, f32)> {
    let bounds = [
        text_left,
        editor_top,
        (sidebar_width + viewport.width - text_left).max(0.0),
        viewport.height,
    ];
    let mut primary = None;
    for cursor in &frame.cursors {
        let x = text_left + cursor.x - viewport.scroll_x;
        let y = editor_top + cursor.y - viewport.scroll_y + line_pad_y - stroke;
        if cursor.primary {
            primary = Some((x, y));
        }
        if cursor.visible {
            push_quad(quad_instances, [x, y, 2.0 * stroke, caret_h], color, bounds);
        }
    }
    primary
}

pub(super) fn push_quad(
    out: &mut Vec<QuadInstance>,
    rect: [f32; 4],
    color: [f32; 4],
    bounds: [f32; 4],
) {
    if let Some(rect) = clip_rect(rect, bounds) {
        out.push(QuadInstance::solid(rect, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guides_are_one_snapped_pixel_of_faint_foreground() {
        let theme = Theme::MONOKAI;
        let (rect, ink) = guide_quad(100.4, 20.0, 18.0, false, &theme);
        assert_eq!(rect, [100.0, 20.0, 1.0, 18.0]);
        let fg = color(theme.foreground);
        assert_eq!(&ink[..3], &fg[..3], "guides carry no syntax color");
        assert_eq!(ink[3], GUIDE_ALPHA);
        let (rect, ink) = guide_quad(100.6, 20.0, 18.0, true, &theme);
        assert_eq!(rect[0], 101.0);
        assert_eq!(ink[3], ACTIVE_GUIDE_ALPHA);
    }

    #[test]
    fn the_text_clip_stops_at_the_minimap() {
        use crate::frame_layout::{ChromeSizes, compute};
        for (scale, minimap) in [(1.0, 90.0), (2.0, 180.0), (2.0, 0.0)] {
            let layout = compute(&ChromeSizes {
                window_width: 1000.0 * scale,
                window_height: 700.0 * scale,
                tab_bar: 32.0 * scale,
                status_bar: 24.0 * scale,
                search_bar: 0.0,
                dock: 0.0,
                sidebar: 220.0 * scale,
                minimap,
            });
            let [x, y, width, height] = layout.editor;
            let viewport = Viewport {
                width,
                height,
                scroll_x: 0.0,
                scroll_y: 0.0,
            };
            let text_left = x + 40.0 * scale;
            let clip = text_bounds(&viewport, y, x, text_left);
            assert_eq!(clip[0] + clip[2], 1000.0 * scale - minimap);
        }
    }
}
