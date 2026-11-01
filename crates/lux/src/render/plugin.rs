//! Rendering for the plugin security capability grant dialog.

use swash::FontRef;
use swash::scale::ScaleContext;

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

use crate::plugin::PluginState;
use crate::render::paint::{active_theme, color, push_glyph};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_plugin_modal(
    plugin: &PluginState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    window_width: f32,
    window_height: f32,
    scale_factor: f32,
    stroke: f32,
) {
    let Some(pending) = &plugin.pending_grant else {
        return;
    };

    // Dimmed background backdrop
    quad_instances.push(QuadInstance {
        rect: [0.0, 0.0, window_width, window_height],
        color: [0.0, 0.0, 0.0, 0.55],
    });

    let card_w = 460.0 * scale_factor;
    let card_h = 160.0 * scale_factor;
    let card_x = ((window_width - card_w) * 0.5).max(0.0);
    let card_y = ((window_height - card_h) * 0.5).max(0.0);

    // Card background & warning border
    quad_instances.push(QuadInstance {
        rect: [card_x, card_y, card_w, card_h],
        color: color(active_theme().chrome.panel),
    });
    quad_instances.push(QuadInstance {
        rect: [card_x, card_y, card_w, stroke * 2.0],
        color: [0.95, 0.70, 0.20, 1.0], // Amber warning accent
    });

    let font_size = 12.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    // Title
    let title = "Plugin Capability Request";
    let title_line = editor_view
        .text_layout_mut()
        .layout_line(&style, title, &[]);
    let t_base = title_line.baseline();
    for cluster in title_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size * 1.1,
            &cluster,
            card_x + 18.0 * scale_factor,
            card_y + 16.0 * scale_factor + t_base,
            [0.95, 0.95, 1.0, 1.0],
            glyph_instances,
        );
    }

    // Body
    let caps_str = format!("{:?}", pending.capabilities);
    let body = format!("Plugin '{}' requests: {}", pending.plugin_id, caps_str);
    let body_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &body, &[]);
    let b_base = body_line.baseline();
    for cluster in body_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            card_x + 18.0 * scale_factor,
            card_y + 48.0 * scale_factor + b_base,
            [0.85, 0.85, 0.90, 1.0],
            glyph_instances,
        );
    }

    // Subtitle note
    let note = "Fuel limits and memory quotas are strictly enforced by Wasmtime.";
    let note_line = editor_view.text_layout_mut().layout_line(&style, note, &[]);
    let n_base = note_line.baseline();
    for cluster in note_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size * 0.85,
            &cluster,
            card_x + 18.0 * scale_factor,
            card_y + 72.0 * scale_factor + n_base,
            [0.60, 0.60, 0.65, 1.0],
            glyph_instances,
        );
    }

    // Buttons
    let btn_y = card_y + card_h - 44.0 * scale_factor;
    let btn_h = 28.0 * scale_factor;

    // Grant & Run button
    let grant_w = 140.0 * scale_factor;
    let grant_x = card_x + 18.0 * scale_factor;
    quad_instances.push(QuadInstance {
        rect: [grant_x, btn_y, grant_w, btn_h],
        color: [0.18, 0.55, 0.30, 1.0],
    });
    let grant_line = editor_view
        .text_layout_mut()
        .layout_line(&style, "Grant & Run (Enter)", &[]);
    let g_base = grant_line.baseline();
    for cluster in grant_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size * 0.9,
            &cluster,
            grant_x + 10.0 * scale_factor,
            btn_y + (btn_h - font_size * 0.9) * 0.5 + g_base,
            [1.0, 1.0, 1.0, 1.0],
            glyph_instances,
        );
    }

    // Deny button
    let deny_w = 90.0 * scale_factor;
    let deny_x = grant_x + grant_w + 12.0 * scale_factor;
    quad_instances.push(QuadInstance {
        rect: [deny_x, btn_y, deny_w, btn_h],
        color: [0.60, 0.20, 0.20, 1.0],
    });
    let deny_line = editor_view
        .text_layout_mut()
        .layout_line(&style, "Deny (Esc)", &[]);
    let d_base = deny_line.baseline();
    for cluster in deny_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size * 0.9,
            &cluster,
            deny_x + 14.0 * scale_factor,
            btn_y + (btn_h - font_size * 0.9) * 0.5 + d_base,
            [1.0, 1.0, 1.0, 1.0],
            glyph_instances,
        );
    }
}
