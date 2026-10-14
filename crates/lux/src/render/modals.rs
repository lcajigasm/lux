//! Grouped modal overlays rendering: palette, goto, search, language picker, conflict, build picker.

use swash::FontRef;
use swash::scale::ScaleContext;

use lux_core::multicursor::MultiCursor;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::{EditorView, Viewport};

use crate::ui::UiOverlays;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_all_modals(
    ui: &mut UiOverlays,
    viewport: &Viewport,
    window_width: f32,
    sidebar_width: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    font_size: f32,
    font: FontRef<'static>,
    cursors: &MultiCursor,
    theme: &lux_ui::Theme,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    if let Some(pal) = &ui.command_palette {
        super::palette::render_command_palette(
            pal,
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
            font,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            theme,
            !cursors.primary().is_empty(),
            quad_instances,
            glyph_instances,
        );
    }

    if let Some(picker) = &ui.language_picker {
        super::language_picker::render_language_picker(
            picker,
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
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

    if let Some(goto_state) = &ui.goto_anything {
        super::goto::render_goto(
            goto_state,
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
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

    if let Some(picker) = &ui.build.picker {
        super::build::render_build_picker(
            picker,
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
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

    if let Some(picker) = &ui.project.picker {
        super::project_picker::render_project_picker(
            picker,
            ui.project.active_project_path.as_deref(),
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
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

    if let Some(picker) = &ui.theme_picker {
        super::theme_picker::render_theme_picker(
            picker,
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
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

    if let Some(picker) = &ui.snippets.picker {
        super::snippet_picker::render_snippet_picker(
            picker,
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
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

    if let Some(proj_search) = &ui.project_search {
        super::project_search::render_project_search(
            proj_search,
            window_width,
            tab_bar_height,
            scale_factor,
            stroke,
            font_size,
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

    if let Some(prompt) = &mut ui.conflict_prompt {
        super::conflict::render_conflict_bar(
            prompt,
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            viewport.width,
            sidebar_width,
            tab_bar_height,
            scale_factor,
            theme,
        );
    }
}
