//! In-progress IME composition, drawn at the caret.
//!
//! Split out of `render.rs` so that file stays under the line limit.

use lux_platform::{ImeComposition, PlatformWindow};
use lux_render::{GlyphInstance, QuadInstance};
use swash::FontRef;

use super::paint::push_glyph;
use crate::app::AtlasState;

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_composition(
    ime: &ImeComposition,
    cursor_screen: Option<(f32, f32)>,
    window: &dyn PlatformWindow,
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    font_size: f32,
    line_height: f32,
    stroke: f32,
    foreground: [f32; 4],
    accent: [f32; 4],
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let Some((x, y)) = cursor_screen else {
        return;
    };
    if !ime.is_composing() {
        window.set_ime_cursor_area(
            dpi::PhysicalPosition::new(x as i32, y as i32),
            dpi::PhysicalSize::new((2.0 * stroke) as u32, line_height as u32),
        );
        return;
    }
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };
    let preedit_text = ime.preedit_text().to_string();
    let layout = editor_view
        .text_layout_mut()
        .layout_line(&style, &preedit_text, &[]);
    for cluster in layout.clusters() {
        push_glyph(
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            font_size,
            &cluster,
            x,
            y + layout.baseline(),
            foreground,
            glyph_instances,
        );
    }
    // A thin underline marks in-progress composition, the same
    // convention a desktop IME uses for uncommitted text.
    quad_instances.push(QuadInstance::solid(
        [
            x,
            y + line_height - 2.0 * stroke,
            layout.width().max(stroke),
            2.0 * stroke,
        ],
        accent,
    ));
    window.set_ime_cursor_area(
        dpi::PhysicalPosition::new(x as i32, y as i32),
        dpi::PhysicalSize::new(layout.width().max(stroke) as u32, line_height as u32),
    );
}
