//! Rendering logic for the floating Quick Switch Project modal overlay (ST14).

use std::path::Path;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::project::ProjectSwitchState;
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 9;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_project_picker(
    picker: &ProjectSwitchState,
    active_path: Option<&Path>,
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
    let width = (viewport_width * 0.52).clamp(380.0 * scale_factor, 640.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height + 12.0 * scale_factor;
    let header_height = 24.0 * scale_factor;
    let input_height = 32.0 * scale_factor;
    let row_height = 30.0 * scale_factor;

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

    let bg = color(theme.chrome.panel);
    let border_color = color(theme.chrome.border);
    let header_bg = color(theme.chrome.tab_bar);
    let sel_bg = color(theme.chrome.panel_row);
    let foreground = color(theme.foreground);
    let muted = color(theme.muted);
    let active_badge_color = [0.25, 0.75, 0.45, 1.0];

    // 1. Overlay main background
    quad_instances.push(QuadInstance {
        rect: [left, top, width, total_height],
        color: bg,
    });

    // 2. Outer border
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

    // Header bar
    quad_instances.push(QuadInstance {
        rect: [left, top, width, header_height],
        color: header_bg,
    });

    let header_line =
        editor_view
            .text_layout_mut()
            .layout_line(&small_style, "QUICK SWITCH PROJECT", &[]);
    let header_base = header_line.baseline();
    let header_y = top + (header_height - small_font_size) * 0.5 + header_base;
    for cluster in header_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            small_font_size,
            &cluster,
            left + 12.0 * scale_factor,
            header_y,
            muted,
            glyph_instances,
        );
    }

    // Input row
    let input_top = top + header_height;
    quad_instances.push(QuadInstance {
        rect: [
            left + 8.0 * scale_factor,
            input_top + 4.0 * scale_factor,
            width - 16.0 * scale_factor,
            input_height - 6.0 * scale_factor,
        ],
        color: color(theme.chrome.panel_input),
    });

    let prompt = "> ";
    let display_query = format!("{}{}", prompt, picker.query);
    let input_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &display_query, &[]);
    let input_base = input_line.baseline();
    let input_y = input_top + (input_height - picker_font_size) * 0.5 + input_base;
    for cluster in input_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            picker_font_size,
            &cluster,
            left + 16.0 * scale_factor,
            input_y,
            foreground,
            glyph_instances,
        );
    }

    // Input cursor
    let query_width = input_line.width();
    let cursor_x = left + 16.0 * scale_factor + query_width + 1.0 * scale_factor;
    quad_instances.push(QuadInstance {
        rect: [
            cursor_x,
            input_top + 8.0 * scale_factor,
            2.0 * scale_factor,
            input_height - 14.0 * scale_factor,
        ],
        color: color(theme.chrome.panel_row),
    });

    // Project results list
    let list_top = input_top + input_height + 2.0 * scale_factor;
    let start_idx = if picker.selected >= MAX_VISIBLE_RESULTS {
        picker.selected - MAX_VISIBLE_RESULTS + 1
    } else {
        0
    };

    for (i, &project_path) in filtered
        .iter()
        .skip(start_idx)
        .take(visible_count)
        .enumerate()
    {
        let actual_idx = start_idx + i;
        let is_selected = actual_idx == picker.selected;
        let row_y = list_top + (i as f32 * row_height);

        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [
                    left + 4.0 * scale_factor,
                    row_y,
                    width - 8.0 * scale_factor,
                    row_height - 2.0 * scale_factor,
                ],
                color: sel_bg,
            });
        }

        let is_active = active_path.is_some_and(|a| a == project_path);
        let project_name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project");

        // Project name
        let item_line = editor_view
            .text_layout_mut()
            .layout_line(&style, project_name, &[]);
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

        // Active indicator badge or checkmark
        if is_active {
            let badge_text = "[active]";
            let badge_line =
                editor_view
                    .text_layout_mut()
                    .layout_line(&small_style, badge_text, &[]);
            let badge_base = badge_line.baseline();
            let badge_x = left + 20.0 * scale_factor + item_line.width();
            let badge_y = row_y + (row_height - small_font_size) * 0.5 + badge_base;
            for cluster in badge_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    small_font_size,
                    &cluster,
                    badge_x,
                    badge_y,
                    active_badge_color,
                    glyph_instances,
                );
            }
        }

        // Directory path on the right
        let dir_str = project_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let dir_line = editor_view
            .text_layout_mut()
            .layout_line(&small_style, &dir_str, &[]);
        let dir_base = dir_line.baseline();
        let dir_x = left + width - 20.0 * scale_factor - dir_line.width();
        let dir_y = row_y + (row_height - small_font_size) * 0.5 + dir_base;
        if dir_x > left + 180.0 * scale_factor {
            for cluster in dir_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    small_font_size,
                    &cluster,
                    dir_x,
                    dir_y,
                    muted,
                    glyph_instances,
                );
            }
        }
    }
}
