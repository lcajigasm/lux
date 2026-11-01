//! Rendering logic for the floating Language Mode Picker modal overlay (T02).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::language_picker::LanguagePickerState;
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 9;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_language_picker(
    picker: &LanguagePickerState,
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
    let width = (viewport_width * 0.52).clamp(340.0 * scale_factor, 560.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height + 12.0 * scale_factor;
    let input_height = 34.0 * scale_factor;
    let row_height = 26.0 * scale_factor;

    let filtered = picker.filtered();
    let visible_count = filtered.len().min(MAX_VISIBLE_RESULTS);
    let total_height = input_height + (visible_count as f32 * row_height) + 4.0 * stroke;

    let picker_font_size = font_size * 0.82;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: picker_font_size,
        ligatures: false,
    };

    let foreground = color(theme.foreground);
    let muted = color(theme.muted);

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

    // 3. Input background & separator
    quad_instances.push(QuadInstance {
        rect: [
            left + stroke,
            top + stroke,
            width - 2.0 * stroke,
            input_height,
        ],
        color: color(theme.chrome.panel_input),
    });
    quad_instances.push(QuadInstance {
        rect: [left, top + input_height + stroke, width, stroke],
        color: color(theme.chrome.border),
    });

    // 4. Input text & caret
    let input_baseline = (input_height - picker_font_size) * 0.5;
    let text_x = left + 12.0 * scale_factor;
    let text_y = top + stroke + input_baseline;

    let (display_text, text_color) = if picker.query.is_empty() {
        ("Select Language Mode...", muted)
    } else {
        (picker.query.as_str(), foreground)
    };

    let input_line = editor_view
        .text_layout_mut()
        .layout_line(&style, display_text, &[]);
    for cluster in input_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            picker_font_size,
            &cluster,
            text_x,
            text_y + input_line.baseline(),
            text_color,
            glyph_instances,
        );
    }

    if !picker.query.is_empty() {
        let caret_x = text_x + input_line.width() + 2.0 * stroke;
        quad_instances.push(QuadInstance {
            rect: [
                caret_x,
                top + stroke + (input_height - picker_font_size) * 0.5,
                2.0 * stroke,
                picker_font_size,
            ],
            color: color(theme.accent),
        });
    }

    // 5. Result rows
    let rows_top = top + input_height + 2.0 * stroke;
    let row_baseline = (row_height - picker_font_size) * 0.5;

    let start_idx = if picker.selected_index >= visible_count {
        picker.selected_index + 1 - visible_count
    } else {
        0
    };

    for (display_idx, &(_, lang_name)) in filtered
        .iter()
        .skip(start_idx)
        .take(visible_count)
        .enumerate()
    {
        let current_actual_idx = start_idx + display_idx;
        let is_selected = current_actual_idx == picker.selected_index;
        let row_y = rows_top + (display_idx as f32 * row_height);

        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [left + stroke, row_y, width - 2.0 * stroke, row_height],
                color: color(theme.chrome.panel_row),
            });
        }

        let label_x = left + 16.0 * scale_factor;
        let label_y = row_y + row_baseline;
        let label_line = editor_view
            .text_layout_mut()
            .layout_line(&style, lang_name, &[]);
        let label_color = if is_selected {
            color(theme.accent)
        } else {
            foreground
        };

        for cluster in label_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                label_x,
                label_y + label_line.baseline(),
                label_color,
                glyph_instances,
            );
        }
    }
}
