//! Buffer context menu, painted with the other overlays.

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::edit_menu::{BufferContextMenu, EditMenuRow};
use crate::render::paint::{color, push_glyph};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_buffer_menu(
    menu: &BufferContextMenu,
    scale: f32,
    stroke: f32,
    font: FontRef<'static>,
    theme: &lux_ui::Theme,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let [x, y, w, h] = menu.rect(scale);
    let border = color(theme.chrome.border);
    let fill = color(theme.chrome.panel_input);
    quads.push(QuadInstance::solid(
        [x - stroke, y - stroke, w + 2.0 * stroke, h + 2.0 * stroke],
        border,
    ));
    quads.push(QuadInstance::solid([x, y, w, h], fill));
    let font_size = 13.0 * scale.max(1.0);
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };
    let enabled_ink = color(theme.foreground);
    let disabled_ink = color(theme.muted);
    for (rect, row) in menu.row_rects(scale).into_iter().zip(menu.rows.iter()) {
        match row {
            EditMenuRow::Separator => {
                let mid = rect[1] + rect[3] * 0.5;
                quads.push(QuadInstance::solid(
                    [
                        rect[0] + 8.0 * scale,
                        mid,
                        (rect[2] - 16.0 * scale).max(1.0),
                        stroke.max(1.0),
                    ],
                    border,
                ));
            }
            EditMenuRow::Action { action, enabled } => {
                let layout = editor_view
                    .text_layout_mut()
                    .layout_line(&style, action.label(), &[]);
                let top = rect[1] + (rect[3] - font_size) * 0.5;
                let ink = if *enabled { enabled_ink } else { disabled_ink };
                for cluster in layout.clusters() {
                    push_glyph(
                        atlas,
                        atlas_dirty,
                        scale_cx,
                        font,
                        font_size,
                        &cluster,
                        rect[0] + 12.0 * scale,
                        top + layout.baseline(),
                        ink,
                        glyphs,
                    );
                }
            }
        }
    }
}
