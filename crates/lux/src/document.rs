//! Document lifecycle. Disk and native dialogs run on a worker; completion
//! wakes the platform event loop, without a timer or recurring polling.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use lux_core::{
    buffer::Buffer, history::History, huge_file::HugeFilePolicy, multicursor::MultiCursor,
    position::Position, selection::Selection,
};
use lux_platform::dialog::SaveDecision;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key};
use lux_platform::{FileDialogOptions, FileDialogs, ImeComposition, PlatformContext};

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

pub(crate) struct Documents {
    pub(crate) path: Option<PathBuf>,
    pub(crate) tabs: lux_ui::tabs::TabBar,
    saved_version: u64,
    disk_snapshot: Option<Buffer>,
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
        Self {
            path: None,
            tabs: lux_ui::tabs::TabBar::new(),
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

enum Outcome {
    Cancelled,
    Finished {
        saved: Option<(PathBuf, Buffer)>,
        opened: Option<(Option<PathBuf>, Buffer)>,
        close: bool,
    },
}

struct Snapshot {
    buffer: Buffer,
    path: Option<PathBuf>,
    dirty: bool,
    disk: Option<Buffer>,
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
        let command = if cfg!(target_os = "macos") {
            self.modifiers.meta
        } else {
            self.modifiers.control && !self.modifiers.alt
        };
        if !command || event.state != ElementState::Pressed || self.ime.is_composing() {
            return false;
        }
        let Key::Character(key) = &event.key else {
            return false;
        };
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
        // Native modal loops may occlude the surface without delivering a
        // matching winit Occluded(false). Completion is an explicit reason
        // to try acquisition again; if still hidden the GPU will say so.
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
                    self.documents.path = Some(path);
                    self.documents.disk_snapshot = Some(snapshot);
                    self.documents.saved_version = pending.version;
                    self.update_syntax_on_open();
                }
                if let Some((path, buffer)) = opened {
                    let title = path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|f| f.to_str())
                        .unwrap_or("Untitled");
                    self.documents.tabs.open(title);
                    self.documents.disk_snapshot = path.as_ref().map(|_| buffer.clone());
                    self.documents.path = path;
                    self.buffer = buffer;
                    self.history = History::new();
                    self.documents.saved_version = 0;
                    self.cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
                    self.mouse.selection_anchor = None;
                    self.snippet_session = None;
                    self.ime = ImeComposition::new();
                    self.viewport.scroll_x = 0.0;
                    self.viewport.scroll_y = 0.0;
                    self.reset_goal_columns();
                    self.update_syntax_on_open();
                }
                if close {
                    if let Some(active) = self.documents.tabs.active() {
                        let id = active.id;
                        self.documents.tabs.close(id);
                    }
                    ctx.exit();
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

fn save_snapshot(
    snapshot: &Snapshot,
    save_as: bool,
    dialogs: &dyn FileDialogs,
) -> Result<Option<(PathBuf, Buffer)>, String> {
    if snapshot.buffer.is_huge_file_mode() {
        return Err("Large file is read-only".into());
    }
    let path = if save_as || snapshot.path.is_none() {
        dialogs.save_file(&FileDialogOptions {
            title: Some("Save document".into()),
            ..Default::default()
        })
    } else {
        snapshot.path.clone()
    };
    let Some(path) = path else {
        return Ok(None);
    };
    if snapshot.path.as_ref() == Some(&path)
        && let Some(expected) = &snapshot.disk
    {
        let current = Buffer::open(&path)
            .map_err(|e| format!("Cannot check {} before saving: {e}", path.display()))?;
        if current.text() != expected.text() || current.encoding() != expected.encoding() {
            return Err(
                "File changed on disk; save to a different path to preserve both versions".into(),
            );
        }
    }
    snapshot
        .buffer
        .save(&path)
        .map_err(|e| format!("Cannot save {}: {e}", path.display()))?;
    if let Some(store) = recovery_store() {
        let _ = store.clear(&path);
    }
    Ok(Some((path, snapshot.buffer.clone())))
}

fn execute(
    request: Request,
    snapshot: Snapshot,
    dialogs: &dyn FileDialogs,
) -> Result<Outcome, String> {
    if let Request::Save { save_as } = request {
        return Ok(match save_snapshot(&snapshot, save_as, dialogs)? {
            Some(saved) => Outcome::Finished {
                saved: Some(saved),
                opened: None,
                close: false,
            },
            None => Outcome::Cancelled,
        });
    }
    let open_path = if let Request::Open(path) = &request {
        let selected = path
            .clone()
            .or_else(|| dialogs.pick_file(&FileDialogOptions::default()));
        let Some(path) = selected else {
            return Ok(Outcome::Cancelled);
        };
        Some(path)
    } else {
        None
    };
    // Load before saving the old document: an invalid open must leave the
    // old document and its on-disk baseline unchanged.
    let mut opened = if let Some(path) = open_path {
        let buffer = Buffer::open_with_policy(&path, HugeFilePolicy::default())
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        Some((Some(path), buffer))
    } else if matches!(request, Request::New) {
        Some((
            None,
            Buffer::from_bytes(b"").expect("empty document decodes"),
        ))
    } else {
        None
    };
    let mut saved = None;
    if snapshot.dirty {
        let name = snapshot
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or("Untitled".into());
        match dialogs.confirm_save(&name) {
            SaveDecision::Cancel => return Ok(Outcome::Cancelled),
            SaveDecision::Discard => {}
            SaveDecision::Save => {
                saved = save_snapshot(&snapshot, false, dialogs)?;
                if saved.is_none() {
                    return Ok(Outcome::Cancelled);
                }
            }
        }
    }
    // Reopening the document just saved (including a symlink alias) must
    // display the saved snapshot, not the older pre-confirmation read.
    if let (Some((saved_path, saved_buffer)), Some((Some(open_path), opened_buffer))) =
        (&saved, &mut opened)
        && let (Ok(saved_target), Ok(open_target)) = (
            std::fs::canonicalize(saved_path),
            std::fs::canonicalize(open_path),
        )
        && saved_target == open_target
    {
        *opened_buffer = saved_buffer.clone();
    }
    Ok(Outcome::Finished {
        saved,
        opened,
        close: matches!(request, Request::Close),
    })
}

fn recovery_store() -> Option<lux_workspace::session::RecoveryStore> {
    let dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir)
        .join("lux");
    Some(lux_workspace::session::RecoveryStore::new(dir))
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
