//! HTML preview pane. The source stays in the editor; this paints the
//! heading, paragraph, link, and preformatted text beside it.

use swash::FontRef;

use lux_render::QuadInstance;
use lux_syntax::{PreviewBlock, PreviewSpan};

use crate::app::AtlasState;
use crate::render::paint::color;

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    buffer: &lux_core::buffer::Buffer,
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    font_size: f32,
    line_height: f32,
    pane_x: f32,
    pane_width: f32,
    editor_top: f32,
    viewport_height: f32,
    pad: f32,
    stroke: f32,
    theme: &lux_ui::Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<lux_render::GlyphInstance>,
) {
    quad_instances.push(QuadInstance::solid(
        [pane_x, editor_top, pane_width, viewport_height],
        color(theme.chrome.panel),
    ));
    quad_instances.push(QuadInstance::solid(
        [pane_x, editor_top, stroke, viewport_height],
        color(theme.chrome.border),
    ));
    let glyph_start = glyph_instances.len();
    let text_left = pane_x + pad;
    let inner = (pane_width - pad).max(1.0);
    let source = buffer.text();
    let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Html);
    doc.set_text(&source);
    let blocks = lux_syntax::preview_html(&doc, &source);
    let family = editor_view.family().to_string();
    let foreground = color(theme.foreground);
    let accent = color(theme.accent);
    let mut y = editor_top + line_height * 0.25;
    let bottom = editor_top + viewport_height;
    for block in blocks {
        if y > bottom {
            break;
        }
        match block {
            PreviewBlock::Heading { level, text } => {
                let size = font_size * (1.45 - f32::from(level.saturating_sub(1)) * 0.06);
                super::markdown::draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &text,
                    text_left,
                    y,
                    size,
                    foreground,
                    glyph_instances,
                );
                y += size * 1.6;
            }
            PreviewBlock::Paragraph { spans } => {
                let mut x = text_left;
                for span in &spans {
                    let (text, paint) = match span {
                        PreviewSpan::Link { label, .. } => (label.as_str(), accent),
                        PreviewSpan::Text(text)
                        | PreviewSpan::Strong(text)
                        | PreviewSpan::Emphasis(text)
                        | PreviewSpan::Code(text) => (text.as_str(), foreground),
                    };
                    x += super::markdown::draw_line(
                        editor_view,
                        atlas_state,
                        font,
                        &family,
                        text,
                        x,
                        y,
                        font_size,
                        paint,
                        glyph_instances,
                    );
                }
                y += line_height * 1.4;
            }
            PreviewBlock::Code { text, .. } => {
                for line in text.lines().take(8) {
                    super::markdown::draw_line(
                        editor_view,
                        atlas_state,
                        font,
                        &family,
                        line,
                        text_left,
                        y,
                        font_size,
                        accent,
                        glyph_instances,
                    );
                    y += line_height;
                }
            }
            PreviewBlock::ListItem { text } | PreviewBlock::Quote { text } => {
                super::markdown::draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &text,
                    text_left,
                    y,
                    font_size,
                    foreground,
                    glyph_instances,
                );
                y += line_height;
            }
            PreviewBlock::Rule => y += line_height,
        }
    }
    super::clip::clip_glyphs(
        glyph_instances,
        glyph_start,
        [text_left, editor_top, inner, viewport_height],
    );
}
