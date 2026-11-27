//! Workspace session persistence and native file watcher management (Phase 4 deliverable 3 & 7, Goal G14).
//! Persists open tabs, drafts, undo history, carets and selections, scroll,
//! split layout and folders across restarts (hot exit, ADR 0019).
//! Watches opened folders/files and classifies external disk changes into clean auto-reloads vs. dirty conflicts.

mod debounce;
pub(crate) mod disk_events;
pub(crate) mod location;
mod restore;
mod snapshot;

pub(crate) use disk_events::{mark_removed, poll_file_watcher};
pub(crate) use restore::restore_session;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lux_core::history::History;
use lux_workspace::watcher::FileWatcher;

use crate::app::App;
use crate::document::DocumentEntry;

#[derive(Default)]
pub(crate) struct WorkspaceSession {
    pub(crate) watcher: Option<FileWatcher>,
    /// Where session files live; `None` is the platform data directory.
    /// The file itself is never stored: it is derived from the workspace
    /// root at each write, so a write always lands in the session of the
    /// project that is open (ADR 0019).
    pub(crate) data_dir: Option<PathBuf>,
    /// Background project symbol index for `Cmd+Shift+R`.
    pub(crate) symbols: Option<lux_workspace::SymbolIndexer>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    /// Sidebar create, rename, duplicate, and trash jobs. Kept here so a
    /// result still reaches the tabs after the last folder closes.
    pub(crate) file_ops: crate::sidebar::worker::Jobs,
    /// Clean open files being read again after an external edit.
    pub(crate) reloads: disk_events::Reloads,
    /// Last measured frame. Written on move and resize, and used at quit
    /// if the window has already closed.
    pub(crate) frame: Option<lux_workspace::WindowFrame>,
    /// Each open buffer's history as last handed to the session writer,
    /// so an unchanged history is shared rather than cloned again.
    pub(crate) histories: HashMap<u64, Arc<History>>,
    /// A change was noted since the last session snapshot. Only set while
    /// the event loop runs (a waker is registered), like the debounce.
    unsaved: bool,
    /// Bumped whenever the watched folders change, so state derived from
    /// the open folders (build systems) knows to reload.
    folder_epoch: u64,
}

impl WorkspaceSession {
    /// A fresh watcher on `root`. The watch and its seeding scan run on a
    /// worker, and the waker fires once the watch is in place.
    pub(crate) fn init_watcher(&mut self, root: &Path) {
        self.folder_epoch += 1;
        self.symbols = Some(lux_workspace::SymbolIndexer::start(
            Some(root.to_path_buf()),
            self.waker.clone(),
        ));
        if let Ok(mut watcher) = FileWatcher::new(Duration::from_millis(150)) {
            if let Some(waker) = &self.waker {
                watcher.set_waker(Arc::clone(waker));
            }
            watcher.watch_detached(root);
            self.watcher = Some(watcher);
        }
    }

    /// `init_watcher` for tests: only the watcher wakes `wake`, so a test
    /// that waits for "the watch is in place" is not released early by the
    /// symbol indexer's first snapshot.
    #[cfg(test)]
    pub(crate) fn init_watcher_waking(&mut self, root: &Path, wake: Arc<dyn Fn() + Send + Sync>) {
        self.folder_epoch += 1;
        self.symbols = Some(lux_workspace::SymbolIndexer::start(
            Some(root.to_path_buf()),
            None,
        ));
        if let Ok(mut watcher) = FileWatcher::new(Duration::from_millis(150)) {
            watcher.set_waker(wake);
            watcher.watch_detached(root);
            self.watcher = Some(watcher);
        }
    }

    /// Adds or drops one more sidebar folder. Off the UI thread.
    pub(crate) fn watch_folder(&mut self, root: &Path, watch: bool) {
        self.folder_epoch += 1;
        match self.watcher.as_mut() {
            Some(watcher) if watch => watcher.watch_detached(root),
            Some(watcher) => watcher.unwatch_detached(root),
            None => {}
        }
    }

    /// Keeps the platform loop's waker so a later `init_watcher` still wakes
    /// the editor when a file changes. No timer: the debouncer thread calls it.
    pub(crate) fn register_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        debounce::SESSION.register_waker(Arc::clone(&waker));
        if let Some(watcher) = &mut self.watcher {
            watcher.set_waker(Arc::clone(&waker));
        }
        if let Some(symbols) = &self.symbols {
            symbols.set_waker(Arc::clone(&waker));
        }
        self.waker = Some(waker);
    }

    pub(crate) fn folder_epoch(&self) -> u64 {
        self.folder_epoch
    }

    /// The platform waker registered in `resumed`, if the window exists yet.
    pub(crate) fn waker(&self) -> Option<Arc<dyn Fn() + Send + Sync>> {
        self.waker.clone()
    }

    pub(crate) fn mark_dirty(&mut self, path: &Path) {
        if let Some(watcher) = &mut self.watcher {
            watcher.mark_dirty(path);
        }
    }

    pub(crate) fn mark_clean(&mut self, path: &Path) {
        if let Some(watcher) = &mut self.watcher {
            watcher.mark_clean(path);
        }
    }
}

/// Syntax tokens for a pane showing `entry` while it is not the active
/// document.
/// The session file `app` writes to.
#[cfg(test)]
pub(crate) fn current_file_for_tests(app: &App) -> std::path::PathBuf {
    snapshot::current_file(app)
}

pub(crate) fn pane_tokens(entry: &DocumentEntry) -> Vec<lux_syntax::Token> {
    let lang = entry
        .language
        .or_else(|| crate::document::detect_language(entry.path.as_deref(), &entry.buffer));
    let Some(lang) = lang else {
        return Vec::new();
    };
    let mut doc = lux_syntax::Document::new(lang);
    doc.set_rope(entry.buffer.rope());
    doc.highlight_rope(entry.buffer.rope())
}

/// The file to restore `root`'s session from, if it has one.
pub(crate) fn readable_file(app: &App, root: Option<&Path>) -> Option<PathBuf> {
    match &app.ui.session.data_dir {
        Some(dir) => location::readable_session_file_in(dir, root),
        None => location::readable_session_file(root),
    }
}

/// Writes the session now, on this thread. Only for the quit path.
pub(crate) fn save_session(app: &mut App, clean: bool) -> std::io::Result<()> {
    app.ui.session.unsaved = false;
    snapshot::write_now(app, clean)
}

/// Records a change worth persisting; the write follows once changes go
/// quiet (see `debounce`).
pub(crate) fn note_change(app: &mut App) {
    if app.ui.session.waker.is_some() {
        app.ui.session.unsaved = true;
    }
    debounce::SESSION.note_change();
}

/// Called before the folder that keys the session changes: another folder
/// or project opens, or the last folder closes. A change still inside the
/// quiet period belongs to the project being left, so its snapshot is
/// queued now, for that project's file. The session writer keeps one job
/// per file, so the next project's writes cannot replace it.
pub(crate) fn leave_root(app: &mut App) {
    if std::mem::take(&mut app.ui.session.unsaved) {
        snapshot::submit(app);
    }
}

/// Called from `background_event`: writes the session on the worker when
/// the quiet period after the last change has passed.
pub(crate) fn write_if_due(app: &mut App) {
    if debounce::SESSION.take_due() {
        app.ui.session.unsaved = false;
        snapshot::submit(app);
    }
}

#[cfg(test)]
mod tests;
