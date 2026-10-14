//! Rendering logic for the floating Snippet Picker modal overlay (`Snippets: Insert Snippet...`).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, push_glyph};
use crate::snippets::SnippetPickerState;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 9;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_snippet_picker(
    picker: &SnippetPickerState,
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
    let width = (viewport_width * 0.58).clamp(380.0 * scale_factor, 620.0 * scale_factor);
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
    let accent_color = [0.92, 0.65, 0.22, 1.0];

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

    // 3. Input background & separator
    quad_instances.push(QuadInstance {
        rect: [
            left + stroke,
            top + stroke,
            width - 2.0 * stroke,
            input_height,
        ],
        color: [0.065, 0.065, 0.085, 1.0],
    });
    quad_instances.push(QuadInstance {
        rect: [left, top + input_height + stroke, width, stroke],
        color: [0.18, 0.18, 0.22, 1.0],
    });

    // 4. Input text & caret
    let input_baseline = (input_height - picker_font_size) * 0.5;
    let text_x = left + 12.0 * scale_factor;
    let text_y = top + stroke + input_baseline;

    let (display_text, text_color) = if picker.query.is_empty() {
        ("Insert Snippet...", muted)
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

    // 5. Results list
    let rows_top = top + input_height + 2.0 * stroke;
    let row_baseline = (row_height - picker_font_size) * 0.5;
    let scroll_offset = if picker.selected_index >= MAX_VISIBLE_RESULTS {
        picker.selected_index - MAX_VISIBLE_RESULTS + 1
    } else {
        0
    };

    for (visible_idx, item_idx) in (scroll_offset..filtered.len())
        .take(MAX_VISIBLE_RESULTS)
        .enumerate()
    {
        let snippet = filtered[item_idx];
        let is_selected = item_idx == picker.selected_index;
        let row_y = rows_top + (visible_idx as f32 * row_height);

        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [left + stroke, row_y, width - 2.0 * stroke, row_height],
                color: [0.18, 0.22, 0.32, 0.95],
            });
            quad_instances.push(QuadInstance {
                rect: [left + stroke, row_y, 3.0 * scale_factor, row_height],
                color: [0.35, 0.65, 0.95, 1.0],
            });
        }

        // Render Trigger
        let label_x = left + 14.0 * scale_factor;
        let label_y = row_y + row_baseline;
        let trigger_text = format!("{:8}", snippet.trigger);
        let trigger_line = editor_view
            .text_layout_mut()
            .layout_line(&style, &trigger_text, &[]);
        for cluster in trigger_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                label_x,
                label_y + trigger_line.baseline(),
                accent_color,
                glyph_instances,
            );
        }

        // Render Description
        let desc_x = label_x + trigger_line.width() + 12.0 * scale_factor;
        let desc_text = if snippet.description.is_empty() {
            snippet.name.as_str()
        } else {
            snippet.description.as_str()
        };
        let desc_line = editor_view
            .text_layout_mut()
            .layout_line(&style, desc_text, &[]);
        for cluster in desc_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                desc_x,
                label_y + desc_line.baseline(),
                foreground,
                glyph_instances,
            );
        }

        // Render Scope Badge on the right
        if let Some(scope) = &snippet.scope {
            let scope_line = editor_view
                .text_layout_mut()
                .layout_line(&style, scope, &[]);
            let scope_x = left + width - scope_line.width() - 16.0 * scale_factor;
            for cluster in scope_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    picker_font_size,
                    &cluster,
                    scope_x,
                    label_y + scope_line.baseline(),
                    muted,
                    glyph_instances,
                );
            }
        }
    }
}
