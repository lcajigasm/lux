//! Complete AccessKit Accessibility Tree & Screen-Reader Integration (G20).
//!
//! Exposes a full node hierarchy covering tabs, sidebar file tree, editor text
//! lines and selections, search bar, terminal, status bar, and live announcements.

use accesskit::TreeUpdate;
use lux_ui::{AccessibilityContext, build_full_window_tree};

use crate::app::{App, FONT_SIZE};

/// Builds the comprehensive application accessibility tree.
pub(crate) fn build_accessibility_tree(app: &mut App) -> TreeUpdate {
    let family = app.editor_view.family().to_string();
    let font_size = FONT_SIZE * app.scale_factor;
    let is_dirty = app.document_dirty();

    let active_id = app.documents.tabs.active().map(|t| t.id);
    let tabs: Vec<(&str, bool)> = app
        .documents
        .tabs
        .tabs()
        .iter()
        .map(|t| (t.title.as_str(), active_id == Some(t.id)))
        .collect();

    let sidebar_items = if let Some(sidebar) = &mut app.ui.sidebar
        && sidebar.visible
        && let Some(tree) = &mut sidebar.tree
    {
        let items: Vec<(&str, bool)> = tree
            .rows()
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    matches!(r.kind, lux_ui::file_tree::NodeKind::Directory),
                )
            })
            .collect();
        Some(items)
    } else {
        None
    };

    let lang = app
        .documents
        .path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .and_then(lux_syntax::LanguageId::from_extension);
    let sb = crate::status_bar::build_status_bar(&app.buffer, app.cursors.primary().head, lang);
    let status_telemetry = Some(format!(
        "{}  {}  {}  {}  {}",
        sb.cursor.label, sb.language.label, sb.encoding.label, sb.eol.label, sb.indent.label
    ));

    let (search_query, search_matches) = if let Some(search) = &app.ui.search_bar {
        let total = search.search_bar.matches().len();
        let current = search.search_bar.current_index().map_or(0, |i| i + 1);
        (
            Some(search.find_query.clone()),
            Some(format!("{current} of {total} matches")),
        )
    } else {
        (None, None)
    };

    let terminal_title = if app.ui.term.visible {
        app.ui
            .term
            .session_names
            .get(app.ui.term.active_tab)
            .cloned()
            .or_else(|| Some("Terminal".into()))
    } else {
        None
    };

    let doc_name = app
        .documents
        .path
        .as_ref()
        .map(|p| crate::document::tab_ops::window_document_label(p))
        .unwrap_or_else(|| "Untitled".into());
    let dirty_marker = if is_dirty { " *" } else { "" };
    let window_title = format!("{doc_name}{dirty_marker} — Lux");
    let announcement = app.ui.announcement.take();
    let focused_target = app.chrome.focus.current();

    let ctx = AccessibilityContext {
        window_title: &window_title,
        tabs,
        status_telemetry,
        sidebar_items,
        search_query,
        search_matches,
        terminal_title,
        announcement,
        focused_target,
    };

    build_full_window_tree(
        app.editor_view.text_layout_mut(),
        &family,
        font_size,
        &app.buffer,
        &app.cursors,
        &ctx,
    )
}

/// Queues an announcement to be spoken by platform screen readers (VoiceOver/NVDA/Orca)
/// and refreshes the accessibility tree.
pub(crate) fn announce(app: &mut App, message: impl Into<String>) {
    app.ui.announcement = Some(message.into());
    app.refresh_accessibility();
}

#[cfg(test)]
mod tests {
    #[test]
    fn announcer_message_queueing() {
        let ui = crate::ui::UiOverlays {
            announcement: Some("Testing announcement".into()),
            ..Default::default()
        };
        assert_eq!(ui.announcement, Some("Testing announcement".into()));
    }
}
