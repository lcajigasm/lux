//! Markdown preview painted from the buffer's parse tree.

use swash::FontRef;

use lux_render::QuadInstance;
use lux_syntax::{PreviewBlock, PreviewSpan};

use crate::app::AtlasState;
use crate::render::paint::{color, push_glyph};

#[allow(clippy::too_many_arguments)]
pub(super) fn render_preview(
    buffer: &lux_core::buffer::Buffer,
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    font_size: f32,
    line_height: f32,
    text_left: f32,
    editor_top: f32,
    viewport_height: f32,
    theme: &lux_ui::Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<lux_render::GlyphInstance>,
) {
    let source = buffer.text();
    let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Markdown);
    doc.set_text(&source);
    let blocks = lux_syntax::preview_document(&doc, &source);
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
                let size = font_size * (1.55 - f32::from(level.saturating_sub(1)) * 0.08);
                draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &text,
                    text_left,
                    y,
                    size,
                    accent,
                    glyph_instances,
                );
                y += size * 1.4 + line_height * 0.35;
            }
            PreviewBlock::Paragraph { spans } => {
                draw_spans(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &spans,
                    text_left,
                    y,
                    font_size,
                    foreground,
                    accent,
                    glyph_instances,
                );
                y += line_height * 1.4;
            }
            PreviewBlock::Code { info, text } => {
                let label = if info.is_empty() {
                    text
                } else {
                    format!("{info}\n{text}")
                };
                for line in label.lines() {
                    draw_line(
                        editor_view,
                        atlas_state,
                        font,
                        &family,
                        line,
                        text_left + font_size,
                        y,
                        font_size * 0.9,
                        foreground,
                        glyph_instances,
                    );
                    y += font_size * 1.3;
                }
                y += line_height * 0.3;
            }
            PreviewBlock::ListItem { text } => {
                draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &format!("• {text}"),
                    text_left,
                    y,
                    font_size,
                    foreground,
                    glyph_instances,
                );
                y += line_height;
            }
            PreviewBlock::Quote { text } => {
                quad_instances.push(QuadInstance::solid(
                    [text_left, y, 3.0, line_height],
                    accent,
                ));
                draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &text,
                    text_left + font_size,
                    y,
                    font_size,
                    foreground,
                    glyph_instances,
                );
                y += line_height;
            }
            PreviewBlock::Rule => {
                quad_instances.push(QuadInstance::solid(
                    [text_left, y + line_height * 0.4, 240.0, 1.0],
                    foreground,
                ));
                y += line_height;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_spans(
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    family: &str,
    spans: &[PreviewSpan],
    mut x: f32,
    y: f32,
    font_size: f32,
    foreground: [f32; 4],
    accent: [f32; 4],
    glyph_instances: &mut Vec<lux_render::GlyphInstance>,
) {
    for span in spans {
        let (text, paint) = match span {
            PreviewSpan::Text(text) => (text.as_str(), foreground),
            PreviewSpan::Strong(text) | PreviewSpan::Link { label: text, .. } => {
                (text.as_str(), accent)
            }
            PreviewSpan::Emphasis(text) | PreviewSpan::Code(text) => (text.as_str(), accent),
        };
        x += draw_line(
            editor_view,
            atlas_state,
            font,
            family,
            text,
            x,
            y,
            font_size,
            paint,
            glyph_instances,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    family: &str,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    paint: [f32; 4],
    glyph_instances: &mut Vec<lux_render::GlyphInstance>,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let style = lux_render::LineStyle {
        family,
        font_size,
        ligatures: false,
    };
    let line = editor_view.text_layout_mut().layout_line(&style, text, &[]);
    for cluster in line.clusters() {
        push_glyph(
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            font_size,
            &cluster,
            x,
            y + line.baseline(),
            paint,
            glyph_instances,
        );
    }
    line.width()
}
