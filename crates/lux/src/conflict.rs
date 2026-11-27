//! External file conflict handling (Phase 4 deliverable 3 & Goal G14).
//! When a file is modified externally while unsaved edits exist in the editor,
//! a non-blocking conflict bar prompts the user to Compare Diff, Overwrite Disk, or Reload Disk.

use std::path::{Path, PathBuf};

use lux_core::buffer::Buffer;
use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};

use crate::app::App;
use crate::document::DocumentEntry;
use crate::session::disk_events::{Loaded, live_buffer, reload_tabs};
use crate::ui_io::Done;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConflictAction {
    CompareDiff,
    OverwriteDisk,
    ReloadDisk,
    Dismiss,
}

#[derive(Debug, Clone)]
pub(crate) struct ConflictButton {
    #[allow(dead_code)]
    pub(crate) label: &'static str,
    pub(crate) action: ConflictAction,
    pub(crate) rect: [f32; 4],
}

#[derive(Debug, Clone)]
pub(crate) struct ConflictPrompt {
    pub(crate) path: PathBuf,
    pub(crate) message: String,
    pub(crate) bar_rect: [f32; 4],
    pub(crate) buttons: Vec<ConflictButton>,
}

impl ConflictPrompt {
    pub(crate) fn new(path: PathBuf) -> Self {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("File");
        let message = format!("File \"{file_name}\" was modified externally on disk.");
        Self {
            path,
            message,
            bar_rect: [0.0, 0.0, 0.0, 0.0],
            buttons: Vec::new(),
        }
    }
}

/// What a conflict action's worker did, applied on the UI thread.
pub(crate) enum Resolved {
    /// The diff of the disk text against the buffer, ready for its tab.
    Diff { title: String, content: String },
    /// The buffer as written over the disk file, and its history state
    /// then, or why the write failed.
    Overwritten {
        path: PathBuf,
        written: Result<(Buffer, u64), String>,
    },
    /// The file as read back from disk, or why the read failed.
    Reloaded {
        path: PathBuf,
        loaded: Result<Loaded, String>,
    },
}

/// Runs the chosen action. The prompt closes at once; the disk read or
/// write runs on a `ui_io` worker and lands through [`apply_resolved`].
pub(crate) fn handle_conflict_action(
    app: &mut App,
    ctx: &dyn PlatformContext,
    action: ConflictAction,
) {
    let Some(prompt) = app.ui.dialogs.conflict_prompt.take() else {
        return;
    };
    app.scheduler.invalidate_all();
    let path = prompt.path;
    match action {
        ConflictAction::CompareDiff => {
            // The conflicting tab's text, which is `app.buffer` only when
            // that tab is the active one.
            let Some(buffer) = live_buffer(app, &path).cloned() else {
                return;
            };
            crate::ui_io::spawn(app, Some(ctx), move || {
                let disk_text = std::fs::read_to_string(&path).unwrap_or_default();
                Some(Done::Conflict(diff(&path, &disk_text, &buffer.text())))
            });
        }
        ConflictAction::OverwriteDisk => {
            let Some(version) = live_version(app, &path) else {
                return;
            };
            let Some(buffer) = live_buffer(app, &path).cloned() else {
                return;
            };
            crate::ui_io::spawn(app, Some(ctx), move || {
                let written = match buffer.save(&path) {
                    Ok(()) => Ok((buffer, version)),
                    Err(error) => Err(error.to_string()),
                };
                Some(Done::Conflict(Resolved::Overwritten { path, written }))
            });
        }
        ConflictAction::ReloadDisk => {
            let Some(old) = live_buffer(app, &path).cloned() else {
                return;
            };
            crate::ui_io::spawn(app, Some(ctx), move || {
                let loaded = Loaded::read(path.clone(), &old).map_err(|error| error.to_string());
                Some(Done::Conflict(Resolved::Reloaded { path, loaded }))
            });
        }
        ConflictAction::Dismiss => {}
    }
}

/// The history state of the tab on `path`.
fn live_version(app: &App, path: &Path) -> Option<u64> {
    let entry = app
        .documents
        .entries
        .iter()
        .find(|e| e.path.as_deref() == Some(path))?;
    Some(if app.documents.active_tab == Some(entry.tab_id) {
        app.history.state_id()
    } else {
        entry.history.state_id()
    })
}

/// Records that the disk now holds `buffer`, the tab's text at history
/// state `version`. The tab reads as saved only if it is still at that
/// state: text typed while the write ran stays modified.
fn note_overwritten(app: &mut App, path: &Path, buffer: Buffer, version: u64) {
    let active = app.documents.active_tab;
    let Some(entry) = app
        .documents
        .entries
        .iter_mut()
        .find(|e| e.path.as_deref() == Some(path))
    else {
        return;
    };
    entry.saved_version = version;
    entry.disk_snapshot = Some(buffer.clone());
    let current = if active == Some(entry.tab_id) {
        app.documents.saved_version = version;
        app.documents.disk_snapshot = Some(buffer);
        app.history.state_id()
    } else {
        entry.history.state_id()
    };
    if current == version {
        app.ui.session.mark_clean(path);
    }
    app.update_document_title();
}

