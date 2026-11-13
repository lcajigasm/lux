//! Document lifecycle. Disk and dialogs run on a worker and wake the event loop.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

mod io;
pub(crate) mod open_queue;
mod save_ops;
pub(crate) mod tab_ops;

use io::{Outcome, Snapshot, execute, recovery_store};
use lux_core::{
    buffer::Buffer, history::History, multicursor::MultiCursor, position::Position,
    selection::Selection,
};
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, ImeComposition, Key, NamedKey, PlatformContext};
use lux_ui::tabs::TabId;

use crate::app::App;

#[derive(Clone)]
pub(crate) struct NamedSave {
    pub(crate) path: PathBuf,
    pub(crate) buffer: Buffer,
    pub(crate) disk: Option<Buffer>,
    pub(crate) state_id: u64,
    pub(crate) active: bool,
}

#[derive(Clone)]
pub(crate) enum Request {
    Open(Option<PathBuf>),
    Save { save_as: bool },
    SaveAll(Vec<NamedSave>),
    New,
    Close,
}

struct Pending {
    request: Request,
    version: u64,
    /// Undo on-save whitespace edits when the worker rejects the save.
    revert_transform: bool,
}

#[derive(Clone)]
pub(crate) struct DocumentEntry {
    pub(crate) tab_id: TabId,
    pub(crate) buffer_id: u64,
    pub(crate) path: Option<PathBuf>,
    pub(crate) buffer: Buffer,
    pub(crate) cursors: MultiCursor,
    pub(crate) goal_columns: Vec<usize>,
    pub(crate) history: History,
    pub(crate) saved_version: u64,
    pub(crate) disk_snapshot: Option<Buffer>,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
    pub(crate) snippet_session: Option<lux_core::snippets::SnippetSession>,
    pub(crate) language: Option<lux_syntax::LanguageId>,
    pub(crate) fold_state: lux_core::folding::FoldState,
    pub(crate) bookmarks: lux_core::bookmarks::BookmarkState,
    pub(crate) markdown_preview: bool,
}

pub(crate) fn detect_language(
    path: Option<&std::path::Path>,
    buffer: &Buffer,
) -> Option<lux_syntax::LanguageId> {
    let first = buffer.line_content(0);
    lux_syntax::LanguageId::detect(path, first.as_deref())
}

impl DocumentEntry {
    pub(crate) fn is_dirty(&self) -> bool {
        self.history.state_id() != self.saved_version
    }

    pub(crate) fn new(tab_id: TabId, path: Option<PathBuf>, buffer: Buffer) -> Self {
        static NEXT_BID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let buffer_id = NEXT_BID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let disk_snapshot = path.as_ref().map(|_| buffer.clone());
        let language = path
            .as_deref()
            .and_then(|p| detect_language(Some(p), &buffer));
        Self {
            tab_id,
            buffer_id,
            path,
            disk_snapshot,
            language,
            buffer,
            cursors: MultiCursor::new(Selection::cursor(Position::document_start())),
            goal_columns: vec![0],
            history: History::new(),
            saved_version: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            snippet_session: None,
            fold_state: lux_core::folding::FoldState::new(),
            bookmarks: lux_core::bookmarks::BookmarkState::new(),
            markdown_preview: false,
        }
    }
}

pub(crate) struct Documents {
    pub(crate) entries: Vec<DocumentEntry>,
    pub(crate) active_tab: Option<TabId>,
    pub(crate) closed_entries: Vec<DocumentEntry>,
    pub(crate) path: Option<PathBuf>,
    pub(crate) tabs: lux_ui::tabs::TabBar,
    pub(crate) saved_version: u64,
    pub(crate) disk_snapshot: Option<Buffer>,
    pending: Option<Pending>,
    sender: Sender<Result<Outcome, String>>,
    receiver: Receiver<Result<Outcome, String>>,
    pub(crate) message: Option<String>,
    /// Cmd+N or Cmd+W pressed while a worker is still finishing.
    follow_up: Option<Request>,
    pub(crate) initial_path: Option<PathBuf>,
    /// Files to open after `initial_path`, one at a time, as their own tabs.
    pub(crate) open_queue: Vec<PathBuf>,
    pub(crate) tab_context_menu: Option<tab_ops::TabContextMenu>,
    /// Tab being dragged, and the pointer x where the drag started.
    pub(crate) tab_drag: Option<(TabId, f32)>,
}

impl Default for Documents {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut tabs = lux_ui::tabs::TabBar::new();
        let tab_id = tabs.open("Untitled");
        let buffer = Buffer::from_bytes(b"").expect("empty document decodes");
        let initial_entry = DocumentEntry::new(tab_id, None, buffer);
        Self {
            entries: vec![initial_entry],
            active_tab: Some(tab_id),
            closed_entries: Vec::new(),
            path: None,
            tabs,
            saved_version: 0,
            disk_snapshot: None,
            pending: None,
            sender,
            receiver,
            message: None,
            follow_up: None,
            initial_path: None,
            open_queue: Vec::new(),
            tab_context_menu: None,
            tab_drag: None,
        }
    }
}

