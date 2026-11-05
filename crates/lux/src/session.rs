//! Workspace session persistence and native file watcher management (Phase 4 deliverable 3 & 7, Goal G14).
//! Persists open tabs, active tab, cursors, scroll offsets, and folders across restarts.
//! Watches opened folders/files and classifies external disk changes into clean auto-reloads vs. dirty conflicts.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_workspace::session::{CursorState, SESSION_FILE_NAME, Session, TabState};
use lux_workspace::watcher::{ChangeKind, FileWatcher};

use crate::app::App;
use crate::conflict::ConflictPrompt;
use crate::document::DocumentEntry;

#[derive(Default)]
pub(crate) struct WorkspaceSession {
    pub(crate) watcher: Option<FileWatcher>,
    pub(crate) session_path: Option<PathBuf>,
    pub(crate) symbol_index: lux_workspace::SymbolIndex,
}

impl WorkspaceSession {
    pub(crate) fn init_watcher(&mut self, root: &Path) {
        self.symbol_index.set_root(root);
        if let Ok(mut watcher) = FileWatcher::new(Duration::from_millis(150)) {
            let _ = watcher.watch(root);
            self.watcher = Some(watcher);
        }
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

pub(crate) fn default_session_path(root: Option<&Path>) -> PathBuf {
    if let Some(root) = root {
        let dot_project = root.join(".lux-project");
        if dot_project.exists() {
            return dot_project;
        }
        root.join(SESSION_FILE_NAME)
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(std::env::temp_dir)
            .join("lux")
            .join("last-session.json")
    }
}

pub(crate) fn save_session(app: &App, clean: bool) -> std::io::Result<()> {
    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|s| s.workspace_root.as_deref());

    let session_path = app
        .ui
        .session
        .session_path
        .clone()
        .unwrap_or_else(|| default_session_path(root));

    let mut session = Session::new();
    if let Some(root) = root {
        session.folders.push(root.to_path_buf());
    }

    for (idx, entry) in app.documents.entries.iter().enumerate() {
        let is_empty_untitled =
            entry.path.is_none() && entry.buffer.len_chars() == 0 && !entry.is_dirty();
        if is_empty_untitled && app.documents.entries.len() > 1 {
            continue;
        }

        let cursors = entry
            .cursors
            .cursors()
            .iter()
            .map(|s| CursorState {
                char_offset: s.head.char,
            })
            .collect();
        let scroll_line =
            (entry.scroll_y / app.editor_view.metrics().line_height.max(1.0)).floor() as usize;
        let title = app
            .documents
            .tabs
            .tabs()
            .iter()
            .find(|t| t.id == entry.tab_id)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "Untitled".to_string());
        let language = entry
            .language
            .map(|l| crate::status_bar::language_name(Some(l)).to_string());
        let is_dirty = entry.is_dirty() || (entry.path.is_none() && entry.buffer.len_chars() > 0);
        let draft_text = if is_dirty || entry.path.is_none() {
            Some(entry.buffer.text())
        } else {
            None
        };

        session.tabs.push(TabState {
            path: entry.path.clone(),
            title,
            pinned: false,
            cursors,
            scroll_line,
            language,
            draft_text,
            is_dirty,
        });
        if app.documents.active_tab == Some(entry.tab_id) {
            session.active_tab = Some(idx);
        }
    }

    if let Some(parent) = session_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if clean {
        session.end_cleanly(&session_path)
    } else {
        session.save(&session_path)
    }
}

pub(crate) fn restore_session(app: &mut App, session_path: &Path) -> std::io::Result<bool> {
    let session = Session::load(session_path)?;
    if session.folders.is_empty() && session.tabs.is_empty() {
        return Ok(false);
    }

    app.ui.session.session_path = Some(session_path.to_path_buf());

    // Restore folders
    if let Some(folder) = session.folders.first()
        && folder.exists()
    {
        app.ui.sidebar = crate::sidebar::SidebarState::open_folder(folder.clone());
        app.ui.session.init_watcher(folder);
    }

    // Restore tabs
    let mut restored_any = false;
    let had_initial_empty = app.documents.entries.len() == 1
        && app.documents.entries[0].path.is_none()
        && !app.documents.entries[0].is_dirty()
        && app.documents.entries[0].buffer.len_chars() == 0;

    for tab_state in &session.tabs {
        let buffer = if let Some(draft) = &tab_state.draft_text {
            Buffer::from_bytes(draft.as_bytes()).ok()
        } else if let Some(path) = &tab_state.path {
            if path.exists() {
                Buffer::open(path).ok()
            } else {
                None
            }
        } else {
            None
        };

        let Some(buffer) = buffer else {
            continue;
        };

        let file_name = if !tab_state.title.is_empty() {
            tab_state.title.clone()
        } else if let Some(path) = &tab_state.path {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string()
        } else {
            "Untitled".to_string()
        };

        let tab_id = app.documents.tabs.open(&file_name);
        let mut entry = DocumentEntry::new(tab_id, tab_state.path.clone(), buffer);

        // Restore language if recorded
        if let Some(lang_name) = &tab_state.language {
            for (id, name) in crate::language_picker::all_languages() {
                if name == lang_name {
                    entry.language = id;
                    break;
                }
            }
        }

        // Restore dirty state
        if tab_state.is_dirty || (tab_state.path.is_none() && entry.buffer.len_chars() > 0) {
            entry.saved_version = u64::MAX;
        }

        // Restore cursors
        if !tab_state.cursors.is_empty() {
            let sels: Vec<_> = tab_state
                .cursors
                .iter()
                .map(|c| {
                    let pos = Position::from_char(&entry.buffer, c.char_offset);
                    Selection::cursor(pos)
                })
                .collect();
            entry.cursors = MultiCursor::from_cursors(sels);
        }
        entry.scroll_y =
            tab_state.scroll_line as f32 * app.editor_view.metrics().line_height.max(1.0);
        app.documents.entries.push(entry);
        restored_any = true;
    }

    if restored_any {
        if had_initial_empty {
            let first_tab = app.documents.entries[0].tab_id;
            app.documents.entries.remove(0);
            app.documents.tabs.close(first_tab);
        }

        let target_idx = session.active_tab.unwrap_or(0);
        if let Some(entry) = app.documents.entries.get(target_idx).cloned() {
            app.switch_to_tab(entry.tab_id);
        }
    }

    app.scheduler.invalidate_all();
    Ok(restored_any)
}

