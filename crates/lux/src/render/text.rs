//! Editor text lines, line numbers gutter, and syntax highlight rendering.

use lux_core::buffer::Buffer;
use lux_render::{GlyphInstance, QuadInstance};
use lux_ui::Theme;
use lux_ui::editor_view::{EditorViewFrame, Viewport};
use swash::FontRef;

use super::paint::{color, push_glyph, syntax_color};
use crate::app::{AtlasState, SyntaxState};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_text_lines(
    frame: &EditorViewFrame,
    buffer: &Buffer,
    syntax: Option<&SyntaxState>,
    bookmarks: Option<&lux_core::bookmarks::BookmarkState>,
    viewport: &Viewport,
    editor_top: f32,
    line_height: f32,
    sidebar_width: f32,
    text_left: f32,
    stroke: f32,
    show_gutter: bool,
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

    // Column rulers
    for ruler_x in &frame.column_ruler_x {
        quad_instances.push(QuadInstance {
            rect: [
                text_left + ruler_x - viewport.scroll_x,
                editor_top,
                stroke,
                viewport.height,
            ],
            color: muted,
        });
    }

    // Indent guides and selections
    for line in &frame.lines {
        let y = editor_top + line.visual_index as f32 * line_height - viewport.scroll_y;
        for guide_x in &line.indent_guides {
            quad_instances.push(QuadInstance {
                rect: [
                    text_left + guide_x - viewport.scroll_x,
                    y,
                    stroke,
                    line_height,
                ],
                color: muted,
            });
        }
    }
    for selection in &frame.selections {
        let y = editor_top + selection.visual_row as f32 * line_height - viewport.scroll_y;
        let width = (selection.x_end - selection.x_start).max(1.0);
        quad_instances.push(QuadInstance {
            rect: [
                text_left + selection.x_start - viewport.scroll_x,
                y,
                width,
                line_height,
            ],
            color: selection_color,
        });
    }

    // Matching bracket highlights: subtle golden underline on matching delimiters
    for b_rect in &frame.matching_brackets {
        let y = editor_top + b_rect.visual_row as f32 * line_height - viewport.scroll_y;
        let width = (b_rect.x_end - b_rect.x_start).max(1.0);
        let x = text_left + b_rect.x_start - viewport.scroll_x;
        quad_instances.push(QuadInstance {
            rect: [x, y + line_height - 2.0 * stroke, width, 2.0 * stroke],
            color: [0.95, 0.75, 0.20, 0.85],
        });
    }

    // Line numbers in gutter, fold indicators, and text layout clusters
    for line in &frame.lines {
        let y = editor_top + line.visual_index as f32 * line_height - viewport.scroll_y;

        if show_gutter {
            // Gutter bookmark indicator (sleek Sublime cyan bookmark ribbon in left gutter margin)
            if bookmarks.is_some_and(|b| b.contains(line.index)) {
                let bm_x = sidebar_width + 2.0 * stroke;
                let bm_w = 4.0 * stroke;
                let bm_h = (line_height * 0.60).clamp(8.0 * stroke, 14.0 * stroke);
                let bm_y = y + (line_height - bm_h) / 2.0;

                quad_instances.push(QuadInstance {
                    rect: [bm_x, bm_y, bm_w, bm_h - 2.0 * stroke],
                    color: [0.25, 0.70, 0.95, 0.95],
                });
                quad_instances.push(QuadInstance {
                    rect: [
                        bm_x + stroke,
                        bm_y + bm_h - 2.0 * stroke,
                        bm_w - 2.0 * stroke,
                        stroke,
                    ],
                    color: [0.20, 0.60, 0.85, 0.95],
                });
            }

            let is_current = frame.cursors.iter().any(|c| c.line == line.index);
            let num_color = if is_current { foreground } else { muted };
            for cluster in line.number_layout.clusters() {
                push_glyph(
                    &mut atlas_state.atlas,
                    &mut atlas_state.dirty,
                    &mut atlas_state.scale_cx,
                    font,
                    font_size,
                    &cluster,
                    sidebar_width + 8.0,
                    y + line.number_layout.baseline(),
                    num_color,
                    glyph_instances,
                );
            }

            // Gutter fold indicator (▼ for unfolded foldable blocks, ▶ for folded blocks)
            if line.is_folded || line.is_foldable {
                let icon_x = sidebar_width + 8.0 + frame.gutter_width + 2.0 * stroke;
                let icon_color = if line.is_folded { foreground } else { muted };
                let tri_h = (line_height * 0.40).clamp(5.0 * stroke, 9.0 * stroke);
                let tri_y = y + (line_height - tri_h) / 2.0;

                if line.is_folded {
                    // ▶ Right pointing triangle (folded)
                    let cols = 4;
                    for c in 0..cols {
                        let col_x = icon_x + c as f32 * stroke;
                        let inset = c as f32 * stroke * 0.85;
                        let col_y = tri_y + inset;
                        let col_h = (tri_h - 2.0 * inset).max(stroke);
                        quad_instances.push(QuadInstance {
                            rect: [col_x, col_y, stroke, col_h],
                            color: icon_color,
                        });
                    }
                } else {
                    // ▼ Down pointing triangle (unfolded)
                    let rows = 4;
                    let tri_w = tri_h;
                    for r in 0..rows {
                        let row_y = tri_y + r as f32 * stroke;
                        let inset = r as f32 * stroke * 0.85;
                        let row_x = icon_x + inset;
                        let row_w = (tri_w - 2.0 * inset).max(stroke);
                        quad_instances.push(QuadInstance {
                            rect: [row_x, row_y, row_w, stroke],
                            color: icon_color,
                        });
                    }
                }
            }
        }

        // Inline fold marker: subtle pill '⋯' at the end of the folded header line
        if line.is_folded {
            let pill_x = text_left + line.layout.width() - viewport.scroll_x + 6.0 * stroke;
            let pill_h = (line_height * 0.65).clamp(10.0 * stroke, 16.0 * stroke);
            let pill_y = y + (line_height - pill_h) / 2.0;
            let pill_w = 20.0 * stroke;

            // Pill background
            quad_instances.push(QuadInstance {
                rect: [pill_x, pill_y, pill_w, pill_h],
                color: [0.24, 0.24, 0.30, 0.85],
            });
            // Pill border stroke
            quad_instances.push(QuadInstance {
                rect: [pill_x, pill_y, pill_w, stroke],
                color: [0.35, 0.35, 0.42, 1.0],
            });
            quad_instances.push(QuadInstance {
                rect: [pill_x, pill_y + pill_h - stroke, pill_w, stroke],
                color: [0.35, 0.35, 0.42, 1.0],
            });

            // 3 dots inside pill
            let dot_size = 2.0 * stroke;
            let dot_y = y + (line_height - dot_size) / 2.0;
            for dot_i in 0..3 {
                let dot_x = pill_x + 4.0 * stroke + dot_i as f32 * (5.0 * stroke);
                quad_instances.push(QuadInstance {
                    rect: [dot_x, dot_y, dot_size, dot_size],
                    color: foreground,
                });
            }
        }

        let line_start = buffer.line_start_byte(line.index).unwrap_or(0);
        let line_len = buffer.line_content(line.index).map_or(0, |c| c.len());
        let line_end = line_start + line_len;
        let line_tokens = if let Some(syntax) = syntax
            && let Some(doc) = &syntax.document
        {
            doc.highlight_range(&syntax.source, line_start..line_end)
        } else {
            Vec::new()
        };

        for cluster in line.layout.clusters() {
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
            push_glyph(
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                font,
                font_size,
                &cluster,
                text_left - viewport.scroll_x,
                y + line.layout.baseline(),
                glyph_color,
                glyph_instances,
            );
        }
    }
}
