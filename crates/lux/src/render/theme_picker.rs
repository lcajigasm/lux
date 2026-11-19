//! Theme picker. Swatches ride in the shared row's detail slot.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::overlay_list::centered_frame;
use crate::render::overlay_list::{OverlayItem, paint_overlay};
use crate::render::paint::color;
use crate::theme_picker::ThemePickerState;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_theme_picker(
    picker: &mut Option<ThemePickerState>,
    schemes: &[crate::color_scheme::LoadedScheme],
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
    let Some(picker) = picker.as_mut() else {
        return;
    };
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let filtered = picker.filtered();
    let labels: Vec<String> = filtered
        .iter()
        .map(|name| {
            if name.eq_ignore_ascii_case(theme.name) {
                format!("{name}  (active)")
            } else {
                (*name).to_string()
            }
        })
        .collect();
    let swatches: Vec<[[f32; 4]; 4]> = filtered
        .iter()
        .map(|name| {
            let target = crate::color_scheme::theme_for(schemes, name);
            [
                color(target.background),
                color(target.foreground),
                color(target.accent),
                color(target.selection),
            ]
        })
        .collect();
    let items: Vec<OverlayItem<'_>> = labels
        .iter()
        .zip(swatches.iter())
        .map(|(title, swatch)| OverlayItem {
            title,
            badge: None,
            detail: None,
            icon: Some(super::icons::Icon::Theme),
            swatches: Some(*swatch),
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
        "Select a color scheme…",
        picker.query.as_str(),
        &items,
        picker.selected_index,
        picker.motion.row(picker.selected_index, now, reduced),
        picker.motion.open_offset(now, reduced, -8.0 * scale_factor),
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
    crate::overlay_motion::note_if_animating(&picker.motion, now);
}