pub(crate) fn poll_file_watcher(app: &mut App) {
    let changes = if let Some(watcher) = &mut app.ui.session.watcher {
        watcher.try_recv().changes
    } else {
        return;
    };

    app.ui.session.symbol_index.apply_changes(&changes);

    let mut repaint_needed = false;
    for change in changes {
        match change.kind {
            ChangeKind::Reload => {
                let is_active_reloaded = if let Some(entry) = app
                    .documents
                    .entries
                    .iter_mut()
                    .find(|e| e.path.as_ref() == Some(&change.path))
                {
                    if !entry.is_dirty() {
                        if let Ok(new_buf) = Buffer::open(&change.path) {
                            entry.buffer = new_buf.clone();
                            entry.saved_version = entry.history.state_id();
                            entry.disk_snapshot = Some(new_buf);
                            app.documents.active_tab == Some(entry.tab_id)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_active_reloaded && let Ok(new_buf) = Buffer::open(&change.path) {
                    app.buffer = new_buf;
                    app.update_syntax_on_open();
                }
                repaint_needed = true;
            }
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
            _ => {}
        }
    }

    if repaint_needed {
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_path_resolution() {
        let p = default_session_path(None);
        assert!(p.to_string_lossy().contains("last-session.json"));

        let dir = tempfile::tempdir().unwrap();
        let ws_p = default_session_path(Some(dir.path()));
        assert_eq!(ws_p, dir.path().join(SESSION_FILE_NAME));
    }

    #[test]
    fn session_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("code.rs");
        std::fs::write(&file_path, "fn main() {}\n").unwrap();

        let mut session = Session::new();
        session.folders.push(dir.path().to_path_buf());
        session.tabs.push(TabState {
            path: Some(file_path.clone()),
            title: "code.rs".to_string(),
            pinned: false,
            cursors: vec![CursorState { char_offset: 3 }],
            scroll_line: 0,
            language: None,
            draft_text: None,
            is_dirty: false,
        });
        session.active_tab = Some(0);

        let session_file = dir.path().join(SESSION_FILE_NAME);
        session.save(&session_file).unwrap();

        let loaded = Session::load(&session_file).unwrap();
        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.tabs.len(), 1);
        assert_eq!(loaded.tabs[0].path, Some(file_path));
        assert_eq!(loaded.tabs[0].cursors[0].char_offset, 3);
        assert_eq!(loaded.active_tab, Some(0));
    }

    #[test]
    fn app_session_save_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("main.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.ui.sidebar = crate::sidebar::SidebarState::open_folder(dir.path().to_path_buf());
        let buf = Buffer::open(&file_path).unwrap();
        let tab_id = app.documents.tabs.open("main.rs");
        let entry = DocumentEntry::new(tab_id, Some(file_path.clone()), buf);
        app.documents.entries.push(entry);
        app.switch_to_tab(tab_id);

        let session_file = dir.path().join(SESSION_FILE_NAME);
        app.ui.session.session_path = Some(session_file.clone());

        save_session(&app, true).unwrap();
        assert!(session_file.exists());

        // Now create a fresh app and restore session
        let mut app2 =
            App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        let restored = restore_session(&mut app2, &session_file).unwrap();
        assert!(restored);
        assert_eq!(app2.documents.entries.len(), 1);
        assert_eq!(app2.documents.entries[0].path, Some(file_path));
        assert!(app2.ui.sidebar.is_some());
    }

    #[test]
    fn app_session_hot_exit_restores_unsaved_untitled_and_draft_files() {
        let dir = tempfile::tempdir().unwrap();
        let session_file = dir.path().join(SESSION_FILE_NAME);

        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.ui.session.session_path = Some(session_file.clone());

        // Edit the default untitled tab with some unsaved notes
        app.buffer = Buffer::from_bytes(b"# My Scratchpad\nDraft notes here").unwrap();
        app.sync_active_document_to_entry();

        save_session(&app, true).unwrap();

        // Restore in a fresh app instance
        let mut app2 =
            App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        let restored = restore_session(&mut app2, &session_file).unwrap();
        assert!(restored);
        assert_eq!(app2.documents.entries.len(), 1);
        assert!(app2.documents.entries[0].path.is_none());
        assert_eq!(
            app2.documents.entries[0].buffer.text(),
            "# My Scratchpad\nDraft notes here"
        );
        assert!(app2.document_dirty());
    }
}
