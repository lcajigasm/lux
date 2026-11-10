//! Language picker, drawn with the same card as the command palette.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::language_picker::LanguagePickerState;
use crate::render::paint::{color, fuzzy_spans, push_bold_glyph, push_glyph, span_covers};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

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
    let filtered = picker.filtered();
    let visible_count = filtered.len().min(super::panel::SELECTOR_VISIBLE);
    let chrome = super::panel::paint_selector_chrome(
        quad_instances,
        viewport_width,
        tab_bar_height,
        scale_factor,
        stroke,
        visible_count,
        0.0,
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
    let query = picker.query.as_str();
    let (display, ink) = if query.is_empty() {
        ("Select a language…", super::panel::placeholder_ink(theme))
    } else {
        (query, foreground)
    };
    let input_line = editor_view
        .text_layout_mut()
        .layout_line(&style, display, &[]);
    let input_baseline = (chrome.input_height - picker_font_size) * 0.5;
    let text_x = chrome.left + 22.0 * scale_factor;
    let text_y = chrome.top + input_baseline;
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
            ink,
            glyph_instances,
        );
    }

    let start = if picker.selected_index >= visible_count {
        picker.selected_index + 1 - visible_count
    } else {
        0
    };
    if visible_count > 0 {
        super::panel::push_moving_row(
            quad_instances,
            chrome.left,
            chrome.rows_top,
            chrome.width,
            chrome.row_height,
            picker.selected_index as f32,
            start,
            visible_count,
            super::panel::selected_fill(theme),
            color(theme.accent),
            scale_factor,
        );
    }
    let row_baseline = (chrome.row_height - picker_font_size) * 0.5;
    let query = picker.query.as_str();
    for (offset, item) in filtered.iter().skip(start).take(visible_count).enumerate() {
        let row_y = chrome.rows_top + offset as f32 * chrome.row_height;
        let is_selected = start + offset == picker.selected_index;
        let line = editor_view
            .text_layout_mut()
            .layout_line(&style, item.1, &[]);
        let hits = fuzzy_spans(item.1, query);
        for cluster in line.clusters() {
            let hit = span_covers(&hits, &cluster.byte_range);
            let glyph_color = if hit {
                super::panel::hit_ink(is_selected, theme)
            } else {
                foreground
            };
            let draw = if hit && is_selected {
                push_bold_glyph
            } else {
                push_glyph
            };
            draw(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                text_x,
                row_y + row_baseline + line.baseline(),
                glyph_color,
                glyph_instances,
            );
        }
    }
}
