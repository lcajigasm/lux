//! Reading a session back (ADR 0019): tabs with their drafts, carets and
//! selections, scroll, undo history, and the split layout.

use std::path::Path;

use lux_core::buffer::Buffer;
use lux_core::history::codec;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_ui::tabs::TabId;
use lux_workspace::session::{CursorState, Session, TabState};
use lux_workspace::session_store::{self, LayoutState};

use crate::app::App;
use crate::document::DocumentEntry;
use crate::split::LayoutPreset;

/// Restores the session read from `read_from`. Later writes go to the
/// current location for the restored project (see
/// `snapshot::current_file`), even when `read_from` is a legacy file.
pub(crate) fn restore_session(app: &mut App, read_from: &Path) -> std::io::Result<bool> {
    let session = Session::load(read_from)?;
    if session.folders.is_empty() && session.tabs.is_empty() {
        return Ok(false);
    }
    let root = session.folders.first().filter(|f| f.exists());
    if let Some(folder) = root {
        crate::sidebar::reads::open_root(app, folder.clone());
        app.ui.session.init_watcher(folder);
    }
    let history_dir = session_store::history_dir(read_from);

    let had_initial_empty = app.documents.entries.len() == 1
        && app.documents.entries[0].path.is_none()
        && !app.documents.entries[0].is_dirty()
        && app.documents.entries[0].buffer.len_chars() == 0;
    let line_height = app.editor_view.metrics().line_height.max(1.0);

    // Session tab index -> restored tab, `None` when the tab's file is gone.
    let mut tab_ids: Vec<Option<TabId>> = Vec::with_capacity(session.tabs.len());
    for tab_state in &session.tabs {
        let entry = restore_tab(app, tab_state, &history_dir, line_height);
        tab_ids.push(entry.as_ref().map(|e| e.tab_id));
        if let Some(entry) = entry {
            if tab_state.pinned {
                app.documents.tabs.pin(entry.tab_id);
            }
            app.documents.entries.push(entry);
        }
    }
    if tab_ids.iter().all(Option::is_none) {
        app.scheduler.invalidate_all();
        return Ok(false);
    }

    if had_initial_empty {
        let first_tab = app.documents.entries[0].tab_id;
        app.documents.entries.remove(0);
        app.documents.tabs.close(first_tab);
    }
    let active = session
        .active_tab
        .and_then(|idx| tab_ids.get(idx).copied().flatten())
        .or_else(|| tab_ids.iter().flatten().next().copied());
    if let Some(active) = active {
        app.switch_to_tab(active);
        // Switching reveals the caret; the restored scroll wins, even when
        // the caret was scrolled out of view.
        if let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == active) {
            app.viewport.scroll_y = entry.scroll_y;
        }
    }
    if let Some(layout) = &session.layout {
        restore_layout(app, layout, &tab_ids, line_height);
    }
    app.scheduler.invalidate_all();
    Ok(true)
}

