//! Rendering logic for the floating Theme / Color Scheme Picker modal overlay (ST15).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, push_glyph};
use crate::theme_picker::ThemePickerState;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 7;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_theme_picker(
    picker: &ThemePickerState,
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
    let input_height = 36.0 * scale_factor;
    let row_height = 28.0 * scale_factor;

    let filtered = picker.filtered();
    let visible_count = filtered.len().min(MAX_VISIBLE_RESULTS);
    let total_height = input_height + (visible_count as f32 * row_height) + 6.0 * stroke;

    let picker_font_size = font_size * 0.85;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: picker_font_size,
        ligatures: false,
    };

    let foreground = color(theme.foreground);
    let muted = color(theme.muted);
    let accent = color(theme.accent);

    // 1. Overlay main background
    quad_instances.push(QuadInstance {
        rect: [left, top, width, total_height],
        color: color(theme.chrome.panel),
    });

    // 2. Outer border
    let border_color = color(theme.chrome.border);
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

    // 3. Search input header
    let query_display = if picker.query.is_empty() {
        "Select Color Scheme (type to filter...)".to_string()
    } else {
        picker.query.clone()
    };
    let query_color = if picker.query.is_empty() {
        muted
    } else {
        foreground
    };

    let input_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &query_display, &[]);
    let input_baseline = (input_height - picker_font_size) * 0.5;
    let input_text_x = left + 14.0 * scale_factor;
    let input_text_y = top + input_baseline;

    for cluster in input_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            picker_font_size,
            &cluster,
            input_text_x,
            input_text_y + input_line.baseline(),
            query_color,
            glyph_instances,
        );
    }

    // Input bottom separator
    quad_instances.push(QuadInstance {
        rect: [left, top + input_height, width, stroke],
        color: border_color,
    });

    // 4. Candidate theme items
    let scroll_offset = if picker.selected_index >= MAX_VISIBLE_RESULTS {
        picker.selected_index - MAX_VISIBLE_RESULTS + 1
    } else {
        0
    };

    let row_baseline = (row_height - picker_font_size) * 0.5;

    for (row_idx, &theme_name) in filtered
        .iter()
        .skip(scroll_offset)
        .take(MAX_VISIBLE_RESULTS)
        .enumerate()
    {
        let item_idx = scroll_offset + row_idx;
        let is_selected = item_idx == picker.selected_index;
        let row_y = top + input_height + (row_idx as f32 * row_height);

        // Highlight selected row
        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [left + stroke, row_y, width - 2.0 * stroke, row_height],
                color: color(theme.chrome.panel_row),
            });
            // Left selection accent indicator bar
            quad_instances.push(QuadInstance {
                rect: [left + stroke, row_y, 3.0 * stroke, row_height],
                color: accent,
            });
        }

        // Theme name text
        let is_active = theme_name.eq_ignore_ascii_case(theme.name);
        let name_color = if is_selected {
            foreground
        } else if is_active {
            accent
        } else {
            [
                foreground[0] * 0.9,
                foreground[1] * 0.9,
                foreground[2] * 0.9,
                1.0,
            ]
        };

        let label_text = if is_active {
            format!("{theme_name}  (active)")
        } else {
            theme_name.to_string()
        };

        let item_layout = editor_view
            .text_layout_mut()
            .layout_line(&style, &label_text, &[]);
        let text_x = left + 14.0 * scale_factor;
        let text_y = row_y + row_baseline;

        for cluster in item_layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                text_x,
                text_y + item_layout.baseline(),
                name_color,
                glyph_instances,
            );
        }

        // Right-aligned swatch preview (4 colors: bg, fg, accent, syntax token)
        if let Some(target_theme) = lux_ui::Theme::by_name(theme_name) {
            let swatch_size = 12.0 * scale_factor;
            let swatch_gap = 3.0 * scale_factor;
            let swatches_total_w = 4.0 * swatch_size + 3.0 * swatch_gap;
            let swatches_x = left + width - swatches_total_w - 14.0 * scale_factor;
            let swatch_y = row_y + (row_height - swatch_size) * 0.5;

            let c1 = color(target_theme.background);
            let c2 = color(target_theme.foreground);
            let c3 = color(target_theme.accent);
            let c4 = color(target_theme.selection);

            let preview_colors = [c1, c2, c3, c4];
            for (i, &sc) in preview_colors.iter().enumerate() {
                let sx = swatches_x + (i as f32 * (swatch_size + swatch_gap));
                quad_instances.push(QuadInstance {
                    rect: [sx, swatch_y, swatch_size, swatch_size],
                    color: sc,
                });
                // Subtle border around swatch
                quad_instances.push(QuadInstance {
                    rect: [sx, swatch_y, swatch_size, stroke],
                    color: color(theme.chrome.border),
                });
                quad_instances.push(QuadInstance {
                    rect: [sx, swatch_y + swatch_size - stroke, swatch_size, stroke],
                    color: color(theme.chrome.border),
                });
            }
        }
    }
}
