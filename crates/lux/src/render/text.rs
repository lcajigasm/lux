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

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_text_lines(
    frame: &EditorViewFrame,
    buffer: &Buffer,
    syntax: Option<&SyntaxState>,
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
    let muted = color(theme.muted);
    let selection_color = color(theme.selection);
    let text_bounds = [
        text_left,
        editor_top,
        (sidebar_width + viewport.width - text_left).max(0.0),
        viewport.height,
    ];

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
        for guide_x in &line.indent_guides {
            push_quad(
                quad_instances,
                [
                    text_left + guide_x - viewport.scroll_x,
                    y,
                    stroke,
                    line_height,
                ],
                muted,
                text_bounds,
            );
        }
    }
    for selection in &frame.selections {
        let y = editor_top + selection.y - viewport.scroll_y;
        let width = (selection.x_end - selection.x_start).max(1.0);
        push_quad(
            quad_instances,
            [
                text_left + selection.x_start - viewport.scroll_x,
                y,
                width,
                line_height,
            ],
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
            let glyph_color = if let Some(syntax) = syntax
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

/// Opaque gutter. Drawn after the text so a horizontal scroll cannot cover
/// the line numbers. It does not move with `scroll_x`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_gutter(
    frame: &EditorViewFrame,
    bookmarks: Option<&lux_core::bookmarks::BookmarkState>,
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
        if bookmarks.is_some_and(|marks| marks.contains(line.index)) {
            let bm_x = sidebar_width + 2.0 * stroke;
            let bm_w = 4.0 * stroke;
            let bm_h = (line_height * 0.60).clamp(8.0 * stroke, 14.0 * stroke);
            let bm_y = y + (line_height - bm_h) / 2.0;
            push_quad(
                quad_instances,
                [bm_x, bm_y, bm_w, bm_h - 2.0 * stroke],
                [0.25, 0.70, 0.95, 0.95],
                gutter_bounds,
            );
            push_quad(
                quad_instances,
                [
                    bm_x + stroke,
                    bm_y + bm_h - 2.0 * stroke,
                    bm_w - 2.0 * stroke,
                    stroke,
                ],
                [0.20, 0.60, 0.85, 0.95],
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
                sidebar_width + 8.0,
                y + line_pad_y + line.number_layout.baseline(),
                num_color,
                glyph_instances,
            );
            clip::clip_glyphs(glyph_instances, start, gutter_bounds);
        }

        if line.is_folded || line.is_foldable {
            let icon_x = sidebar_width + 8.0 + frame.gutter_width + 2.0 * stroke;
            let icon_color = if line.is_folded { foreground } else { muted };
            let tri_h = (line_height * 0.40).clamp(5.0 * stroke, 9.0 * stroke);
            let tri_y = y + (line_height - tri_h) / 2.0;
            if line.is_folded {
                let cols = 4;
                for column in 0..cols {
                    let col_x = icon_x + column as f32 * stroke;
                    let inset = column as f32 * stroke * 0.85;
                    let col_y = tri_y + inset;
                    let col_h = (tri_h - 2.0 * inset).max(stroke);
                    push_quad(
                        quad_instances,
                        [col_x, col_y, stroke, col_h],
                        icon_color,
                        gutter_bounds,
                    );
                }
            } else {
                let rows = 4;
                let tri_w = tri_h;
                for row in 0..rows {
                    let row_y = tri_y + row as f32 * stroke;
                    let inset = row as f32 * stroke * 0.85;
                    let row_x = icon_x + inset;
                    let row_w = (tri_w - 2.0 * inset).max(stroke);
                    push_quad(
                        quad_instances,
                        [row_x, row_y, row_w, stroke],
                        icon_color,
                        gutter_bounds,
                    );
                }
            }
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

fn push_quad(out: &mut Vec<QuadInstance>, rect: [f32; 4], color: [f32; 4], bounds: [f32; 4]) {
    if let Some(rect) = clip_rect(rect, bounds) {
        out.push(QuadInstance::solid(rect, color));
    }
}