fn restore_tab(
    app: &mut App,
    tab_state: &TabState,
    history_dir: &Path,
    line_height: f32,
) -> Option<DocumentEntry> {
    let buffer = match (&tab_state.draft_text, &tab_state.path) {
        (Some(draft), _) => Buffer::from_bytes(draft.as_bytes()).ok()?,
        (None, Some(path)) if path.exists() => Buffer::open(path).ok()?,
        _ => return None,
    };
    let title = if !tab_state.title.is_empty() {
        tab_state.title.clone()
    } else if let Some(path) = &tab_state.path {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string()
    } else {
        "Untitled".to_string()
    };
    let tab_id = app.documents.tabs.open(&title);
    let mut entry = DocumentEntry::new(tab_id, tab_state.path.clone(), buffer);

    // A pathless tab stays Plain Text until the user picks a language.
    if tab_state.path.is_some()
        && let Some(lang_name) = &tab_state.language
        && let Some((id, _)) = crate::language_picker::all_languages()
            .into_iter()
            .find(|(_, name)| name == lang_name)
    {
        entry.language = id;
    }

    // The history only comes back when it was recorded against exactly
    // this text; a file changed on disk since then restores without undo.
    if let Some(history) = tab_state
        .history
        .as_deref()
        .and_then(|name| session_store::read_history(history_dir, name))
        .and_then(|bytes| codec::decode(&bytes, &entry.buffer))
    {
        entry.history = history;
    }
    // A restored dirty tab stays dirty even if undo walks back to the text
    // that was on disk: nothing proves the file still holds that text.
    entry.saved_version =
        if tab_state.is_dirty || (tab_state.path.is_none() && entry.buffer.len_chars() > 0) {
            u64::MAX
        } else {
            entry.history.state_id()
        };
    if let Some(cursors) = multi_cursor(&entry.buffer, &tab_state.cursors) {
        entry.cursors = cursors;
    }
    entry.scroll_y = tab_state.scroll_line as f32 * line_height;
    Some(entry)
}

/// Every caret with its selection, clamped to the restored text.
fn multi_cursor(buffer: &Buffer, cursors: &[CursorState]) -> Option<MultiCursor> {
    if cursors.is_empty() {
        return None;
    }
    let end = buffer.len_chars();
    let sels = cursors
        .iter()
        .map(|c| {
            let head = Position::from_char(buffer, c.char_offset.min(end));
            let anchor = c
                .anchor
                .map_or(head, |a| Position::from_char(buffer, a.min(end)));
            Selection::new(anchor, head)
        })
        .collect();
    Some(MultiCursor::from_cursors(sels))
}

fn parse_preset(name: &str) -> Option<LayoutPreset> {
    Some(match name {
        "Columns2" => LayoutPreset::Columns2,
        "Columns3" => LayoutPreset::Columns3,
        "Columns4" => LayoutPreset::Columns4,
        "Rows2" => LayoutPreset::Rows2,
        "Rows3" => LayoutPreset::Rows3,
        "Grid2x2" => LayoutPreset::Grid2x2,
        _ => return None,
    })
}

fn restore_layout(
    app: &mut App,
    layout: &LayoutState,
    tab_ids: &[Option<TabId>],
    line_height: f32,
) {
    let Some(preset) = parse_preset(&layout.preset) else {
        return;
    };
    let ratio = |millis: u32| (millis as f32 / 1000.0).clamp(0.1, 0.9);
    app.ui.split.ratio = ratio(layout.ratio_millis);
    app.ui.split.ratio_y = ratio(layout.ratio_y_millis);
    app.set_layout(preset);
    let count = app.ui.split.panes.len();
    for (idx, saved) in layout.panes.iter().enumerate().take(count) {
        let Some(tab_id) = tab_ids.get(saved.tab).copied().flatten() else {
            continue;
        };
        let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == tab_id) else {
            continue;
        };
        let cursors = multi_cursor(&entry.buffer, &saved.cursors);
        let tokens = super::pane_tokens(entry);
        let pane = &mut app.ui.split.panes[idx];
        pane.tab_id = tab_id;
        pane.active_tab = tab_id;
        if let Some(cursors) = cursors {
            pane.cursors = cursors;
        }
        pane.viewport.scroll_y = saved.scroll_line as f32 * line_height;
        pane.tokens = tokens;
    }
    let active = layout.active_pane.min(count.saturating_sub(1));
    if let Some(pane) = app.ui.split.panes.get(active).cloned() {
        // Load the pane straight into the editor. `focus_group` would first
        // copy the editor's current state over the restored pane.
        app.ui.split.active_pane = active;
        app.switch_to_tab(pane.tab_id);
        app.ui.split.panes[active] = pane.clone();
        app.cursors = pane.cursors;
        app.reset_goal_columns();
        app.viewport.scroll_y = pane.viewport.scroll_y;
    }
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
}
