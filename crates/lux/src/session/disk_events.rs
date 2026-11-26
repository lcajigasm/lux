//! What the file watcher's batches do to the open documents. Draining a
//! batch is memory only. A clean file edited elsewhere is read again on a
//! worker thread, and the new text replaces the tab's when that read
//! arrives, if the tab is still clean. A file removed from disk, by
//! another program or by the sidebar's Move to Trash, keeps its tab with
//! the unsaved text, marked modified; saving it writes the file again.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use lux_core::buffer::Buffer;
use lux_workspace::watcher::{ChangeKind, FileChange};

use crate::app::App;
use crate::conflict::ConflictPrompt;

/// No history state has this id, so a tab holding it reads as modified
/// until it is saved.
const NOT_ON_DISK: u64 = u64::MAX;

struct Loaded {
    path: PathBuf,
    buffer: Buffer,
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
            if tx.send(Loaded { path, buffer }).is_ok()
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
    for Loaded { path, buffer } in loaded {
        if !is_clean(app, &path) {
            continue;
        }
        for entry in app.documents.entries.iter_mut() {
            if entry.path.as_deref() == Some(path.as_path()) {
                entry.buffer = buffer.clone();
                entry.saved_version = entry.history.state_id();
                entry.disk_snapshot = Some(buffer.clone());
            }
        }
        if is_active(app, &path) {
            app.buffer = buffer.clone();
            app.documents.saved_version = app.history.state_id();
            app.documents.disk_snapshot = Some(buffer);
            app.update_syntax_on_open();
        }
        changed = true;
    }
    changed
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
