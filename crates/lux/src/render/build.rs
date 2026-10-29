//! Rendering logic for the floating Build With... variant picker modal overlay (ST13).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::build::BuildPickerState;
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 9;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_build_picker(
    picker: &BuildPickerState,
    viewport_width: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let width = (viewport_width * 0.52).clamp(360.0 * scale_factor, 600.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height + 12.0 * scale_factor;
    let header_height = 24.0 * scale_factor;
    let input_height = 32.0 * scale_factor;
    let row_height = 28.0 * scale_factor;

    let filtered = picker.filtered();
    let visible_count = filtered.len().min(MAX_VISIBLE_RESULTS);
    let total_height =
        header_height + input_height + (visible_count as f32 * row_height) + 6.0 * stroke;

    let picker_font_size = font_size * 0.82;
    let small_font_size = font_size * 0.72;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: picker_font_size,
        ligatures: false,
    };
    let small_style = lux_render::LineStyle {
        family: &family,
        font_size: small_font_size,
        ligatures: false,
    };

    let foreground = color(theme.foreground);
    let muted = color(theme.muted);
    let accent = color(theme.accent);

    // 1. Overlay main background
    quad_instances.push(QuadInstance {
        rect: [left, top, width, total_height],
        color: [0.09, 0.09, 0.12, 0.98],
    });

    // 2. Outer border
    let border_color = [0.25, 0.25, 0.32, 1.0];
    quad_instances.push(QuadInstance {
        rect: [left, top, width, stroke],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left, top + total_height - stroke, width, stroke],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left, top, stroke, total_height],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left + width - stroke, top, stroke, total_height],
        color: border_color,
    });

    // 3. Header title
    let title_line = editor_view
        .text_layout_mut()
        .layout_line(&small_style, "Build With...", &[]);
    let title_base = title_line.baseline();
    for cluster in title_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            small_font_size,
            &cluster,
            left + 12.0 * scale_factor,
            top + 6.0 * scale_factor + title_base,
            accent,
            glyph_instances,
        );
    }

    // 4. Input box
    let input_top = top + header_height;
    quad_instances.push(QuadInstance {
        rect: [
            left + 8.0 * scale_factor,
            input_top,
            width - 16.0 * scale_factor,
            input_height,
        ],
        color: [0.14, 0.14, 0.18, 1.0],
    });

    let display_text = if picker.query.is_empty() {
        "Type to filter build variants..."
    } else {
        &picker.query
    };
    let input_fg = if picker.query.is_empty() {
        muted
    } else {
        foreground
    };

    let line = editor_view
        .text_layout_mut()
        .layout_line(&style, display_text, &[]);
    let base = line.baseline();
    let text_x = left + 16.0 * scale_factor;
    let text_y = input_top + (input_height - picker_font_size) * 0.5 + base;

    for cluster in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            picker_font_size,
            &cluster,
            text_x,
            text_y,
            input_fg,
            glyph_instances,
        );
    }

    // 5. Results list
    let list_top = input_top + input_height + 4.0 * scale_factor;
    let start_idx = if picker.selected >= visible_count {
        picker.selected + 1 - visible_count
    } else {
        0
    };
    for (i, variant) in filtered
        .iter()
        .skip(start_idx)
        .take(visible_count)
        .enumerate()
    {
        let row_y = list_top + (i as f32 * row_height);
        let actual_idx = start_idx + i;
        let is_selected = actual_idx == picker.selected;

        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [
                    left + 8.0 * scale_factor,
                    row_y,
                    width - 16.0 * scale_factor,
                    row_height - 2.0 * scale_factor,
                ],
                color: [accent[0], accent[1], accent[2], 0.28],
            });
            quad_instances.push(QuadInstance {
                rect: [
                    left + 8.0 * scale_factor,
                    row_y,
                    2.0 * stroke,
                    row_height - 2.0 * scale_factor,
                ],
                color: accent,
            });
        }

        // Variant name
        let item_line = editor_view
            .text_layout_mut()
            .layout_line(&style, &variant.name, &[]);
        let item_base = item_line.baseline();
        let item_y = row_y + (row_height - picker_font_size) * 0.5 + item_base;
        for cluster in item_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                left + 16.0 * scale_factor,
                item_y,
                if is_selected {
                    foreground
                } else {
                    [0.85, 0.85, 0.88, 1.0]
                },
                glyph_instances,
            );
        }

        // Command preview on the right
        let cmd_preview = format!("{} {}", variant.command, variant.args.join(" "));
        let cmd_line = editor_view
            .text_layout_mut()
            .layout_line(&small_style, &cmd_preview, &[]);
        let cmd_base = cmd_line.baseline();
        let cmd_x = left + width - 20.0 * scale_factor - cmd_line.width();
        let cmd_y = row_y + (row_height - small_font_size) * 0.5 + cmd_base;
        if cmd_x > left + 180.0 * scale_factor {
            for cluster in cmd_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    small_font_size,
                    &cluster,
                    cmd_x,
                    cmd_y,
                    muted,
                    glyph_instances,
                );
            }
        }
    }
}