/// Builds the diff tab's text. No disk access.
fn diff(path: &Path, disk_text: &str, buffer_text: &str) -> Resolved {
    let hunks = lux_git::diff_view::inline_diff(disk_text, buffer_text);
    let mut content = String::new();
    content.push_str(&format!("--- a/{}\n", path.display()));
    content.push_str(&format!("+++ b/{}\n", path.display()));
    for line in hunks {
        let mark = match line.kind {
            lux_git::diff_view::InlineDiffLineKind::Context => ' ',
            lux_git::diff_view::InlineDiffLineKind::Added => '+',
            lux_git::diff_view::InlineDiffLineKind::Removed => '-',
        };
        content.push(mark);
        content.push_str(&line.text);
        content.push('\n');
    }
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    Resolved::Diff {
        title: format!("Diff: {file_name}"),
        content,
    }
}

/// Applies a finished conflict action. Memory only.
pub(crate) fn apply_resolved(app: &mut App, resolved: Resolved) {
    match resolved {
        Resolved::Diff { title, content } => {
            if let Ok(diff_buffer) = Buffer::from_bytes(content.as_bytes()) {
                let tab_id = app.documents.tabs.open(&title);
                let entry = DocumentEntry::new(tab_id, None, diff_buffer);
                app.documents.entries.push(entry);
                app.switch_to_tab(tab_id);
            }
        }
        Resolved::Overwritten { path, written } => match written {
            Ok((buffer, version)) => note_overwritten(app, &path, buffer, version),
            Err(error) => {
                app.documents.message =
                    Some(format!("Cannot overwrite {}: {error}", path.display()));
            }
        },
        Resolved::Reloaded { path, loaded } => match loaded {
            Ok(loaded) if loaded.still_matches(app) => {
                reload_tabs(app, &loaded);
                app.ui.session.mark_clean(&path);
                app.update_document_title();
            }
            Ok(_) if live_buffer(app, &path).is_some() => {
                // Typed into while the file was being read: replacing the
                // text now would drop those keystrokes. Ask again.
                app.documents.message = Some(format!(
                    "{} changed while it was reloading; choose again",
                    path.display()
                ));
                if app.ui.dialogs.conflict_prompt.is_none() {
                    app.ui.dialogs.conflict_prompt = Some(ConflictPrompt::new(path));
                }
            }
            Ok(_) => {}
            Err(error) => {
                app.documents.message = Some(format!("Cannot reload {}: {error}", path.display()));
            }
        },
    }
    app.scheduler.invalidate_all();
}

pub(crate) fn handle_conflict_click(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    let Some(prompt) = &app.ui.dialogs.conflict_prompt else {
        return false;
    };

    let [bx, by, bw, bh] = prompt.bar_rect;
    if mouse_x < bx || mouse_x > bx + bw || mouse_y < by || mouse_y > by + bh {
        return false;
    }

    for btn in &prompt.buttons {
        let [x, y, w, h] = btn.rect;
        if mouse_x >= x && mouse_x <= x + w && mouse_y >= y && mouse_y <= y + h {
            let action = btn.action;
            handle_conflict_action(app, ctx, action);
            return true;
        }
    }

    true
}

