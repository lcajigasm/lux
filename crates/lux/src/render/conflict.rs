//! Rendering for the external file conflict bar (G14).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::conflict::{ConflictAction, ConflictButton, ConflictPrompt};
use crate::render::paint::push_glyph;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_conflict_bar(
    prompt: &mut ConflictPrompt,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    viewport_width: f32,
    sidebar_width: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    _theme: &lux_ui::Theme,
) {
    let bar_h = 36.0 * scale_factor;
    let bar_x = sidebar_width;
    let bar_y = tab_bar_height;
    let bar_w = viewport_width;

    prompt.bar_rect = [bar_x, bar_y, bar_w, bar_h];

    // Background quad: warning amber/dark brown
    quad_instances.push(QuadInstance::solid(
        [bar_x, bar_y, bar_w, bar_h],
        [0.28, 0.20, 0.05, 0.95],
    ));

    // Border bottom: amber accent
    quad_instances.push(QuadInstance::solid(
        [bar_x, bar_y + bar_h - 1.0, bar_w, 1.0],
        [0.85, 0.65, 0.15, 1.0],
    ));

    let font_size = 12.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    // Render message text
    let text_x = bar_x + 12.0 * scale_factor;
    let text_y = bar_y + (bar_h - font_size) * 0.5;
    let msg_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &prompt.message, &[]);
    for cluster in msg_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            text_x,
            text_y + msg_line.baseline(),
            [0.95, 0.85, 0.65, 1.0],
            glyph_instances,
        );
    }

    // Render buttons
    let btn_h = 24.0 * scale_factor;
    let btn_y = bar_y + (bar_h - btn_h) / 2.0;

    let button_defs: [(&str, ConflictAction, f32, [f32; 4]); 4] = [
        (
            "Compare Diff",
            ConflictAction::CompareDiff,
            100.0 * scale_factor,
            [0.18, 0.35, 0.55, 0.9],
        ),
        (
            "Overwrite Disk",
            ConflictAction::OverwriteDisk,
            105.0 * scale_factor,
            [0.55, 0.22, 0.22, 0.9],
        ),
        (
            "Reload Disk",
            ConflictAction::ReloadDisk,
            95.0 * scale_factor,
            [0.20, 0.45, 0.25, 0.9],
        ),
        (
            "Dismiss",
            ConflictAction::Dismiss,
            70.0 * scale_factor,
            [0.35, 0.35, 0.35, 0.8],
        ),
    ];

    prompt.buttons.clear();
    let mut cur_x = bar_x + bar_w - 12.0 * scale_factor;

    for (label, action, btn_w, btn_color) in button_defs.into_iter().rev() {
        cur_x -= btn_w;
        let rect = [cur_x, btn_y, btn_w, btn_h];
        quad_instances.push(QuadInstance::solid(rect, btn_color));

        // Label glyphs
        let btn_line = editor_view
            .text_layout_mut()
            .layout_line(&style, label, &[]);
        let label_x = cur_x + (btn_w - btn_line.width()) * 0.5;
        let label_y = btn_y + (btn_h - font_size) * 0.5;
        for cluster in btn_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &cluster,
                label_x,
                label_y + btn_line.baseline(),
                [1.0, 1.0, 1.0, 1.0],
                glyph_instances,
            );
        }

        prompt.buttons.push(ConflictButton {
            label,
            action,
            rect,
        });
        cur_x -= 8.0 * scale_factor;
    }
}
