//! Rendering logic for the floating Theme / Color Scheme Picker modal overlay (ST15).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, fuzzy_spans, push_glyph, span_covers};
use crate::theme_picker::ThemePickerState;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 7;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_theme_picker(
    picker: &mut ThemePickerState,
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
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let width = (viewport_width * 0.52).clamp(360.0 * scale_factor, 600.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height
        + 12.0 * scale_factor
        + picker.motion.open_offset(now, reduced, -8.0 * scale_factor);
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

    let radius = 8.0 * scale_factor;
    let border_color = color(theme.chrome.border);
    super::panel::push_rounded_surface(
        quad_instances,
        [left, top, width, total_height],
        radius,
        color(theme.chrome.panel),
        border_color,
        6.0 * scale_factor,
        stroke,
    );
    let well_inset = radius * 0.75;
    quad_instances.push(QuadInstance::rounded(
        [
            left + well_inset,
            top + well_inset * 0.4,
            width - 2.0 * well_inset,
            input_height - well_inset * 0.55,
        ],
        color(theme.chrome.panel_input),
        5.0 * scale_factor,
    ));

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

    let scroll_offset = if picker.selected_index >= MAX_VISIBLE_RESULTS {
        picker.selected_index - MAX_VISIBLE_RESULTS + 1
    } else {
        0
    };

    let row_baseline = (row_height - picker_font_size) * 0.5;
    let rows_top = top + input_height;
    let visual = picker.motion.row(picker.selected_index, now, reduced);
    super::panel::push_moving_row(
        quad_instances,
        left,
        rows_top,
        width,
        row_height,
        visual,
        scroll_offset,
        visible_count,
        color(theme.chrome.panel_row),
        accent,
        scale_factor,
    );

    for (row_idx, &theme_name) in filtered
        .iter()
        .skip(scroll_offset)
        .take(MAX_VISIBLE_RESULTS)
        .enumerate()
    {
        let item_idx = scroll_offset + row_idx;
        let is_selected = item_idx == picker.selected_index;
        let row_y = top + input_height + (row_idx as f32 * row_height);
        let icon_size = 14.0 * scale_factor;
        let hole = if is_selected {
            color(theme.chrome.panel_row)
        } else {
            color(theme.chrome.panel)
        };
        super::icons::push_icon(
            super::icons::Icon::Theme,
            quad_instances,
            left + 12.0 * scale_factor,
            row_y + (row_height - icon_size) * 0.5,
            icon_size,
            accent,
            hole,
        );

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
        let text_x = left + 12.0 * scale_factor + icon_size + 8.0 * scale_factor;
        let text_y = row_y + row_baseline;
        let hits = fuzzy_spans(&label_text, &picker.query);

        for cluster in item_layout.clusters() {
            let glyph_color = if span_covers(&hits, &cluster.byte_range) {
                accent
            } else {
                name_color
            };
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                text_x,
                text_y + item_layout.baseline(),
                glyph_color,
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
                quad_instances.push(QuadInstance::rounded(
                    [sx, swatch_y, swatch_size, swatch_size],
                    sc,
                    2.0 * scale_factor,
                ));
            }
        }
    }
    crate::overlay_motion::note_if_animating(&picker.motion, now);
}
