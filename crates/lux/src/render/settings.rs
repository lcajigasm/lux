//! Rendering for the Preferences & Settings UI panel (Goal G22).

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, push_glyph};
use crate::settings::{SettingKind, SettingsPanelState};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_settings_panel(
    panel: &SettingsPanelState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    viewport_width: f32,
    viewport_height: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    theme: &lux_ui::Theme,
) {
    let width = (viewport_width * 0.72).clamp(440.0 * scale_factor, 780.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height + 16.0 * scale_factor;
    let row_height = 42.0 * scale_factor;
    let header_height = 56.0 * scale_factor;
    let total_height =
        header_height + (panel.items.len() as f32 * row_height) + 12.0 * scale_factor;

    // 1. Dimmed backdrop
    quad_instances.push(QuadInstance {
        rect: [0.0, 0.0, viewport_width, viewport_height],
        color: [0.0, 0.0, 0.0, 0.45],
    });

    // 2. Card Background
    quad_instances.push(QuadInstance {
        rect: [left, top, width, total_height],
        color: [0.10, 0.10, 0.13, 0.98],
    });

    // 3. Card Border / Focus Ring
    let border_color = color(theme.accent);
    quad_instances.push(QuadInstance {
        rect: [left, top, width, stroke * 2.0],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left, top + total_height - stroke * 2.0, width, stroke * 2.0],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left, top, stroke * 2.0, total_height],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left + width - stroke * 2.0, top, stroke * 2.0, total_height],
        color: border_color,
    });

    let header_font_size = 14.0 * scale_factor;
    let sub_font_size = 10.5 * scale_factor;
    let item_font_size = 12.0 * scale_factor;
    let desc_font_size = 10.0 * scale_factor;

    let family = editor_view.family().to_string();
    let text_fg = color(theme.foreground);
    let text_muted = color(theme.muted);
    let accent_fg = color(theme.accent);

    // 4. Header Title
    let header_style = lux_render::LineStyle {
        family: &family,
        font_size: header_font_size,
        ligatures: false,
    };
    let layout =
        editor_view
            .text_layout_mut()
            .layout_line(&header_style, "Preferences & Settings", &[]);
    for cluster in layout.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            header_font_size,
            &cluster,
            left + 16.0 * scale_factor,
            top + 14.0 * scale_factor,
            text_fg,
            glyph_instances,
        );
    }

    // Header Subtitle
    let sub_style = lux_render::LineStyle {
        family: &family,
        font_size: sub_font_size,
        ligatures: false,
    };
    let layout = editor_view.text_layout_mut().layout_line(
        &sub_style,
        "Esc to close · ↑/↓ navigate · Enter/←/→ toggle values",
        &[],
    );
    for cluster in layout.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            sub_font_size,
            &cluster,
            left + 16.0 * scale_factor,
            top + 34.0 * scale_factor,
            text_muted,
            glyph_instances,
        );
    }

    // Header separator line
    quad_instances.push(QuadInstance {
        rect: [left, top + header_height, width, stroke],
        color: [0.25, 0.25, 0.30, 0.6],
    });

    // 5. Items Rows
    let item_style = lux_render::LineStyle {
        family: &family,
        font_size: item_font_size,
        ligatures: false,
    };
    let desc_style = lux_render::LineStyle {
        family: &family,
        font_size: desc_font_size,
        ligatures: false,
    };

    for (i, item) in panel.items.iter().enumerate() {
        let row_top = top + header_height + 4.0 * scale_factor + (i as f32 * row_height);
        let is_selected = i == panel.selected_index;

        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [
                    left + 4.0 * scale_factor,
                    row_top,
                    width - 8.0 * scale_factor,
                    row_height - 2.0 * scale_factor,
                ],
                color: [0.20, 0.22, 0.30, 0.95],
            });
            quad_instances.push(QuadInstance {
                rect: [
                    left + 4.0 * scale_factor,
                    row_top,
                    3.0 * scale_factor,
                    row_height - 2.0 * scale_factor,
                ],
                color: accent_fg,
            });
        }

        // Setting Label
        let layout = editor_view
            .text_layout_mut()
            .layout_line(&item_style, item.label, &[]);
        for cluster in layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                item_font_size,
                &cluster,
                left + 16.0 * scale_factor,
                row_top + 6.0 * scale_factor,
                if is_selected { accent_fg } else { text_fg },
                glyph_instances,
            );
        }

        // Setting Description
        let layout = editor_view
            .text_layout_mut()
            .layout_line(&desc_style, item.description, &[]);
        for cluster in layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                desc_font_size,
                &cluster,
                left + 16.0 * scale_factor,
                row_top + 23.0 * scale_factor,
                text_muted,
                glyph_instances,
            );
        }

        // Setting Value Badge
        let val_text = match &item.kind {
            SettingKind::Boolean { value } => {
                if *value {
                    "[ ON ]".to_string()
                } else {
                    "[ OFF ]".to_string()
                }
            }
            SettingKind::Number { value, .. } => format!("[ {value:.1} ]"),
            SettingKind::Enum { value, .. } => format!("[ {value} ]"),
        };

        let badge_style = lux_render::LineStyle {
            family: &family,
            font_size: item_font_size,
            ligatures: false,
        };
        let layout = editor_view
            .text_layout_mut()
            .layout_line(&badge_style, &val_text, &[]);
        for cluster in layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                item_font_size,
                &cluster,
                left + width - 130.0 * scale_factor,
                row_top + 13.0 * scale_factor,
                if is_selected { accent_fg } else { text_fg },
                glyph_instances,
            );
        }
    }
}
