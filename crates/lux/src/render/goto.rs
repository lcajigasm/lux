//! Goto anything, including go to line. The list is the shared overlay.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::goto::{GotoItemKind, GotoState};
use crate::overlay_list::centered_frame;
use crate::render::overlay_list::{OverlayItem, paint_overlay};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_goto(
    goto: &mut GotoState,
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
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let placeholder = if goto.query.as_str().starts_with(':') {
        "Go to line…"
    } else if goto.mode == crate::goto::GotoMode::ProjectSymbols {
        "Go to symbol…"
    } else {
        "Go to file…"
    };
    let items: Vec<OverlayItem<'_>> = goto
        .items
        .iter()
        .map(|item| {
            let (icon, badge) = match &item.kind {
                GotoItemKind::File { .. } => (super::icons::Icon::File, None),
                GotoItemKind::Symbol { kind_badge, .. } => {
                    (super::icons::Icon::Symbol, Some(*kind_badge))
                }
                GotoItemKind::Term { .. } => (super::icons::Icon::Find, None),
            };
            OverlayItem {
                title: item.label.as_str(),
                badge,
                detail: if item.detail.is_empty() {
                    None
                } else {
                    Some(item.detail.as_str())
                },
                icon: Some(icon),
                swatches: None,
            }
        })
        .collect();
    let frame = centered_frame(
        viewport_width,
        window_height,
        tab_bar_height,
        status_bar_height,
        scale_factor,
        items.len(),
        goto.selected_index,
    );
    paint_overlay(
        &frame,
        placeholder,
        goto.query.as_str(),
        &items,
        goto.selected_index,
        goto.motion.row(goto.selected_index, now, reduced),
        goto.motion.open_offset(now, reduced, -8.0 * scale_factor),
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
    crate::overlay_motion::note_if_animating(&goto.motion, now);
}