impl Documents {
    pub(crate) fn active_entry(&self) -> Option<&DocumentEntry> {
        let active_id = self.active_tab?;
        self.entries.iter().find(|e| e.tab_id == active_id)
    }

    pub(crate) fn language(&self) -> Option<lux_syntax::LanguageId> {
        self.entries
            .iter()
            .find(|e| Some(e.tab_id) == self.active_tab)
            .and_then(|e| e.language)
    }

    pub(crate) fn set_language(&mut self, language: Option<lux_syntax::LanguageId>) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| Some(e.tab_id) == self.active_tab)
        {
            entry.language = language;
        }
    }

    pub(crate) fn active_buffer_id(&self) -> Option<u64> {
        let active_id = self.active_tab?;
        self.entries
            .iter()
            .find(|e| e.tab_id == active_id)
            .map(|e| e.buffer_id)
    }

    pub(crate) fn buffer_id_for_tab(&self, tab_id: TabId) -> Option<u64> {
        self.entries
            .iter()
            .find(|e| e.tab_id == tab_id)
            .map(|e| e.buffer_id)
    }
}

impl App {
    pub(crate) fn document_dirty(&self) -> bool {
        self.history.state_id() != self.documents.saved_version
    }

    pub(crate) fn editing_blocked(&self) -> bool {
        self.documents
            .pending
            .as_ref()
            .is_some_and(|p| !matches!(p.request, Request::Save { .. }))
    }

    pub(crate) fn document_key(&mut self, ctx: &dyn PlatformContext, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed || self.ime.is_composing() {
            return false;
        }
        if self.ui.pending_chord.is_some() {
            return crate::input::dispatch::dispatch_keymap_or_chord(
                self,
                event,
                Some(ctx.clipboard()),
                Some(ctx),
            );
        }
        let command = if cfg!(target_os = "macos") {
            self.modifiers.meta && !self.modifiers.control && !self.modifiers.alt
        } else {
            self.modifiers.control && !self.modifiers.meta && !self.modifiers.alt
        };
        let tab_cycle = if cfg!(target_os = "macos") {
            (self.modifiers.control || self.modifiers.meta) && !self.modifiers.alt
        } else {
            self.modifiers.control && !self.modifiers.alt && !self.modifiers.meta
        };
        if tab_cycle && matches!(&event.key, Key::Named(NamedKey::Tab)) {
            if self.modifiers.shift {
                self.switch_to_prev_tab();
            } else {
                self.switch_to_next_tab();
            }
            return true;
        }
        if crate::edit_menu::dismiss_on_escape(self, event) {
            return true;
        }
        if !command {
            return false;
        }
        let Some(key) = crate::input::shortcut_letter(event) else {
            return false;
        };
        if self.modifiers.shift && key.eq_ignore_ascii_case("t") {
            return self.reopen_last_closed_tab();
        }
        if !self.modifiers.shift
            && let Some(digit) = key.chars().next().and_then(|c| c.to_digit(10))
            && (1..=9).contains(&digit)
        {
            if self.ui.split.is_active() && (1..=4).contains(&digit) && self.modifiers.control {
                return false;
            }
            if digit == 9 {
                return self.switch_to_last_tab();
            } else {
                return self.switch_to_tab_index((digit - 1) as usize);
            }
        }
        let request = match key.to_lowercase().as_str() {
            "o" if !self.modifiers.shift => Request::Open(None),
            "s" => Request::Save {
                save_as: self.modifiers.shift,
            },
            "n" if !self.modifiers.shift => Request::New,
            "w" | "q" if !self.modifiers.shift => Request::Close,
            _ => return false,
        };
        if !crate::menu::claim_document_request(&request) {
            return true;
        }
        self.request_document(ctx, request);
        true
    }