pub(crate) fn handle_conflict_key(
    app: &mut App,
    ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    if app.ui.dialogs.conflict_prompt.is_none() {
        return false;
    }

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            handle_conflict_action(app, ctx, ConflictAction::Dismiss);
            true
        }
        Key::Character(s) if s.eq_ignore_ascii_case("d") => {
            handle_conflict_action(app, ctx, ConflictAction::CompareDiff);
            true
        }
        Key::Character(s) if s.eq_ignore_ascii_case("o") => {
            handle_conflict_action(app, ctx, ConflictAction::OverwriteDisk);
            true
        }
        Key::Character(s) if s.eq_ignore_ascii_case("r") => {
            handle_conflict_action(app, ctx, ConflictAction::ReloadDisk);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyContext;
    impl PlatformContext for DummyContext {
        fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
            std::sync::Arc::new(|| {})
        }
        fn background_dialogs(&self) -> std::sync::Arc<dyn lux_platform::FileDialogs> {
            unimplemented!()
        }
        fn create_window(
            &self,
            _: lux_platform::WindowAttributes,
        ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
            unimplemented!()
        }
        fn clipboard(&self) -> &dyn lux_platform::Clipboard {
            unimplemented!()
        }
        fn file_dialogs(&self) -> &dyn lux_platform::FileDialogs {
            unimplemented!()
        }
        fn exit(&self) {}
        fn request_wakeup(&self, _: std::time::Instant) {}
    }

    #[test]
    fn conflict_prompt_initialization() {
        let path = PathBuf::from("foo/bar.rs");
        let prompt = ConflictPrompt::new(path.clone());
        assert_eq!(prompt.path, path);
        assert!(prompt.message.contains("bar.rs"));
    }

    #[test]
    fn conflict_resolution_actions() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("conflict.txt");
        std::fs::write(&file_path, "disk content\n").unwrap();

        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        let buf = Buffer::from_bytes(b"local edited content\n").unwrap();
        let tab_id = app.documents.tabs.open("conflict.txt");
        let entry = DocumentEntry::new(tab_id, Some(file_path.clone()), buf.clone());
        app.documents.entries.push(entry);
        app.documents.active_tab = Some(tab_id);
        app.documents.path = Some(file_path.clone());
        app.buffer = buf;

        // Compare: the disk read and the diff run on a worker.
        app.ui.dialogs.conflict_prompt = Some(ConflictPrompt::new(file_path.clone()));
        let hold = app.ui.dialogs.jobs.hold();
        handle_conflict_action(&mut app, &DummyContext, ConflictAction::CompareDiff);
        assert!(app.ui.dialogs.conflict_prompt.is_none());
        assert_eq!(app.documents.tabs.tabs().len(), 2, "diffed inline");
        hold.release();
        crate::ui_io::finish_all(&mut app, &DummyContext);
        assert_eq!(app.documents.tabs.tabs().len(), 3);
        let diff_text = app.buffer.text();
        assert!(diff_text.contains("-disk content"));
        assert!(diff_text.contains("+local edited content"));

        // Overwrite: written on a worker.
        if let Some(entry) = app
            .documents
            .entries
            .iter_mut()
            .find(|e| e.path.as_ref() == Some(&file_path))
        {
            entry.buffer = Buffer::from_bytes(b"overwritten content\n").unwrap();
        }
        app.ui.dialogs.conflict_prompt = Some(ConflictPrompt::new(file_path.clone()));
        let hold = app.ui.dialogs.jobs.hold();
        handle_conflict_action(&mut app, &DummyContext, ConflictAction::OverwriteDisk);
        assert!(app.ui.dialogs.conflict_prompt.is_none());
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "disk content\n",
            "written inline"
        );
        hold.release();
        crate::ui_io::finish_all(&mut app, &DummyContext);
        let on_disk = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(on_disk, "overwritten content\n");

        // Reload: read on a worker.
        std::fs::write(&file_path, "new disk data\n").unwrap();
        app.ui.dialogs.conflict_prompt = Some(ConflictPrompt::new(file_path.clone()));
        let hold = app.ui.dialogs.jobs.hold();
        handle_conflict_action(&mut app, &DummyContext, ConflictAction::ReloadDisk);
        assert!(app.ui.dialogs.conflict_prompt.is_none());
        let entry_text = |app: &App| {
            app.documents
                .entries
                .iter()
                .find(|e| e.path.as_ref() == Some(&file_path))
                .unwrap()
                .buffer
                .text()
        };
        assert_eq!(entry_text(&app), "overwritten content\n", "read inline");
        hold.release();
        crate::ui_io::finish_all(&mut app, &DummyContext);
        assert_eq!(entry_text(&app), "new disk data\n");
    }

    /// Overwrite Disk on the active tab writes the text being edited, which
    /// lives in `app.buffer`, not the tab entry's copy from the last switch.
    #[test]
    fn overwrite_on_the_active_tab_writes_the_live_text() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("active.txt");
        std::fs::write(&file_path, "disk\n").unwrap();
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        let stale = Buffer::from_bytes(b"stale\n").unwrap();
        let tab_id = app.documents.tabs.open("active.txt");
        app.documents
            .entries
            .push(DocumentEntry::new(tab_id, Some(file_path.clone()), stale));
        app.documents.active_tab = Some(tab_id);
        app.documents.path = Some(file_path.clone());
        app.buffer = Buffer::from_bytes(b"live\n").unwrap();
        app.documents.saved_version = u64::MAX;

        app.ui.dialogs.conflict_prompt = Some(ConflictPrompt::new(file_path.clone()));
        handle_conflict_action(&mut app, &DummyContext, ConflictAction::OverwriteDisk);
        crate::ui_io::finish_all(&mut app, &DummyContext);
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "live\n");
        assert_eq!(app.buffer.text(), "live\n");
        assert_eq!(app.documents.saved_version, app.history.state_id());
    }
}
