//! What the file watcher's batches do to the open documents. Draining a
//! batch is memory only. A clean file edited elsewhere is read again on a
//! worker thread, and the new text replaces the tab's when that read
//! arrives, if the tab is still clean. The reload is recorded in the tab's
//! undo history as one replace, as Sublime Text does, so undo brings the
//! previous text back instead of replaying old edits onto the new text at
//! offsets that no longer match. A file removed from disk, by
//! another program or by the sidebar's Move to Trash, keeps its tab with
//! the unsaved text, marked modified; saving it writes the file again.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use lux_core::buffer::Buffer;
use lux_core::edit::{Edit, replace_edits};
use lux_core::history::{History, Transaction};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_workspace::watcher::{ChangeKind, FileChange};

use crate::app::App;
use crate::conflict::ConflictPrompt;

/// No history state has this id, so a tab holding it reads as modified
/// until it is saved.
const NOT_ON_DISK: u64 = u64::MAX;

struct Loaded {
    path: PathBuf,
    buffer: Buffer,
    /// Revision of the text the reload replaces, and the edits from that
    /// text to the new one, worked out on the reader thread.
    base: u64,
    edits: Vec<Edit>,
}

pub(crate) struct Reloads {
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    /// Tests: the next read waits for one message here before it starts.
    #[cfg(test)]
    pub(crate) hold: Option<Receiver<()>>,
}

impl Default for Reloads {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            #[cfg(test)]
            hold: None,
        }
    }
}

