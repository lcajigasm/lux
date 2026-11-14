//! Toggle the preview for the active Markdown or HTML document.

use crate::app::App;

pub(crate) fn toggle(app: &mut App) {
    let message = {
        let Some(entry) = app
            .documents
            .entries
            .iter_mut()
            .find(|entry| Some(entry.tab_id) == app.documents.active_tab)
        else {
            return;
        };
        match entry.language {
            Some(lux_syntax::LanguageId::Markdown) => {
                entry.markdown_preview = !entry.markdown_preview;
                if entry.markdown_preview {
                    "Markdown preview"
                } else {
                    "Markdown source"
                }
            }
            Some(lux_syntax::LanguageId::Html) => {
                entry.markdown_preview = !entry.markdown_preview;
                if entry.markdown_preview {
                    "HTML preview"
                } else {
                    "HTML source"
                }
            }
            _ => "Preview needs a Markdown or HTML file",
        }
    };
    app.documents.message = Some(message.to_string());
    app.scheduler.invalidate_all();
}

/// Labels the HTML probe prints. An edit shows up on the next read because
/// the labels come from the current buffer.
pub(crate) fn html_summary(source: &str) -> String {
    let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Html);
    doc.set_text(source);
    let mut parts = Vec::new();
    for block in lux_syntax::preview_html(&doc, source) {
        match block {
            lux_syntax::PreviewBlock::Heading { text, .. } => {
                parts.push(format!("heading:{text}"));
            }
            lux_syntax::PreviewBlock::Paragraph { spans }
                if spans
                    .iter()
                    .any(|span| matches!(span, lux_syntax::PreviewSpan::Link { .. })) =>
            {
                parts.push("link".to_string());
            }
            lux_syntax::PreviewBlock::Paragraph { spans } => {
                let text = span_text(&spans);
                if !text.is_empty() {
                    parts.push(format!("paragraph:{text}"));
                }
            }
            lux_syntax::PreviewBlock::Code { text, .. } => {
                let first = text.lines().next().unwrap_or("");
                parts.push(format!("code:{first}"));
            }
            _ => {}
        }
    }
    parts.join("|")
}

fn span_text(spans: &[lux_syntax::PreviewSpan]) -> String {
    spans
        .iter()
        .map(|span| match span {
            lux_syntax::PreviewSpan::Text(text)
            | lux_syntax::PreviewSpan::Strong(text)
            | lux_syntax::PreviewSpan::Emphasis(text)
            | lux_syntax::PreviewSpan::Code(text) => text.as_str(),
            lux_syntax::PreviewSpan::Link { label, .. } => label.as_str(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
