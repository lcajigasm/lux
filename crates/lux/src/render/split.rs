//! Split editor multi-panel rendering (T04, ST12).

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::{EditorView, Theme};
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::document::Documents;
use crate::render::paint::{color, push_glyph};
use crate::split::{SplitLayout, compute_layout_geometry};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_split_layout(
    split: &SplitLayout,
    documents: &Documents,
    primary_rect: [f32; 4],
    scale_factor: f32,
    stroke: f32,
    font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    if !split.is_active() {
        return;
    }

    let geom = compute_layout_geometry(split, primary_rect, stroke);

    // 1. Render all dividers
    for (rect, _) in &geom.dividers {
        let divider_color = if split.active_pane > 0 {
            color(theme.accent)
        } else {
            [0.20, 0.20, 0.28, 1.0]
        };
        quad_instances.push(QuadInstance {
            rect: *rect,
            color: divider_color,
        });
    }

    // 2. Render secondary / non-primary panes
    let line_height = font_size * 1.4;
    let gutter_w = 40.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: editor_view.metrics().ligatures,
    };

    for (i, pane_rect) in geom.pane_rects.iter().enumerate() {
        // Pane 0 active is rendered directly by the main editor canvas
        if i == 0 && split.active_pane == 0 {
            continue;
        }

        let [sx, sy, sw, sh] = *pane_rect;

        // Pane background
        quad_instances.push(QuadInstance {
            rect: *pane_rect,
            color: color(theme.background),
        });

        // Active pane indicator border
        if i == split.active_pane {
            quad_instances.push(QuadInstance {
                rect: [sx, sy, sw, 2.0 * stroke],
                color: color(theme.accent),
            });
        }

        let target_tab = if i < split.panes.len() {
            Some(split.panes[i].tab_id)
        } else if i == 1 && split.secondary.is_some() {
            split.secondary.as_ref().map(|s| s.tab_id)
        } else {
            documents.active_tab
        };

        let Some(tab_id) = target_tab else {
            continue;
        };

        let sec_buffer = documents
            .entries
            .iter()
            .find(|e| e.tab_id == tab_id)
            .map(|e| &e.buffer);

        let Some(buf) = sec_buffer else {
            continue;
        };

        let text_x = sx + gutter_w + 12.0 * stroke;
        let total_lines = buf.len_lines();
        let visible_count = (sh / line_height).ceil() as usize;

        for line_idx in 0..visible_count.min(total_lines) {
            let line_y = sy + line_idx as f32 * line_height;
            let content = buf.line_content(line_idx).unwrap_or_default();
            let layout = editor_view
                .text_layout_mut()
                .layout_line(&style, content.as_str(), &[]);

            for cluster in layout.clusters() {
                if cluster.x + text_x > sx + sw {
                    break;
                }
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    font_size,
                    &cluster,
                    text_x,
                    line_y + layout.baseline(),
                    color(theme.foreground),
                    glyph_instances,
                );
            }
        }
    }
}
