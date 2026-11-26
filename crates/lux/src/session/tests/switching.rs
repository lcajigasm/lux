//! A session belongs to the project open when it is written, and an undo
//! history written after a reload from disk still matches its text.

use std::sync::Arc;
use std::sync::mpsc;

use lux_platform::PlatformEventHandler;
use lux_workspace::watcher::{ChangeKind, FileChange};

use super::hot_exit::{Ctx, new_app, open_file, type_text, undo_key};
use super::*;

fn project(root: &Path, file: &str, text: &str) -> PathBuf {
    std::fs::create_dir_all(root).unwrap();
    let path = root.join(file);
    std::fs::write(&path, text).unwrap();
    path
}

fn tab_paths(session: &Session) -> Vec<PathBuf> {
    session.tabs.iter().filter_map(|t| t.path.clone()).collect()
}

/// Launch restores project A, then the user opens folder B, which has no
/// session yet. Writes from then on belong to B: A's session keeps A's
/// tabs, and the next launch on A does not show B's.
#[test]
fn opening_another_folder_writes_its_own_session_not_the_restored_one() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let root_a = dir.path().join("a");
    let root_b = dir.path().join("b");
    let file_a = project(&root_a, "a.txt", "alpha\n");
    let file_b = project(&root_b, "b.txt", "beta\n");

    let mut first = new_app();
    first.ui.session.data_dir = Some(data.clone());
    first.ui.sidebar = crate::sidebar::SidebarState::open_folder(root_a.clone());
    open_file(&mut first, &file_a);
    save_session(&mut first, true).unwrap();
    let session_a = crate::session::snapshot::current_file(&first);

    let mut app = new_app();
    app.ui.session.data_dir = Some(data.clone());
    let read_from = readable_file(&app, Some(&root_a)).expect("A has a session");
    assert_eq!(read_from, session_a);
    assert!(restore_session(&mut app, &read_from).unwrap());

    app.open_workspace(root_b.clone());
    open_file(&mut app, &file_b);
    let session_b = crate::session::snapshot::current_file(&app);
    assert_ne!(session_b, session_a);
    save_session(&mut app, false).unwrap();

    let written_b = Session::load(&session_b).expect("B's session was never created");
    assert_eq!(written_b.folders, vec![root_b.clone()]);
    assert!(tab_paths(&written_b).contains(&file_b));
    let still_a = Session::load(&session_a).unwrap();
    assert_eq!(still_a.folders, vec![root_a.clone()]);
    assert_eq!(tab_paths(&still_a), vec![file_a.clone()]);

    let mut relaunch = new_app();
    relaunch.ui.session.data_dir = Some(data);
    let read_from = readable_file(&relaunch, Some(&root_a)).unwrap();
    assert!(restore_session(&mut relaunch, &read_from).unwrap());
    let open: Vec<_> = relaunch
        .documents
        .entries
        .iter()
        .filter_map(|e| e.path.clone())
        .collect();
    assert_eq!(open, vec![file_a]);
}

/// Closing the last folder moves writes to the folderless session.
#[test]
fn closing_the_folder_moves_writes_to_the_folderless_session() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("p");
    project(&root, "p.txt", "p\n");
    let mut app = new_app();
    app.ui.session.data_dir = Some(dir.path().join("data"));
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root.clone());
    let with_folder = crate::session::snapshot::current_file(&app);
    app.ui.sidebar = None;
    let without = crate::session::snapshot::current_file(&app);
    assert_ne!(with_folder, without);
    assert_eq!(
        without,
        lux_workspace::session_store::session_file(&dir.path().join("data"), None)
    );
}

/// Edit, save, have the file rewritten elsewhere, quit, relaunch: undo
/// gives back the text from before the reload, never edits replayed onto
/// the new text at stale offsets.
#[test]
fn undo_after_a_reload_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "note.txt", "start\n");
    let mut app = new_app();
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);
    open_file(&mut app, &file);
    type_text(&mut app, "typed ");
    std::fs::write(&file, app.buffer.text()).unwrap();
    app.documents.saved_version = app.history.state_id();
    let before_reload = app.buffer.text();
    assert_eq!(before_reload, "typed start\n");

    std::fs::write(&file, "rewritten by another program\n").unwrap();
    let (wake_tx, woke) = mpsc::channel();
    app.ui.session.register_waker(Arc::new(move || {
        let _ = wake_tx.send(());
    }));
    let change = FileChange {
        path: file.clone(),
        kind: ChangeKind::Reload,
        is_dir: false,
    };
    disk_events::apply_changes(&mut app, vec![change]);
    woke.recv_timeout(Duration::from_secs(5)).unwrap();
    poll_file_watcher(&mut app);
    assert_eq!(app.buffer.text(), "rewritten by another program\n");
    assert!(!app.document_dirty());

    PlatformEventHandler::exiting(&mut app, &Ctx);
    let mut fresh = new_app();
    assert!(restore_session(&mut fresh, &session_file).unwrap());
    assert_eq!(fresh.buffer.text(), "rewritten by another program\n");
    assert!(fresh.history.can_undo(), "the reload step was persisted");
    undo_key(&mut fresh);
    assert_eq!(fresh.buffer.text(), before_reload);
    while fresh.history.can_undo() {
        undo_key(&mut fresh);
    }
    assert_eq!(fresh.buffer.text(), "start\n");
}
