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

use crate::app::{App, LINE_HEIGHT};
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
        if let Some(path) = &entry.path {
            let cursors = entry
                .cursors
                .cursors()
                .iter()
                .map(|s| CursorState {
                    char_offset: s.head.char,
                })
                .collect();
            let scroll_line = (entry.scroll_y / LINE_HEIGHT).floor() as usize;
            session.tabs.push(TabState {
                path: path.clone(),
                pinned: false,
                cursors,
                scroll_line,
            });
            if app.documents.active_tab == Some(entry.tab_id) {
                session.active_tab = Some(idx);
            }
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
        && !app.documents.entries[0].is_dirty();

    for tab_state in &session.tabs {
        if !tab_state.path.exists() {
            continue;
        }
        if let Ok(buffer) = Buffer::open(&tab_state.path) {
            let file_name = tab_state
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            let tab_id = app.documents.tabs.open(file_name);
            let mut entry = DocumentEntry::new(tab_id, Some(tab_state.path.clone()), buffer);

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
            entry.scroll_y = tab_state.scroll_line as f32 * LINE_HEIGHT;
            app.documents.entries.push(entry);
            restored_any = true;
        }
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
                    app.ui.conflict_prompt = Some(ConflictPrompt::new(change.path));
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
            path: file_path.clone(),
            pinned: false,
            cursors: vec![CursorState { char_offset: 3 }],
            scroll_line: 0,
        });
        session.active_tab = Some(0);

        let session_file = dir.path().join(SESSION_FILE_NAME);
        session.save(&session_file).unwrap();

        let loaded = Session::load(&session_file).unwrap();
        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.tabs.len(), 1);
        assert_eq!(loaded.tabs[0].path, file_path);
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
}