pub(crate) fn poll_file_watcher(app: &mut App) {
    crate::goto::symbols::poll(app);
    crate::build::systems::poll(app);
    let mut repaint_needed = apply_reloads(app);
    if let Some(batch) = app.ui.session.watcher.as_mut().map(|w| w.try_recv()) {
        if let Some(error) = batch.errors.first() {
            app.ui.announcement = Some(format!("File watcher: {error}"));
        }
        repaint_needed |= apply_changes(app, batch.changes);
    }
    if repaint_needed {
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
}

/// Applies one drained batch. `true` when something visible changed.
pub(crate) fn apply_changes(app: &mut App, changes: Vec<FileChange>) -> bool {
    crate::goto::symbols::note_changes(app, &changes);
    crate::git::note_disk_changes(app, changes.iter().map(|change| &change.path));
    crate::build::systems::note_disk_changes(app, &changes);
    let mut repaint_needed = false;
    for change in changes {
        match change.kind {
            ChangeKind::Reload => start_reload(app, change.path),
            ChangeKind::Conflict => {
                let is_dirty_open = app
                    .documents
                    .entries
                    .iter()
                    .any(|e| e.path.as_ref() == Some(&change.path) && e.is_dirty());
                if is_dirty_open {
                    app.ui.dialogs.conflict_prompt = Some(ConflictPrompt::new(change.path));
                    repaint_needed = true;
                }
            }
            ChangeKind::Created | ChangeKind::Removed => {
                repaint_needed |= crate::sidebar::watch::apply(app, &change);
                // A sidebar rename reports its source as removed. The tab
                // follows the rename when its result arrives instead.
                if change.kind == ChangeKind::Removed
                    && !app.ui.session.file_ops.renames.covers(&change.path)
                {
                    repaint_needed |= mark_removed(app, &change.path);
                }
            }
        }
    }
    repaint_needed
}

/// Whether the editor's live buffer is the document at `path`.
fn is_active(app: &App, path: &Path) -> bool {
    app.documents.path.as_deref() == Some(path)
        || app
            .documents
            .active_entry()
            .is_some_and(|entry| entry.path.as_deref() == Some(path))
}

fn is_clean(app: &App, path: &Path) -> bool {
    if is_active(app, path) && app.document_dirty() {
        return false;
    }
    app.documents
        .entries
        .iter()
        .filter(|entry| entry.path.as_deref() == Some(path))
        .all(|entry| !entry.is_dirty())
}

fn is_open(app: &App, path: &Path) -> bool {
    app.documents
        .entries
        .iter()
        .any(|entry| entry.path.as_deref() == Some(path))
}

/// Reads `path` on a worker thread. The UI thread returns at once.
fn start_reload(app: &mut App, path: PathBuf) {
    if !is_open(app, &path) || !is_clean(app, &path) {
        return;
    }
    // A rope clone shares structure, so handing the current text to the
    // reader is cheap; the diff against it runs there, not here.
    let old = if is_active(app, &path) {
        app.buffer.clone()
    } else {
        match app
            .documents
            .entries
            .iter()
            .find(|entry| entry.path.as_deref() == Some(path.as_path()))
        {
            Some(entry) => entry.buffer.clone(),
            None => return,
        }
    };
    let reloads = &mut app.ui.session.reloads;
    let tx = reloads.tx.clone();
    #[cfg(test)]
    let hold = reloads.hold.take();
    let wake = app.ui.session.waker();
    let _ = std::thread::Builder::new()
        .name("lux-reload".into())
        .spawn(move || {
            #[cfg(test)]
            if let Some(hold) = hold {
                let _ = hold.recv();
            }
            let Ok(buffer) = Buffer::open(&path) else {
                return;
            };
            let edits = if old.is_huge_file_mode() {
                Vec::new()
            } else {
                replace_edits(&old, &buffer)
            };
            let loaded = Loaded {
                path,
                buffer,
                base: old.revision(),
                edits,
            };
            if tx.send(loaded).is_ok()
                && let Some(wake) = wake
            {
                wake();
            }
        });
}

/// Puts finished reads into the tabs that are still clean.
fn apply_reloads(app: &mut App) -> bool {
    let loaded: Vec<Loaded> = app.ui.session.reloads.rx.try_iter().collect();
    let mut changed = false;
    for loaded in loaded {
        let path = loaded.path.as_path();
        if !is_clean(app, path) {
            continue;
        }
        let active = is_active(app, path);
        if active {
            record_reload(&mut app.buffer, &mut app.history, &mut app.cursors, &loaded);
            app.reset_goal_columns();
            app.documents.saved_version = app.history.state_id();
            app.documents.disk_snapshot = Some(loaded.buffer.clone());
        }
        let active_tab = app.documents.active_tab.filter(|_| active);
        for entry in app.documents.entries.iter_mut() {
            if entry.path.as_deref() != Some(path) {
                continue;
            }
            if Some(entry.tab_id) == active_tab {
                // The editor holds the live copy; keep the entry in step.
                entry.buffer = app.buffer.clone();
                entry.history = app.history.clone();
                entry.cursors = app.cursors.clone();
            } else {
                record_reload(
                    &mut entry.buffer,
                    &mut entry.history,
                    &mut entry.cursors,
                    &loaded,
                );
            }
            entry.saved_version = entry.history.state_id();
            entry.disk_snapshot = Some(loaded.buffer.clone());
        }
        if active {
            app.update_syntax_on_open();
        }
        crate::goto::preview::note_reload(app, path);
        changed = true;
    }
    changed
}

/// Swaps in the reloaded text and records the swap as one undo step, so
/// the history keeps matching the text it undoes. When the text changed
/// after the read started, the precomputed edits no longer describe it,
/// so the history is dropped instead: losing undo is safe, replaying it
/// onto other text is not.
fn record_reload(
    buffer: &mut Buffer,
    history: &mut History,
    cursors: &mut MultiCursor,
    loaded: &Loaded,
) {
    let new = &loaded.buffer;
    let before = cursors.cursors().to_vec();
    let after: Vec<Selection> = match loaded.edits.as_slice() {
        _ if buffer.revision() != loaded.base || buffer.is_huge_file_mode() => {
            *history = History::new();
            clamp_all(&before, new)
        }
        [] => before.clone(),
        edits => {
            let map = |sel: &Selection| map_selection(sel, edits, new);
            let after: Vec<Selection> = before.iter().map(map).collect();
            history.checkpoint();
            history.record(Transaction::new(edits.to_vec(), before, after.clone()));
            after
        }
    };
    *buffer = new.clone();
    *cursors = MultiCursor::from_cursors(after);
}

fn clamp_all(sels: &[Selection], buffer: &Buffer) -> Vec<Selection> {
    let end = buffer.len_chars();
    let at = |c: usize| Position::from_char(buffer, c.min(end));
    sels.iter()
        .map(|s| Selection::new(at(s.anchor.char), at(s.head.char)))
        .collect()
}

/// Moves a selection across the replaced span: carets before it stay,
/// carets after it shift by the length change, carets inside it clamp to
/// its new end.
fn map_selection(sel: &Selection, edits: &[Edit], buffer: &Buffer) -> Selection {
    let (start, removed, added) = edits
        .iter()
        .fold((usize::MAX, 0, 0), |(s, r, a), e| match e {
            Edit::Delete { char_idx, text } => ((*char_idx).min(s), text.chars().count(), a),
            Edit::Insert { char_idx, text } => ((*char_idx).min(s), r, text.chars().count()),
        });
    let map = |c: usize| {
        let c = if c <= start {
            c
        } else if c >= start + removed {
            c - removed + added
        } else {
            (start + added).min(c)
        };
        Position::from_char(buffer, c.min(buffer.len_chars()))
    };
    Selection::new(map(sel.anchor.char), map(sel.head.char))
}

/// Marks every tab on `path`, or below it, as modified with nothing on
/// disk to compare against, so a save writes the file again rather than
/// failing its on-disk check. `true` when a tab was marked.
pub(crate) fn mark_removed(app: &mut App, path: &Path) -> bool {
    let mut marked = false;
    for entry in app.documents.entries.iter_mut() {
        if entry.path.as_deref().is_some_and(|p| p.starts_with(path)) {
            entry.saved_version = NOT_ON_DISK;
            entry.disk_snapshot = None;
            marked = true;
        }
    }
    if app
        .documents
        .path
        .as_deref()
        .is_some_and(|p| p.starts_with(path))
    {
        app.documents.saved_version = NOT_ON_DISK;
        app.documents.disk_snapshot = None;
        marked = true;
    }
    if marked {
        app.update_document_title();
    }
    marked
}

#[cfg(test)]
mod tests;
