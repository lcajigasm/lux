//! Document lifecycle. Disk and native dialogs run on a worker; completion
//! wakes the platform event loop, without a timer or recurring polling.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

mod io;

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
pub(crate) enum Request {
    Open(Option<PathBuf>),
    Save { save_as: bool },
    New,
    Close,
}

struct Pending {
    request: Request,
    version: u64,
}

#[derive(Clone)]
pub(crate) struct DocumentEntry {
    pub(crate) tab_id: TabId,
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
}

impl DocumentEntry {
    pub(crate) fn is_dirty(&self) -> bool {
        self.history.state_id() != self.saved_version
    }

    pub(crate) fn new(tab_id: TabId, path: Option<PathBuf>, buffer: Buffer) -> Self {
        let disk_snapshot = path.as_ref().map(|_| buffer.clone());
        Self {
            tab_id,
            path,
            disk_snapshot,
            buffer,
            cursors: MultiCursor::new(Selection::cursor(Position::document_start())),
            goal_columns: vec![0],
            history: History::new(),
            saved_version: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            snippet_session: None,
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
    close_when_idle: bool,
    pub(crate) initial_path: Option<PathBuf>,
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
            close_when_idle: false,
            initial_path: None,
        }
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
        let command = if cfg!(target_os = "macos") {
            self.modifiers.meta
        } else {
            self.modifiers.control && !self.modifiers.alt
        };
        if (self.modifiers.control || command) && matches!(&event.key, Key::Named(NamedKey::Tab)) {
            if self.modifiers.shift {
                self.switch_to_prev_tab();
            } else {
                self.switch_to_next_tab();
            }
            return true;
        }
        if !command {
            return false;
        }
        let Key::Character(key) = &event.key else {
            return false;
        };
        if self.modifiers.shift && key.eq_ignore_ascii_case("t") {
            return self.reopen_last_closed_tab();
        }
        let request = match key.to_lowercase().as_str() {
            "o" => Request::Open(None),
            "s" => Request::Save {
                save_as: self.modifiers.shift,
            },
            "n" => Request::New,
            "w" | "q" => Request::Close,
            _ => return false,
        };
        self.request_document(ctx, request);
        true
    }

    pub(crate) fn sync_active_document_to_entry(&mut self) {
        if let Some(active_id) = self.documents.active_tab {
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
        self.mouse.selection_anchor = None;
        self.ime = ImeComposition::new();
        self.reset_goal_columns();
        self.update_syntax_on_open();
        self.update_document_title();
        self.refresh_accessibility();
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
        true
    }

    pub(crate) fn cycle_tab(&mut self, forward: bool) -> bool {
        let tabs = self.documents.tabs.tabs();
        if tabs.len() <= 1 {
            return false;
        }
        let Some(pos) = tabs
            .iter()
            .position(|t| Some(t.id) == self.documents.active_tab)
        else {
            return false;
        };
        let next = if forward {
            (pos + 1) % tabs.len()
        } else {
            (pos + tabs.len() - 1) % tabs.len()
        };
        self.switch_to_tab(tabs[next].id)
    }

    pub(crate) fn switch_to_next_tab(&mut self) -> bool {
        self.cycle_tab(true)
    }

    pub(crate) fn switch_to_prev_tab(&mut self) -> bool {
        self.cycle_tab(false)
    }

    pub(crate) fn reopen_last_closed_tab(&mut self) -> bool {
        let Some(reopened_id) = self.documents.tabs.reopen_last_closed() else {
            return false;
        };
        let Some(mut entry) = self.documents.closed_entries.pop() else {
            return false;
        };
        entry.tab_id = reopened_id;
        self.documents.entries.push(entry);
        self.switch_to_tab(reopened_id)
    }

    pub(crate) fn close_tab(&mut self, ctx: &dyn PlatformContext, target_id: TabId) {
        if self.documents.active_tab == Some(target_id) {
            self.request_document(ctx, Request::Close);
            return;
        }
        let is_dirty = self
            .documents
            .entries
            .iter()
            .find(|e| e.tab_id == target_id)
            .is_some_and(|e| e.is_dirty());
        if is_dirty {
            self.switch_to_tab(target_id);
            self.request_document(ctx, Request::Close);
        } else {
            self.documents.tabs.close(target_id);
            if let Some(pos) = self
                .documents
                .entries
                .iter()
                .position(|e| e.tab_id == target_id)
            {
                let removed = self.documents.entries.remove(pos);
                self.documents.closed_entries.push(removed);
            }
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    pub(crate) fn request_document(&mut self, ctx: &dyn PlatformContext, request: Request) {
        if self.documents.pending.is_some() {
            if matches!(request, Request::Close) {
                self.documents.close_when_idle = true;
            }
            return;
        }
        let snapshot = Snapshot {
            buffer: self.buffer.clone(),
            path: self.documents.path.clone(),
            dirty: self.document_dirty(),
            disk: self.documents.disk_snapshot.clone(),
        };
        let version = self.history.checkpoint();
        self.documents.pending = Some(Pending {
            request: request.clone(),
            version,
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
            self.documents.pending = None;
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
            Ok(Outcome::Cancelled) => self.documents.close_when_idle = false,
            Err(error) => {
                self.documents.message = Some(error);
                self.documents.close_when_idle = false;
            }
            Ok(Outcome::Finished {
                saved,
                opened,
                close,
            }) => {
                if let Some((path, snapshot)) = saved {
                    if let Some(store) = recovery_store() {
                        let _ = store.clear(&path);
                    }
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
                        let is_untouched = self.documents.entries.len() == 1
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
                        } else {
                            self.sync_active_document_to_entry();
                            let tab_id = self.documents.tabs.open(title);
                            let entry = DocumentEntry::new(tab_id, path, buffer);
                            self.documents.entries.push(entry);
                            self.switch_to_tab(tab_id);
                        }
                    }
                }
                if close {
                    if self.documents.tabs.tabs().len() > 1 {
                        if let Some(active_id) = self.documents.active_tab {
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
                            self.documents.tabs.close(active.id);
                        }
                        ctx.exit();
                    }
                } else if std::mem::take(&mut self.documents.close_when_idle) {
                    self.request_document(ctx, Request::Close);
                }
            }
        }
        self.update_document_title();
        self.refresh_accessibility();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn update_document_title(&self) {
        let Some(gpu) = &self.gpu_window else {
            return;
        };
        let name = self
            .documents
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or("Untitled".into());
        let dirty = if self.document_dirty() { " *" } else { "" };
        let readonly = if self.buffer.is_huge_file_mode() {
            " [read-only: large file]"
        } else {
            ""
        };
        let status = if self.documents.pending.is_some() {
            " — File operation in progress".to_string()
        } else {
            self.documents
                .message
                .as_ref()
                .map(|m| format!(" — {m}"))
                .unwrap_or_default()
        };
        gpu.window
            .set_title(&format!("{name}{dirty}{readonly} — Lux{status}"));
    }
}

impl App {
    pub(crate) fn save_recovery_snapshot(&self) {
        if self.document_dirty()
            && let Some(path) = &self.documents.path
            && !self.buffer.is_huge_file_mode()
            && let Some(store) = recovery_store()
        {
            let _ = store.save(path, &self.buffer.text());
        }
    }

    pub(crate) fn recoverable_documents() -> Vec<PathBuf> {
        if let Some(store) = recovery_store()
            && let Ok(recoverable) = store.recoverable()
        {
            recoverable.into_iter().map(|s| s.path).collect()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests;
