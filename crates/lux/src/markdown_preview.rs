//! Toggle the Markdown preview for the active document.

use lux_syntax::{PreviewBlock, PreviewSpan};

use crate::app::App;

/// Block labels the probe prints. An edit shows up here on the next read
/// because the labels come from the current buffer.
pub(crate) fn summary(source: &str) -> String {
    let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Markdown);
    doc.set_text(source);
    let mut parts = Vec::new();
    for block in lux_syntax::preview_document(&doc, source) {
        match block {
            PreviewBlock::Heading { text, .. } => parts.push(format!("heading:{text}")),
            PreviewBlock::ListItem { text } => parts.push(format!("list:{}", list_body(&text))),
            PreviewBlock::Code { text, .. } => {
                let first = text.lines().next().unwrap_or("");
                parts.push(format!("code:{first}"));
            }
            PreviewBlock::Paragraph { spans }
                if spans
                    .iter()
                    .any(|span| matches!(span, PreviewSpan::Link { .. })) =>
            {
                parts.push("link".to_string());
            }
            _ => {}
        }
    }
    parts.join("|")
}

/// The marker is already drawn as a bullet. `- item` is `item`.
pub(crate) fn list_body(text: &str) -> &str {
    let trimmed = text.trim();
    trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
        .unwrap_or(trimmed)
}

pub(crate) fn toggle(app: &mut App) {
    let result = {
        let Some(entry) = app
            .documents
            .entries
            .iter_mut()
            .find(|entry| Some(entry.tab_id) == app.documents.active_tab)
        else {
            return;
        };
        if entry.language != Some(lux_syntax::LanguageId::Markdown) {
            Err("Markdown preview needs a Markdown file")
        } else {
            entry.markdown_preview = !entry.markdown_preview;
            Ok(entry.markdown_preview)
        }
    };
    app.documents.message = Some(match result {
        Ok(true) => "Markdown preview".to_string(),
        Ok(false) => "Markdown source".to_string(),
        Err(message) => message.to_string(),
    });
    app.scheduler.invalidate_all();
}
