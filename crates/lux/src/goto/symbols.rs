//! Goto Symbol in Project (`Cmd+Shift+R`) over the background index.
//!
//! The index is built and kept current by `lux_workspace::SymbolIndexer`
//! on its own thread. This module only swaps in the snapshot it publishes
//! and filters it, so a keystroke in the panel never reads a file or
//! parses one.

use lux_workspace::FileChange;

use super::{GotoItem, GotoItemKind, GotoMode};
use crate::app::App;

/// Rows ranked per keystroke. The panel shows a window of these.
pub(crate) const MAX_RESULTS: usize = 200;

/// Shown in the input while the first scan has not finished.
pub(crate) const INDEXING_NOTICE: &str = "Indexing…";
/// Shown when no folder and no saved file is open: nothing to index.
pub(crate) const NO_FOLDER_NOTICE: &str = "Open a folder to index symbols";

/// Runs as the panel opens. Asks the indexing thread to read the open
/// tabs' files, so a saved file outside the folder (or any file, when no
/// folder is open) is listed too. Only paths cross the channel; the
/// thread reads the files, and skips any whose mtime is unchanged.
pub(crate) fn index_open_documents(app: &mut App) {
    let mut paths: Vec<_> = app
        .documents
        .entries
        .iter()
        .filter_map(|e| e.path.clone())
        .collect();
    if let Some(path) = &app.documents.path
        && !paths.contains(path)
    {
        paths.push(path.clone());
    }
    let session = &mut app.ui.session;
    if session.symbols.is_none() {
        if paths.is_empty() {
            return;
        }
        session.symbols = Some(lux_workspace::SymbolIndexer::start(None, session.waker()));
    }
    if let Some(indexer) = &session.symbols {
        indexer.index_files(paths);
    }
}

/// Passes watcher changes to the indexing thread. No I/O here.
pub(crate) fn note_changes(app: &App, changes: &[FileChange]) {
    if let Some(indexer) = &app.ui.session.symbols {
        indexer.apply_changes(changes);
    }
}

/// Runs when the platform loop is woken. Takes the newest snapshot and,
/// if the panel is open on project symbols, re-filters it. The selected
/// symbol stays selected if it is still listed; if the selection lands on
/// a different symbol, the live preview follows it, so Enter always opens
/// what the preview shows.
pub(crate) fn poll(app: &mut App) {
    let changed = app
        .ui
        .session
        .symbols
        .as_mut()
        .is_some_and(|indexer| indexer.refresh());
    if !changed {
        return;
    }
    let Some(goto) = app.ui.goto_anything.as_ref() else {
        return;
    };
    if goto.mode != GotoMode::ProjectSymbols {
        return;
    }
    let previous = goto.items.get(goto.selected_index).cloned();
    let (items, notice) = items(app, &goto.query);
    let mut moved = false;
    if let Some(goto) = app.ui.goto_anything.as_mut() {
        goto.items = items;
        goto.notice = notice;
        let kept = previous
            .as_ref()
            .and_then(|prev| goto.items.iter().position(|it| it == prev));
        goto.selected_index =
            kept.unwrap_or_else(|| goto.selected_index.min(goto.items.len().saturating_sub(1)));
        moved = goto.items.get(goto.selected_index) != previous.as_ref();
    }
    if moved {
        super::update_live_preview(app);
    }
    app.scheduler.invalidate_all();
}

/// Filters the current snapshot. Returns the rows and the notice to show.
pub(crate) fn items(app: &App, query: &str) -> (Vec<GotoItem>, Option<&'static str>) {
    let Some(indexer) = &app.ui.session.symbols else {
        return (Vec::new(), Some(NO_FOLDER_NOTICE));
    };
    let snapshot = indexer.snapshot();
    let notice = snapshot.is_indexing().then_some(INDEXING_NOTICE);
    // The index filters names by substring of the part after `@`, if any.
    let trimmed = query.trim();
    let sym_part = trimmed
        .split_once('@')
        .map_or(trimmed, |(_, sym)| sym.trim());
    let rows = snapshot
        .search(query, MAX_RESULTS)
        .into_iter()
        .map(|s| {
            let path = s.path.to_string_lossy().to_string();
            GotoItem {
                hits: lux_search::fuzzy::substring_span(&s.name, sym_part)
                    .into_iter()
                    .collect(),
                label: s.name.clone(),
                detail: format!("{}:{}", path, s.line),
                path,
                line: Some(s.line),
                col: Some(s.column),
                kind: GotoItemKind::Symbol {
                    kind_badge: s.kind.badge(),
                    line: s.line,
                    col: s.column,
                },
            }
        })
        .collect();
    (rows, notice)
}
