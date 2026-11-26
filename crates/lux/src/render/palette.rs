//! Command palette. Rows, shortcuts, and the scrollbar come from the shared overlay.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::overlay_list::centered_frame;
use crate::palette::CommandPaletteState;
use crate::render::overlay_list::{OverlayItem, paint_overlay};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::command_palette::Context;
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_command_palette(
    palette: &mut CommandPaletteState,
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
    has_selection: bool,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let mut context = Context::new();
    context.set("editorFocus", true);
    context.set("hasSelection", has_selection);
    let visual = palette.motion.row(palette.selected_index, now, reduced);
    let open_offset = palette
        .motion
        .open_offset(now, reduced, -8.0 * scale_factor);
    let results = palette.results(&context);
    let items: Vec<OverlayItem<'_>> = results
        .iter()
        .map(|entry| OverlayItem {
            title: entry.command.title.as_str(),
            badge: None,
            detail: palette.shortcuts.get(&entry.command.id).map(String::as_str),
            icon: Some(super::icons::icon_for_category(&entry.command.category)),
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
        palette.selected_index,
    );
    paint_overlay(
        &frame,
        "Execute a command…",
        palette.query.as_str(),
        &items,
        palette.selected_index,
        visual,
        open_offset,
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
    crate::overlay_motion::note_if_animating(&palette.motion, now);
}

#[cfg(test)]
mod tests {
    use super::super::panel::push_rounded_surface;

    #[test]
    fn a_card_shadow_is_a_soft_halo() {
        let cases = [
            ([0.93, 0.91, 0.84, 1.0], 0.22_f32),
            ([0.16, 0.16, 0.14, 1.0], 0.30),
        ];
        for (fill, cap) in cases {
            let mut quads = Vec::new();
            push_rounded_surface(
                &mut quads,
                [10.0, 20.0, 400.0, 300.0],
                12.0,
                fill,
                fill,
                40.0,
                2.0,
            );
            let shadows: Vec<_> = quads.iter().filter(|quad| quad.color[3] < 0.5).collect();
            assert!(shadows.len() >= 3, "{}", shadows.len());
            assert!(shadows.iter().all(|quad| quad.color[3] <= cap));
        }
    }
}
