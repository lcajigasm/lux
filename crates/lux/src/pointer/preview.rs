//! Hit testing for the Markdown and HTML preview panes.
//!
//! The vertical walk matches `render/markdown.rs` and `render/html_view.rs`
//! so the pointer sits on the glyphs those painters draw.

use lux_platform::CursorShape;
use lux_syntax::{PreviewBlock, PreviewSpan};

use crate::app::App;

struct Walk<'a> {
    app: &'a mut App,
    x: f32,
    y: f32,
    text_left: f32,
    bottom: f32,
    font_size: f32,
    line_height: f32,
}

pub(super) fn hit(app: &mut App, x: f32, y: f32, pane_x: f32) -> Option<CursorShape> {
    let html = app.documents.active_entry().is_some_and(|entry| {
        entry.markdown_preview && entry.language == Some(lux_syntax::LanguageId::Html)
    });
    let markdown = app.documents.active_entry().is_some_and(|entry| {
        entry.markdown_preview && entry.language == Some(lux_syntax::LanguageId::Markdown)
    });
    if !html && !markdown {
        return None;
    }
    let editor_top = app.tab_bar_height();
    let mut walk = Walk {
        x,
        y,
        text_left: pane_x + 16.0 * app.scale_factor,
        bottom: editor_top + app.viewport.height,
        font_size: app.editor_view.metrics().font_size,
        line_height: app.editor_view.metrics().line_height,
        app,
    };
    let source = walk.app.buffer.text().to_string();
    if html {
        let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Html);
        doc.set_text(&source);
        let blocks = lux_syntax::preview_html(&doc, &source);
        walk.html(&blocks, editor_top)
    } else {
        let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Markdown);
        doc.set_text(&source);
        let blocks = lux_syntax::preview_document(&doc, &source);
        walk.markdown(&blocks, editor_top)
    }
}

impl Walk<'_> {
    fn markdown(&mut self, blocks: &[PreviewBlock], editor_top: f32) -> Option<CursorShape> {
        let mut cursor_y = editor_top + self.line_height * 0.25;
        for block in blocks {
            if cursor_y > self.bottom {
                break;
            }
            match block {
                PreviewBlock::Heading { level, text } => {
                    let size = self.font_size * (1.55 - f32::from(level.saturating_sub(1)) * 0.08);
                    if let Some(hit) = self.run(self.text_left, cursor_y, text, size) {
                        return Some(hit);
                    }
                    cursor_y += size * 1.4 + self.line_height * 0.35;
                }
                PreviewBlock::Paragraph { spans } => {
                    if let Some(hit) = self.spans(self.text_left, cursor_y, self.font_size, spans) {
                        return Some(hit);
                    }
                    cursor_y += self.line_height * 1.4;
                }
                PreviewBlock::Code { text, .. } => {
                    let size = self.font_size * 0.9;
                    for line in text.lines() {
                        let left = self.text_left + self.font_size;
                        if let Some(hit) = self.run(left, cursor_y, line, size) {
                            return Some(hit);
                        }
                        cursor_y += size * 1.3;
                    }
                    cursor_y += self.line_height * 0.3;
                }
                PreviewBlock::ListItem { text } => {
                    let body = crate::markdown_preview::list_body(text);
                    let label = format!("• {body}");
                    if let Some(hit) = self.run(self.text_left, cursor_y, &label, self.font_size) {
                        return Some(hit);
                    }
                    cursor_y += self.line_height;
                }
                PreviewBlock::Quote { text } => {
                    let left = self.text_left + self.font_size;
                    if let Some(hit) = self.run(left, cursor_y, text, self.font_size) {
                        return Some(hit);
                    }
                    cursor_y += self.line_height;
                }
                PreviewBlock::Rule => cursor_y += self.line_height,
            }
        }
        None
    }

    fn html(&mut self, blocks: &[PreviewBlock], editor_top: f32) -> Option<CursorShape> {
        let mut cursor_y = editor_top + self.line_height * 0.25;
        for block in blocks {
            if cursor_y > self.bottom {
                break;
            }
            match block {
                PreviewBlock::Heading { level, text } => {
                    let size = self.font_size * (1.45 - f32::from(level.saturating_sub(1)) * 0.06);
                    if let Some(hit) = self.run(self.text_left, cursor_y, text, size) {
                        return Some(hit);
                    }
                    cursor_y += size * 1.6;
                }
                PreviewBlock::Paragraph { spans } => {
                    if let Some(hit) = self.spans(self.text_left, cursor_y, self.font_size, spans) {
                        return Some(hit);
                    }
                    cursor_y += self.line_height * 1.4;
                }
                PreviewBlock::Code { text, .. } => {
                    for line in text.lines().take(8) {
                        if let Some(hit) = self.run(self.text_left, cursor_y, line, self.font_size)
                        {
                            return Some(hit);
                        }
                        cursor_y += self.line_height;
                    }
                }
                PreviewBlock::ListItem { text } | PreviewBlock::Quote { text } => {
                    if let Some(hit) = self.run(self.text_left, cursor_y, text, self.font_size) {
                        return Some(hit);
                    }
                    cursor_y += self.line_height;
                }
                PreviewBlock::Rule => cursor_y += self.line_height,
            }
        }
        None
    }

    fn spans(
        &mut self,
        mut left: f32,
        top: f32,
        size: f32,
        spans: &[PreviewSpan],
    ) -> Option<CursorShape> {
        if self.y < top || self.y >= top + size {
            return None;
        }
        for span in spans {
            let (text, link) = match span {
                PreviewSpan::Link { label, .. } => (label.as_str(), true),
                PreviewSpan::Text(text)
                | PreviewSpan::Strong(text)
                | PreviewSpan::Emphasis(text)
                | PreviewSpan::Code(text) => (text.as_str(), false),
            };
            let width = self.measure(text, size);
            if self.x >= left && self.x < left + width {
                return Some(if link {
                    CursorShape::Hand
                } else {
                    CursorShape::Text
                });
            }
            left += width;
        }
        None
    }

    fn run(&mut self, left: f32, top: f32, text: &str, size: f32) -> Option<CursorShape> {
        if self.y < top || self.y >= top + size || text.is_empty() {
            return None;
        }
        let width = self.measure(text, size);
        if self.x >= left && self.x < left + width {
            Some(CursorShape::Text)
        } else {
            None
        }
    }

    fn measure(&mut self, text: &str, size: f32) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let family = self.app.editor_view.family().to_string();
        let style = lux_render::LineStyle {
            family: &family,
            font_size: size,
            ligatures: false,
        };
        self.app
            .editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[])
            .width()
    }
}
