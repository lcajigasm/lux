//! Quick switch project. The list is the shared overlay.

use std::path::Path;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::overlay_list::centered_frame;
use crate::project::ProjectSwitchState;
use crate::render::overlay_list::{OverlayItem, paint_overlay};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_project_picker(
    picker: &ProjectSwitchState,
    active_path: Option<&Path>,
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
    let titles: Vec<String> = filtered
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project")
                .to_string()
        })
        .collect();
    let details: Vec<String> = filtered
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let items: Vec<OverlayItem<'_>> = filtered
        .iter()
        .zip(titles.iter().zip(details.iter()))
        .map(|(path, (title, detail))| OverlayItem {
            title,
            badge: if active_path == Some(path.as_path()) {
                Some("active")
            } else {
                None
            },
            detail: Some(detail.as_str()),
            icon: Some(super::icons::Icon::Folder),
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
        picker.selected,
    );
    paint_overlay(
        &frame,
        "Switch project…",
        picker.query.as_str(),
        &items,
        picker.selected,
        picker.selected as f32,
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
