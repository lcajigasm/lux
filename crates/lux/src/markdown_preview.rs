//! Toggle the Markdown preview for the active document.

use crate::app::App;

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