    pub(crate) fn sync_active_document_to_entry(&mut self) {
        if let Some(active_id) = self.documents.active_tab {
            let active_bid = self
                .documents
                .entries
                .iter()
                .find(|e| e.tab_id == active_id)
                .map(|e| e.buffer_id);

            if let Some(entry) = self
                .documents
                .entries
                .iter_mut()
                .find(|e| e.tab_id == active_id)
            {
                entry.path = self.documents.path.clone();
                entry.buffer = self.buffer.clone();
                entry.cursors = self.cursors.clone();
                entry.goal_columns = self.goal_columns.clone();
                entry.history = self.history.clone();
                entry.saved_version = self.documents.saved_version;
                entry.disk_snapshot = self.documents.disk_snapshot.clone();
                entry.scroll_x = self.viewport.scroll_x;
                entry.scroll_y = self.viewport.scroll_y;
                entry.snippet_session = self.snippet_session.clone();
                entry.fold_state = self.fold_state.clone();
                entry.bookmarks = self.bookmarks.clone();
            }

            if let Some(bid) = active_bid {
                for entry in self.documents.entries.iter_mut() {
                    if entry.tab_id != active_id && entry.buffer_id == bid {
                        entry.path = self.documents.path.clone();
                        entry.buffer = self.buffer.clone();
                        entry.history = self.history.clone();
                        entry.saved_version = self.documents.saved_version;
                        entry.disk_snapshot = self.documents.disk_snapshot.clone();
                    }
                }
            }
        }
    }

    pub(crate) fn switch_to_tab(&mut self, target_id: TabId) -> bool {
        if self.documents.active_tab == Some(target_id) {
            return true;
        }
        self.sync_active_document_to_entry();
        let Some(entry) = self
            .documents
            .entries
            .iter()
            .find(|e| e.tab_id == target_id)
            .cloned()
        else {
            return false;
        };
        self.documents.active_tab = Some(target_id);
        self.documents.tabs.activate(target_id);
        if self.ui.split.is_active() && self.ui.split.active_pane < self.ui.split.panes.len() {
            self.ui.split.panes[self.ui.split.active_pane].tab_id = target_id;
            self.ui.split.panes[self.ui.split.active_pane].active_tab = target_id;
            if self.ui.split.active_pane == 1 {
                self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
            }
        }
        self.documents.path = entry.path;
        self.buffer = entry.buffer;
        self.cursors = entry.cursors;
        self.goal_columns = entry.goal_columns;
        self.history = entry.history;
        self.documents.saved_version = entry.saved_version;
        self.documents.disk_snapshot = entry.disk_snapshot;
        self.viewport.scroll_x = entry.scroll_x;
        self.viewport.scroll_y = entry.scroll_y;
        self.snippet_session = entry.snippet_session;
        self.fold_state = entry.fold_state;
        self.bookmarks = entry.bookmarks;
        self.mouse.selection_anchor = None;
        self.ime = ImeComposition::new();
        self.reset_goal_columns();
        self.update_syntax_on_open();
        self.update_document_title();
        let tab_title = self
            .documents
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Untitled".into());
        crate::accessibility::announce(self, format!("Switched to tab {tab_title}"));
        crate::git::refresh_git_state(self);
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
        true
    }

    /// Drops on-save whitespace edits when the worker did not write the file.
    fn revert_rejected_save_transform(&mut self, pending: &Pending) {
        if !pending.revert_transform || self.history.state_id() != pending.version {
            return;
        }
        if let Some(selections) = self.history.undo(&mut self.buffer) {
            self.cursors = MultiCursor::from_cursors(selections);
            self.reset_goal_columns();
            self.sync_syntax_full();
        }
    }

