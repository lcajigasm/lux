use std::path::PathBuf;
use std::time::Duration;

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_workspace::session::{CursorState, SESSION_FILE_NAME, Session, TabState};

use super::*;
use crate::document::DocumentEntry;

mod hot_exit;
mod switching;
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
        cursors: vec![CursorState::caret(3)],
        scroll_line: 0,
        language: None,
        draft_text: None,
        is_dirty: false,
        history: None,
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

    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(dir.path().to_path_buf());
    let buf = Buffer::open(&file_path).unwrap();
    let tab_id = app.documents.tabs.open("main.rs");
    let entry = DocumentEntry::new(tab_id, Some(file_path.clone()), buf);
    app.documents.entries.push(entry);
    app.switch_to_tab(tab_id);

    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);

    save_session(&mut app, true).unwrap();
    assert!(session_file.exists());

    // Now create a fresh app and restore session
    let mut app2 = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let restored = restore_session(&mut app2, &session_file).unwrap();
    assert!(restored);
    assert_eq!(app2.documents.entries.len(), 1);
    assert_eq!(app2.documents.entries[0].path, Some(file_path));
    assert!(app2.ui.sidebar.is_some());
}

#[test]
fn app_session_hot_exit_restores_unsaved_untitled_and_draft_files() {
    let dir = tempfile::tempdir().unwrap();

    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);

    // Edit the default untitled tab with some unsaved notes
    app.buffer = Buffer::from_bytes(b"# My Scratchpad\nDraft notes here").unwrap();
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&app.buffer, 4)));
    app.sync_active_document_to_entry();

    save_session(&mut app, true).unwrap();

    // Restore in a fresh app instance
    let mut app2 = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let restored = restore_session(&mut app2, &session_file).unwrap();
    assert!(restored);
    assert_eq!(app2.documents.entries.len(), 1);
    assert!(app2.documents.entries[0].path.is_none());
    assert_eq!(
        app2.documents.entries[0].buffer.text(),
        "# My Scratchpad\nDraft notes here"
    );
    assert!(app2.document_dirty());
    assert_eq!(app2.cursors.cursors()[0].head.char, 4);
}

#[test]
fn hot_exit_restores_a_dirty_named_file_beside_a_clean_tab() {
    let dir = tempfile::tempdir().unwrap();
    let named = dir.path().join("saved.txt");
    std::fs::write(&named, "on disk\n").unwrap();
    let other = dir.path().join("other.txt");
    std::fs::write(&other, "clean\n").unwrap();

    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);
    app.documents.entries[0].path = Some(named.clone());
    app.documents.entries[0].buffer = Buffer::from_bytes(b"draft text\n").unwrap();
    app.documents.entries[0].saved_version = 1;
    app.buffer = app.documents.entries[0].buffer.clone();
    app.documents.path = Some(named.clone());
    app.documents.saved_version = 1;
    let tab = app.documents.tabs.open("other.txt");
    app.documents.entries.push(DocumentEntry::new(
        tab,
        Some(other.clone()),
        Buffer::from_bytes(b"clean\n").unwrap(),
    ));

    save_session(&mut app, true).unwrap();

    let mut app2 = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    assert!(restore_session(&mut app2, &session_file).unwrap());
    assert_eq!(app2.documents.entries.len(), 2);
    let dirty = app2
        .documents
        .entries
        .iter()
        .find(|entry| entry.path.as_ref() == Some(&named))
        .unwrap();
    assert_eq!(dirty.buffer.text(), "draft text\n");
    assert!(dirty.is_dirty());
    assert_eq!(app2.buffer.text(), "draft text\n");
    let clean = app2
        .documents
        .entries
        .iter()
        .find(|entry| entry.path.as_ref() == Some(&other))
        .unwrap();
    assert_eq!(clean.buffer.text(), "clean\n");
    assert!(!clean.is_dirty());
}

#[test]
fn save_session_records_the_window_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);
    app.ui.session.frame = Some(lux_workspace::WindowFrame {
        x: 12,
        y: 34,
        width: 640,
        height: 480,
        monitor_name: "Built-in".to_string(),
        monitor_x: 0,
        monitor_y: 0,
        scale_millis: 2000,
        fullscreen: false,
        maximized: true,
    });
    save_session(&mut app, true).unwrap();
    let loaded = Session::load(&session_file).unwrap();
    assert_eq!(loaded.windows.len(), 1);
    assert_eq!(loaded.windows[0].x, 12);
    assert_eq!(loaded.windows[0].width, 640);
    assert!(loaded.windows[0].maximized);
    assert!(!loaded.windows[0].fullscreen);
}

#[test]
fn an_external_edit_reloads_a_clean_file_and_asks_when_dirty() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("note.txt");
    std::fs::write(&raw, "disk\n").unwrap();
    let file = std::fs::canonicalize(&raw).unwrap();

    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let buf = Buffer::from_bytes(b"disk\n").unwrap();
    app.documents.entries[0].path = Some(file.clone());
    app.documents.entries[0].buffer = buf.clone();
    app.buffer = buf;
    crate::sidebar::watch::watch_and_wait(&mut app, dir.path());

    std::fs::write(&file, "fresh\n").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        poll_file_watcher(&mut app);
        if app.buffer.text() == "fresh\n" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "clean buffer was not reloaded"
        );
        std::thread::sleep(Duration::from_millis(40));
    }
    assert!(app.ui.dialogs.conflict_prompt.is_none());

    app.buffer = Buffer::from_bytes(b"local\n").unwrap();
    app.documents.entries[0].buffer = app.buffer.clone();
    app.documents.entries[0].saved_version =
        app.documents.entries[0].history.state_id().wrapping_add(1);
    app.ui.session.mark_dirty(&file);
    std::fs::write(&file, "other\n").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    loop {
        poll_file_watcher(&mut app);
        if app.ui.dialogs.conflict_prompt.is_some() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "dirty buffer did not ask before reload"
        );
        std::thread::sleep(Duration::from_millis(40));
    }
    assert_eq!(app.buffer.text(), "local\n");
}
