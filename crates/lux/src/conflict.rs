//! External file conflict handling (Phase 4 deliverable 3 & Goal G14).
//! When a file is modified externally while unsaved edits exist in the editor,
//! a non-blocking conflict bar prompts the user to Compare Diff, Overwrite Disk, or Reload Disk.

use std::path::PathBuf;

use lux_core::buffer::Buffer;
use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};

use crate::app::App;
use crate::document::DocumentEntry;

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

pub(crate) fn handle_conflict_action(
    app: &mut App,
    _ctx: &dyn PlatformContext,
    action: ConflictAction,
) {
    let Some(prompt) = app.ui.conflict_prompt.clone() else {
        return;
    };
    let path = prompt.path;

    match action {
        ConflictAction::CompareDiff => {
            let disk_text = std::fs::read_to_string(&path).unwrap_or_default();
            let buffer_text = app.buffer.text();

            let hunks = lux_git::diff_view::inline_diff(&disk_text, &buffer_text);
            let mut diff_content = String::new();
            diff_content.push_str(&format!("--- a/{}\n", path.display()));
            diff_content.push_str(&format!("+++ b/{}\n", path.display()));
            for line in hunks {
                match line.kind {
                    lux_git::diff_view::InlineDiffLineKind::Context => {
                        diff_content.push(' ');
                        diff_content.push_str(&line.text);
                        diff_content.push('\n');
                    }
                    lux_git::diff_view::InlineDiffLineKind::Added => {
                        diff_content.push('+');
                        diff_content.push_str(&line.text);
                        diff_content.push('\n');
                    }
                    lux_git::diff_view::InlineDiffLineKind::Removed => {
                        diff_content.push('-');
                        diff_content.push_str(&line.text);
                        diff_content.push('\n');
                    }
                }
            }

            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            let tab_title = format!("Diff: {file_name}");
            if let Ok(diff_buffer) = Buffer::from_bytes(diff_content.as_bytes()) {
                let tab_id = app.documents.tabs.open(&tab_title);
                let entry = DocumentEntry::new(tab_id, None, diff_buffer);
                app.documents.entries.push(entry);
                app.switch_to_tab(tab_id);
            }
            app.ui.conflict_prompt = None;
            app.scheduler.invalidate_all();
        }
        ConflictAction::OverwriteDisk => {
            if let Some(entry) = app
                .documents
                .entries
                .iter_mut()
                .find(|e| e.path.as_ref() == Some(&path))
                && let Ok(()) = entry.buffer.save(&path)
            {
                entry.saved_version = entry.history.state_id();
                entry.disk_snapshot = Some(entry.buffer.clone());
                if app.documents.active_tab == Some(entry.tab_id) {
                    app.buffer = entry.buffer.clone();
                }
            }
            app.ui.session.mark_clean(&path);
            app.ui.conflict_prompt = None;
            app.scheduler.invalidate_all();
        }
        ConflictAction::ReloadDisk => {
            if let Ok(new_buf) = Buffer::open(&path)
                && let Some(entry) = app
                    .documents
                    .entries
                    .iter_mut()
                    .find(|e| e.path.as_ref() == Some(&path))
            {
                entry.buffer = new_buf.clone();
                entry.saved_version = entry.history.state_id();
                entry.disk_snapshot = Some(new_buf.clone());
                if app.documents.active_tab == Some(entry.tab_id) {
                    app.buffer = new_buf;
                    app.update_syntax_on_open();
                }
            }
            app.ui.session.mark_clean(&path);
            app.ui.conflict_prompt = None;
            app.scheduler.invalidate_all();
        }
        ConflictAction::Dismiss => {
            app.ui.conflict_prompt = None;
            app.scheduler.invalidate_all();
        }
    }
}

pub(crate) fn handle_conflict_click(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    let Some(prompt) = &app.ui.conflict_prompt else {
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
    if app.ui.conflict_prompt.is_none() {
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

        // Prompt
        app.ui.conflict_prompt = Some(ConflictPrompt::new(file_path.clone()));

        // Test CompareDiff
        handle_conflict_action(&mut app, &DummyContext, ConflictAction::CompareDiff);
        assert!(app.ui.conflict_prompt.is_none());
        assert_eq!(app.documents.tabs.tabs().len(), 3);
        let diff_text = app.buffer.text();
        assert!(diff_text.contains("-disk content"));
        assert!(diff_text.contains("+local edited content"));

        // Reset prompt and test OverwriteDisk
        if let Some(entry) = app
            .documents
            .entries
            .iter_mut()
            .find(|e| e.path.as_ref() == Some(&file_path))
        {
            entry.buffer = Buffer::from_bytes(b"overwritten content\n").unwrap();
        }
        app.buffer = Buffer::from_bytes(b"overwritten content\n").unwrap();
        app.ui.conflict_prompt = Some(ConflictPrompt::new(file_path.clone()));
        handle_conflict_action(&mut app, &DummyContext, ConflictAction::OverwriteDisk);
        assert!(app.ui.conflict_prompt.is_none());
        let on_disk = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(on_disk, "overwritten content\n");

        // Test ReloadDisk
        std::fs::write(&file_path, "new disk data\n").unwrap();
        app.ui.conflict_prompt = Some(ConflictPrompt::new(file_path.clone()));
        handle_conflict_action(&mut app, &DummyContext, ConflictAction::ReloadDisk);
        assert!(app.ui.conflict_prompt.is_none());
        let entry = app
            .documents
            .entries
            .iter()
            .find(|e| e.path.as_ref() == Some(&file_path))
            .unwrap();
        assert_eq!(entry.buffer.text(), "new disk data\n");
    }
}