    pub(crate) fn request_document(&mut self, ctx: &dyn PlatformContext, request: Request) {
        // An empty New has no disk work, so the tab exists before the next frame.
        let Some(request) = io::accept(self, request) else {
            self.update_document_title();
            return;
        };
        let state_before_save = self.history.state_id();
        if matches!(request, Request::Save { .. }) {
            self.apply_on_save_transforms();
        }
        let snapshot = Snapshot {
            buffer: self.buffer.clone(),
            path: self.documents.path.clone(),
            dirty: self.document_dirty(),
            disk: self.documents.disk_snapshot.clone(),
        };
        let version = self.history.checkpoint();
        let revert_transform = self.history.state_id() != state_before_save;
        self.documents.pending = Some(Pending {
            request: request.clone(),
            version,
            revert_transform,
        });
        self.documents.message = None;
        let sender = self.documents.sender.clone();
        let wake = ctx.background_waker();
        let dialogs = ctx.background_dialogs();
        let spawned = std::thread::Builder::new()
            .name("lux-document-io".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    execute(request, snapshot, dialogs.as_ref())
                }))
                .unwrap_or_else(|_| Err("File operation failed unexpectedly".into()));
                if sender.send(outcome).is_ok() {
                    wake();
                }
            });
        if let Err(error) = spawned {
            if let Some(pending) = self.documents.pending.take() {
                self.revert_rejected_save_transform(&pending);
            }
            self.documents.message = Some(format!("Cannot start file operation: {error}"));
        }
        self.update_document_title();
    }

    pub(crate) fn finish_document_operation(&mut self, ctx: &dyn PlatformContext) {
        let Ok(outcome) = self.documents.receiver.try_recv() else {
            return;
        };
        let Some(pending) = self.documents.pending.take() else {
            return;
        };
        self.presentation.set_hidden(false);
        self.scheduler.invalidate_all();
        match outcome {
            Ok(Outcome::Cancelled) => {
                self.revert_rejected_save_transform(&pending);
                crate::cli::note_open_failed(&pending.request);
                self.documents.follow_up = None;
            }
            Err(error) => {
                self.revert_rejected_save_transform(&pending);
                crate::cli::note_open_failed(&pending.request);
                self.documents.message = Some(error);
                self.documents.follow_up = None;
            }
            Ok(Outcome::Finished {
                saved,
                opened,
                close,
                saved_paths,
            }) => {
                self.mark_named_saves(&pending, &saved_paths);
                if let Some((path, snapshot)) = saved {
                    if let Some(store) = recovery_store() {
                        let _ = store.clear(&path);
                    }
                    self.ui.session.mark_clean(&path);
                    self.documents.path = Some(path.clone());
                    self.documents.disk_snapshot = Some(snapshot);
                    self.documents.saved_version = pending.version;
                    let title = path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("Untitled");
                    if let Some(id) = self.documents.active_tab {
                        self.documents.tabs.set_title(id, title);
                    }
                    self.sync_active_document_to_entry();
                    self.update_syntax_on_open();
                    crate::git::refresh_git_state(self);
                    if let Some(path) = self.documents.path.clone() {
                        crate::lsp::sync_document_save(self, &path);
                    }
                }
                if let Some((path, buffer)) = opened {
                    if let Some(ref p) = path
                        && let Some(existing) = self
                            .documents
                            .entries
                            .iter()
                            .find(|e| e.path.as_ref() == Some(p))
                    {
                        let id = existing.tab_id;
                        self.switch_to_tab(id);
                    } else {
                        let title = path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|f| f.to_str())
                            .unwrap_or("Untitled");
                        // The first real file replaces the untouched Untitled tab. New always adds one.
                        let is_untouched = !matches!(pending.request, Request::New)
                            && self.documents.entries.len() == 1
                            && self.documents.path.is_none()
                            && !self.document_dirty()
                            && self.buffer.text().is_empty();
                        if is_untouched {
                            let tab_id = self
                                .documents
                                .active_tab
                                .unwrap_or_else(|| self.documents.tabs.open(title));
                            self.documents.tabs.set_title(tab_id, title);
                            self.documents.disk_snapshot = path.as_ref().map(|_| buffer.clone());
                            self.documents.path = path;
                            self.buffer = buffer;
                            self.history = History::new();
                            self.documents.saved_version = 0;
                            self.cursors =
                                MultiCursor::new(Selection::cursor(Position::document_start()));
                            self.reset_goal_columns();
                            self.mouse.selection_anchor = None;
                            self.snippet_session = None;
                            self.ime = ImeComposition::new();
                            self.viewport.scroll_x = 0.0;
                            self.viewport.scroll_y = 0.0;
                            self.sync_active_document_to_entry();
                            self.update_syntax_on_open();
                            crate::git::refresh_git_state(self);
                        } else {
                            self.sync_active_document_to_entry();
                            let tab_id = self.documents.tabs.open(title);
                            let entry = DocumentEntry::new(tab_id, path, buffer);
                            self.documents.entries.push(entry);
                            self.switch_to_tab(tab_id);
                        }
                    }
                    if let Some(doc_path) = self.documents.path.clone() {
                        let text = self.buffer.text();
                        crate::lsp::sync_document_open(self, ctx, &doc_path, &text);
                    }
                    open_queue::note_opened(self);
                }
                if close {
                    if self.documents.tabs.tabs().len() > 1 {
                        if let Some(active_id) = self.documents.active_tab {
                            crate::cli::release_tab(active_id);
                            self.documents.tabs.close(active_id);
                            if let Some(pos) = self
                                .documents
                                .entries
                                .iter()
                                .position(|e| e.tab_id == active_id)
                            {
                                let removed = self.documents.entries.remove(pos);
                                self.documents.closed_entries.push(removed);
                            }
                            if let Some(new_active) = self.documents.tabs.active() {
                                let new_id = new_active.id;
                                self.documents.active_tab = None;
                                self.switch_to_tab(new_id);
                            }
                        }
                    } else {
                        if let Some(active) = self.documents.tabs.active() {
                            crate::cli::release_tab(active.id);
                            self.documents.tabs.close(active.id);
                        }
                        crate::cli::release_all();
                        ctx.exit();
                    }
                } else if let Some(next) = self.documents.follow_up.take() {
                    self.request_document(ctx, next);
                }
            }
        }
        crate::menu::note(self.documents.path.as_deref());
        self.update_document_title();
        open_queue::pump(self, ctx);
        self.refresh_accessibility();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod save_all_tests;
#[cfg(test)]
mod shortcut_tests;
#[cfg(test)]
mod tab_strip_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod whitespace_tests;
