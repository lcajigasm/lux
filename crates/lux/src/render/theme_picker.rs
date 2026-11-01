//! Rendering logic for the floating Theme / Color Scheme Picker modal overlay (ST15).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, fuzzy_spans, overlay_text, push_glyph, span_covers};
use crate::theme_picker::ThemePickerState;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

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
    let _overlay = overlay_text();
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let filtered = picker.filtered();
    let visible_count = filtered.len().min(super::panel::THEME_VISIBLE);
    let open_offset = picker.motion.open_offset(now, reduced, -8.0 * scale_factor);
    let chrome = super::panel::paint_selector_chrome(
        quad_instances,
        viewport_width,
        tab_bar_height,
        scale_factor,
        stroke,
        visible_count,
        open_offset,
        theme,
    );
    let picker_font_size = font_size * 0.92;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: picker_font_size,
        ligatures: false,
    };
    let foreground = color(theme.foreground);
    let accent = color(theme.accent);

    let query_display = if picker.query.is_empty() {
        "Select a color scheme…".to_string()
    } else {
        picker.query.clone()
    };
    let query_color = if picker.query.is_empty() {
        super::panel::placeholder_ink(theme)
    } else {
        foreground
    };
    let input_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &query_display, &[]);
    let input_baseline = (chrome.input_height - picker_font_size) * 0.5;
    let input_text_x = chrome.left + 22.0 * scale_factor;
    let input_text_y = chrome.top + input_baseline;

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

    let scroll_offset = if picker.selected_index >= visible_count {
        picker.selected_index + 1 - visible_count
    } else {
        0
    };
    let row_height = chrome.row_height;
    let row_baseline = (row_height - picker_font_size) * 0.5;
    let rows_top = chrome.rows_top;
    let visual = picker.motion.row(picker.selected_index, now, reduced);
    super::panel::push_moving_row(
        quad_instances,
        chrome.left,
        rows_top,
        chrome.width,
        row_height,
        visual,
        scroll_offset,
        visible_count,
        super::panel::selected_fill(),
        accent,
        scale_factor,
    );

    for (row_idx, &theme_name) in filtered
        .iter()
        .skip(scroll_offset)
        .take(visible_count)
        .enumerate()
    {
        let item_idx = scroll_offset + row_idx;
        let is_selected = item_idx == picker.selected_index;
        let row_y = chrome.rows_top + (row_idx as f32 * row_height);
        let icon_size = 16.0 * scale_factor;
        let hole = if is_selected {
            super::panel::selected_fill()
        } else {
            super::panel::card_fill(theme)
        };
        super::icons::push_icon(
            super::icons::Icon::Theme,
            quad_instances,
            chrome.left + 16.0 * scale_factor,
            row_y + (row_height - icon_size) * 0.5,
            icon_size,
            accent,
            hole,
        );

        // Theme name text
        let is_active = theme_name.eq_ignore_ascii_case(theme.name);
        let name_color = foreground;

        let label_text = if is_active {
            format!("{theme_name}  (active)")
        } else {
            theme_name.to_string()
        };

        let item_layout = editor_view
            .text_layout_mut()
            .layout_line(&style, &label_text, &[]);
        let text_x = chrome.left + 16.0 * scale_factor + icon_size + 10.0 * scale_factor;
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
            let swatches_x = chrome.left + chrome.width - swatches_total_w - 18.0 * scale_factor;
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
