//! Branch picker, drawn with the same card as the other selectors.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::git::branch_picker::BranchPicker;
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_branch_picker(
    picker: &BranchPicker,
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
    let rows = picker.rows();
    let visible_count = rows.len().clamp(1, super::panel::SELECTOR_VISIBLE);
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
    let query = picker.query.as_str();
    let (display, ink) = if query.is_empty() {
        (
            "Select or create a branch…",
            super::panel::placeholder_ink(theme),
        )
    } else {
        (query, color(theme.foreground))
    };
    let input_line = editor_view
        .text_layout_mut()
        .layout_line(&style, display, &[]);
    let text_x = chrome.left + 22.0 * scale_factor;
    let text_y = chrome.top + (chrome.input_height - picker_font_size) * 0.5;
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
    if !rows.is_empty() {
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
    for (offset, label) in rows.iter().skip(start).take(visible_count).enumerate() {
        let line = editor_view
            .text_layout_mut()
            .layout_line(&style, label, &[]);
        let row_y = chrome.rows_top + offset as f32 * chrome.row_height;
        for cluster in line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                picker_font_size,
                &cluster,
                text_x,
                row_y + row_baseline + line.baseline(),
                color(theme.foreground),
                glyph_instances,
            );
        }
    }
}
