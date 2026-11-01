//! Render pass for the project search & replace floating overlay.

use crate::project_search::{ProjectSearchState, SearchField};
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::{EditorView, Theme};
use swash::FontRef;
use swash::scale::ScaleContext;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_project_search(
    state: &ProjectSearchState,
    window_width: f32,
    tab_bar_height: f32,
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
    let modal_width = (640.0 * scale_factor).min(window_width - 32.0 * scale_factor);
    let modal_x = (window_width - modal_width) * 0.5;
    let modal_y = tab_bar_height + 24.0 * scale_factor;

    let row_h = 24.0 * scale_factor;
    let header_h = if state.is_replace {
        row_h * 4.0
    } else {
        row_h * 2.8
    };
    let max_visible_rows = 10;
    let list_h = (state.matches.len().min(max_visible_rows) as f32) * row_h;
    let modal_h = header_h + list_h + 16.0 * scale_factor;

    // Background shadow & modal quad
    quad_instances.push(QuadInstance::solid(
        [modal_x, modal_y, modal_width, modal_h],
        color(theme.chrome.panel),
    ));

    // Border outline
    quad_instances.push(QuadInstance::solid(
        [modal_x, modal_y, modal_width, stroke],
        color(theme.accent),
    ));

    let input_font_size = font_size * 0.95;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: input_font_size,
        ligatures: false,
    };

    // 1. Find Query Input Box
    let input_x = modal_x + 12.0 * scale_factor;
    let input_y = modal_y + 10.0 * scale_factor;
    let input_w = modal_width - 24.0 * scale_factor;
    let input_h = 24.0 * scale_factor;

    let find_bg = if state.focused_field == SearchField::Find {
        color(theme.chrome.panel_input)
    } else {
        color(theme.chrome.gutter)
    };
    quad_instances.push(QuadInstance::solid(
        [input_x, input_y, input_w, input_h],
        find_bg,
    ));

    let find_text = if state.find_query.is_empty() {
        "Find: Search in files...".to_string()
    } else {
        format!("Find: {}", state.find_query)
    };
    let find_color = if state.find_query.is_empty() {
        color(theme.muted)
    } else {
        color(theme.foreground)
    };

    let layout = editor_view
        .text_layout_mut()
        .layout_line(&style, &find_text, &[]);
    for cluster in layout.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            input_font_size,
            &cluster,
            input_x + 6.0 * scale_factor,
            input_y + (input_h - input_font_size) * 0.5 + layout.baseline(),
            find_color,
            glyph_instances,
        );
    }

    if state.focused_field == SearchField::Find {
        let caret_x = input_x + 6.0 * scale_factor + layout.width();
        quad_instances.push(QuadInstance::solid(
            [
                caret_x,
                input_y + 3.0 * scale_factor,
                2.0 * stroke,
                input_h - 6.0 * scale_factor,
            ],
            color(theme.accent),
        ));
    }

    // 2. Where Query Input Box
    let where_y = input_y + input_h + 6.0 * scale_factor;
    let where_bg = if state.focused_field == SearchField::Where {
        color(theme.chrome.panel_input)
    } else {
        color(theme.chrome.gutter)
    };
    quad_instances.push(QuadInstance::solid(
        [input_x, where_y, input_w, input_h],
        where_bg,
    ));

    let where_text = if state.where_query.is_empty() {
        "Where: (e.g. ./src, *.rs, -*.test.rs)".to_string()
    } else {
        format!("Where: {}", state.where_query)
    };
    let where_color = if state.where_query.is_empty() {
        color(theme.muted)
    } else {
        color(theme.foreground)
    };

    let where_layout = editor_view
        .text_layout_mut()
        .layout_line(&style, &where_text, &[]);
    for cluster in where_layout.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            input_font_size,
            &cluster,
            input_x + 6.0 * scale_factor,
            where_y + (input_h - input_font_size) * 0.5 + where_layout.baseline(),
            where_color,
            glyph_instances,
        );
    }

    if state.focused_field == SearchField::Where {
        let caret_x = input_x + 6.0 * scale_factor + where_layout.width();
        quad_instances.push(QuadInstance::solid(
            [
                caret_x,
                where_y + 3.0 * scale_factor,
                2.0 * stroke,
                input_h - 6.0 * scale_factor,
            ],
            color(theme.accent),
        ));
    }

    // 3. Replace Query Input Box (if replace mode)
    if state.is_replace {
        let replace_y = where_y + input_h + 6.0 * scale_factor;
        let replace_bg = if state.focused_field == SearchField::Replace {
            color(theme.chrome.panel_input)
        } else {
            color(theme.chrome.gutter)
        };
        quad_instances.push(QuadInstance::solid(
            [input_x, replace_y, input_w, input_h],
            replace_bg,
        ));

        let rep_text = if state.replace_query.is_empty() {
            "Replace: (Enter on Replace to apply)".to_string()
        } else {
            format!("Replace: {}", state.replace_query)
        };
        let rep_color = if state.replace_query.is_empty() {
            color(theme.muted)
        } else {
            color(theme.foreground)
        };

        let rep_layout = editor_view
            .text_layout_mut()
            .layout_line(&style, &rep_text, &[]);
        for cluster in rep_layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                input_font_size,
                &cluster,
                input_x + 6.0 * scale_factor,
                replace_y + (input_h - input_font_size) * 0.5 + rep_layout.baseline(),
                rep_color,
                glyph_instances,
            );
        }

        if state.focused_field == SearchField::Replace {
            let caret_x = input_x + 6.0 * scale_factor + rep_layout.width();
            quad_instances.push(QuadInstance::solid(
                [
                    caret_x,
                    replace_y + 3.0 * scale_factor,
                    2.0 * stroke,
                    input_h - 6.0 * scale_factor,
                ],
                color(theme.accent),
            ));
        }
    }

    // 3. Results List
    let list_y = modal_y + header_h;
    let item_style = lux_render::LineStyle {
        family: &family,
        font_size: font_size * 0.85,
        ligatures: false,
    };

    let start_idx = state
        .selected_index
        .saturating_sub(max_visible_rows.saturating_sub(1));
    let end_idx = (start_idx + max_visible_rows).min(state.matches.len());

    for (row_offset, match_idx) in (start_idx..end_idx).enumerate() {
        let is_selected = match_idx == state.selected_index;
        let y = list_y + row_offset as f32 * row_h;

        if is_selected {
            quad_instances.push(QuadInstance::solid(
                [input_x, y, input_w, row_h],
                [
                    theme.selection.r as f32 / 255.0,
                    theme.selection.g as f32 / 255.0,
                    theme.selection.b as f32 / 255.0,
                    0.4,
                ],
            ));
        }

        let matched = &state.matches[match_idx];
        let file_name = matched
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let row_label = format!(
            "{}:{}: {}",
            file_name,
            matched.line_number,
            matched.line.trim()
        );

        let label_color = if is_selected {
            color(theme.foreground)
        } else {
            color(theme.muted)
        };

        let row_layout = editor_view
            .text_layout_mut()
            .layout_line(&item_style, &row_label, &[]);
        for cluster in row_layout.clusters() {
            if cluster.x + 8.0 * scale_factor > input_w {
                break;
            }
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size * 0.85,
                &cluster,
                input_x + 8.0 * scale_factor,
                y + (row_h - font_size * 0.85) * 0.5 + row_layout.baseline(),
                label_color,
                glyph_instances,
            );
        }
    }
}
