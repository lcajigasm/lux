//! The Font Family row's live preview: its description drawn with the
//! editor face and the editor's ligature setting, so picking a family or
//! toggling ligatures shows the result on the Settings page (ADR 0020).

use lux_render::{GlyphAtlas, GlyphInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::push_glyph;

/// The editor face as the settings page found it before switching to the
/// UI face for its own text.
pub(crate) struct EditorFace {
    pub(crate) font: FontRef<'static>,
    pub(crate) family: String,
    pub(crate) ligatures: bool,
}

impl EditorFace {
    pub(crate) fn of(editor_view: &EditorView, font: FontRef<'static>) -> Self {
        Self {
            font,
            family: editor_view.family().to_string(),
            ligatures: editor_view.metrics().ligatures,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_preview(
    face: &EditorFace,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font_size: f32,
    text: &str,
    x: f32,
    top: f32,
    ink: [f32; 4],
    glyphs: &mut Vec<GlyphInstance>,
) {
    let style = lux_render::LineStyle {
        family: &face.family,
        font_size,
        ligatures: face.ligatures,
    };
    let line = editor_view.text_layout_mut().layout_line(&style, text, &[]);
    let origin_y = top + line.baseline();
    for cluster in line.clusters() {
        let glyph_face = line.face(cluster.font_id).unwrap_or(face.font);
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            glyph_face,
            font_size,
            &cluster,
            x,
            origin_y,
            ink,
            glyphs,
        );
    }
}
