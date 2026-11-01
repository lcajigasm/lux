//! Rendering logic for the floating "About Lux" modal dialog (ST-FIX-02).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::about::AboutDialogState;
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_about_dialog(
    about: &mut AboutDialogState,
    viewport_width: f32,
    viewport_height: f32,
    scale_factor: f32,
    stroke: f32,
    base_font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    _theme: &lux_ui::Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    // 1. Fullscreen dimming backdrop
    quad_instances.push(QuadInstance::solid(
        [0.0, 0.0, viewport_width, viewport_height],
        [0.0, 0.0, 0.0, 0.45],
    ));

    // 2. Centered modal card geometry
    let width = (viewport_width * 0.45).clamp(380.0 * scale_factor, 480.0 * scale_factor);
    let height = 236.0 * scale_factor;
    let left = (viewport_width - width) * 0.5;
    let top = ((viewport_height - height) * 0.4).max(20.0);
    about.card_rect = [left, top, width, height];

    // Card background
    quad_instances.push(QuadInstance::solid(
        [left, top, width, height],
        color(_theme.chrome.panel),
    ));

    // 4-side borders
    quad_instances.push(QuadInstance::solid(
        [left, top, width, stroke],
        color(_theme.chrome.border),
    ));
    quad_instances.push(QuadInstance::solid(
        [left, top + height - stroke, width, stroke],
        color(_theme.chrome.border),
    ));
    quad_instances.push(QuadInstance::solid(
        [left, top, stroke, height],
        color(_theme.chrome.border),
    ));
    quad_instances.push(QuadInstance::solid(
        [left + width - stroke, top, stroke, height],
        color(_theme.chrome.border),
    ));

    // Top sublime-amber accent band
    quad_instances.push(QuadInstance::solid(
        [left, top, width, 3.0 * scale_factor],
        [0.95, 0.55, 0.15, 1.0],
    ));

    let family = editor_view.family().to_string();

    // Helper closure to render centered text line
    let mut render_centered_text = |text: &str, y_offset: f32, size: f32, text_color: [f32; 4]| {
        let style = lux_render::LineStyle {
            family: &family,
            font_size: size,
            ligatures: false,
        };
        let line = editor_view.text_layout_mut().layout_line(&style, text, &[]);
        let text_w = line.width();
        let text_x = left + (width - text_w) * 0.5;
        let text_y = top + y_offset;
        for cluster in line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                size,
                &cluster,
                text_x,
                text_y + line.baseline(),
                text_color,
                glyph_instances,
            );
        }
    };

    // 3. Header title "Lux"
    render_centered_text(
        "Lux",
        24.0 * scale_factor,
        base_font_size * 1.6,
        [1.0, 1.0, 1.0, 1.0],
    );

    // 4. Version line
    let version_str = format!("Version {} (macOS)", about.version);
    render_centered_text(
        &version_str,
        60.0 * scale_factor,
        base_font_size * 0.85,
        [0.95, 0.65, 0.25, 1.0],
    );

    // 5. Tagline
    render_centered_text(
        "Sublime-grade high-performance text editor",
        90.0 * scale_factor,
        base_font_size * 0.82,
        [0.85, 0.85, 0.9, 1.0],
    );

    // 6. Subtitle description
    render_centered_text(
        "Built in Rust • GPU accelerated rendering",
        118.0 * scale_factor,
        base_font_size * 0.72,
        [0.55, 0.55, 0.62, 1.0],
    );

    // 7. License & Copyright
    render_centered_text(
        "MIT License • Copyright © 2026 Lux Contributors",
        140.0 * scale_factor,
        base_font_size * 0.70,
        [0.48, 0.48, 0.54, 1.0],
    );

    // 8. Close Button
    let btn_w = 88.0 * scale_factor;
    let btn_h = 28.0 * scale_factor;
    let btn_x = left + (width - btn_w) * 0.5;
    let btn_y = top + height - 44.0 * scale_factor;
    about.close_btn_rect = [btn_x, btn_y, btn_w, btn_h];

    let btn_bg = if about.close_hovered {
        color(_theme.chrome.panel_row)
    } else {
        color(_theme.chrome.panel_input)
    };
    quad_instances.push(QuadInstance::solid([btn_x, btn_y, btn_w, btn_h], btn_bg));
    // Button border
    quad_instances.push(QuadInstance::solid(
        [btn_x, btn_y, btn_w, stroke],
        color(_theme.chrome.border),
    ));
    quad_instances.push(QuadInstance::solid(
        [btn_x, btn_y + btn_h - stroke, btn_w, stroke],
        color(_theme.chrome.border),
    ));
    quad_instances.push(QuadInstance::solid(
        [btn_x, btn_y, stroke, btn_h],
        color(_theme.chrome.border),
    ));
    quad_instances.push(QuadInstance::solid(
        [btn_x + btn_w - stroke, btn_y, stroke, btn_h],
        color(_theme.chrome.border),
    ));

    // Close text inside button
    let style = lux_render::LineStyle {
        family: &family,
        font_size: base_font_size * 0.8,
        ligatures: false,
    };
    let line = editor_view
        .text_layout_mut()
        .layout_line(&style, "Close", &[]);
    let text_w = line.width();
    let text_x = btn_x + (btn_w - text_w) * 0.5;
    let text_y = btn_y + (btn_h - base_font_size * 0.8) * 0.5;
    for cluster in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            base_font_size * 0.8,
            &cluster,
            text_x,
            text_y + line.baseline(),
            [0.92, 0.92, 0.96, 1.0],
            glyph_instances,
        );
    }
}
