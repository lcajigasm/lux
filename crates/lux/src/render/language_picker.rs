//! Language picker. The list itself is the shared overlay.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::language_picker::LanguagePickerState;
use crate::overlay_list::centered_frame;
use crate::render::overlay_list::{OverlayItem, paint_overlay};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_language_picker(
    picker: &LanguagePickerState,
    viewport_width: f32,
    window_height: f32,
    tab_bar_height: f32,
    status_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
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
    let items: Vec<OverlayItem<'_>> = filtered
        .iter()
        .map(|item| OverlayItem {
            title: item.1,
            badge: None,
            detail: None,
            icon: None,
            swatches: None,
        })
        .collect();
    let frame = centered_frame(
        viewport_width,
        window_height,
        tab_bar_height,
        status_bar_height,
        scale_factor,
        items.len(),
        picker.selected_index,
    );
    paint_overlay(
        &frame,
        "Select a language…",
        picker.query.as_str(),
        &items,
        picker.selected_index,
        picker.selected_index as f32,
        0.0,
        scale_factor,
        stroke,
        font,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        theme,
        quad_instances,
        glyph_instances,
    );
}
