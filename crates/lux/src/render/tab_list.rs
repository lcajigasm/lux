//! The `▼` list of open tabs, painted on the shared overlay card. The
//! active tab's badge is `›`; a dirty tab's badge carries `●`.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::overlay_list::{ListFrame, centered_frame};
use crate::render::overlay_list::{OverlayItem, paint_overlay};
use crate::tab_strip::list::{TabListView, badge};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

/// Paints the list and returns the frame it painted, so a test can check it
/// against the frame hit-testing uses.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_tab_list(
    list: &TabListView,
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
) -> ListFrame {
    let items: Vec<OverlayItem<'_>> = list
        .rows
        .iter()
        .map(|row| OverlayItem {
            title: row.title.as_str(),
            badge: badge(row),
            detail: None,
            icon: None,
            swatches: None,
            hits: row.hits.clone(),
        })
        .collect();
    let frame = centered_frame(
        viewport_width,
        window_height,
        tab_bar_height,
        status_bar_height,
        scale_factor,
        items.len(),
        list.selected,
    );
    paint_overlay(
        &frame,
        "Open tabs…",
        list.query.as_str(),
        &items,
        list.selected,
        list.selected as f32,
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
    frame
}

#[cfg(test)]
#[path = "tab_list_tests.rs"]
mod tests;
