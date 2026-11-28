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

/// An edit made in project A inside the quiet period, right before the
/// user opens project B, lands in A's session. Without the flush on the
/// root change it was never written (the debounced write snapshots B), and
/// a write still queued for A was replaced by B's.
#[test]
fn an_edit_just_before_switching_projects_is_written_to_the_old_project() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let root_a = dir.path().join("a");
    let root_b = dir.path().join("b");
    let file_a = project(&root_a, "a.txt", "alpha\n");
    project(&root_b, "b.txt", "beta\n");

    let mut app = new_app();
    app.ui.session.data_dir = Some(data);
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root_a.clone());
    app.ui.session.register_waker(Arc::new(|| {}));
    open_file(&mut app, &file_a);
    let session_a = crate::session::snapshot::current_file(&app);
    type_text(&mut app, "late ");

    app.open_workspace(root_b.clone());
    assert_ne!(crate::session::snapshot::current_file(&app), session_a);
    crate::session::snapshot::wait_written();

    let written = Session::load(&session_a).expect("A's session was never written");
    assert_eq!(written.folders, vec![root_a]);
    let tab = written
        .tabs
        .iter()
        .find(|tab| tab.path.as_deref() == Some(file_a.as_path()))
        .expect("A's tab");
    assert!(tab.is_dirty);
    assert_eq!(tab.draft_text.as_deref(), Some("late alpha\n"));
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

/// `lux demo.sublime-project`: the project is read on a worker, so the
/// window appears before it lands. The folderless session must not be
/// restored into that window, and a project the user has since replaced
/// must not land.
#[test]
fn a_project_from_the_command_line_skips_the_folderless_restore() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    let loose = project(&dir.path().join("loose"), "loose.txt", "loose\n");
    let mut first = new_app();
    first.ui.session.data_dir = Some(data.clone());
    open_file(&mut first, &loose);
    save_session(&mut first, true).unwrap();

    let root = dir.path().join("p");
    project(&root, "p.txt", "p\n");
    let project_file = project(
        &root,
        "demo.sublime-project",
        r#"{"folders":[{"path":"."}]}"#,
    );
    let mut app = new_app();
    app.ui.session.data_dir = Some(data.clone());
    app.open_project_file(&project_file, None);
    crate::session::restore_at_launch(&mut app, &Ctx);
    crate::ui_io::finish_all(&mut app, &Ctx);

    assert_eq!(app.ui.project.active_project_path, Some(project_file));
    let open: Vec<_> = app
        .documents
        .entries
        .iter()
        .filter_map(|e| e.path.clone())
        .collect();
    assert!(!open.contains(&loose), "{open:?}");

    // Without a pending project the folderless session still restores.
    let mut plain = new_app();
    plain.ui.session.data_dir = Some(data);
    crate::session::restore_at_launch(&mut plain, &Ctx);
    crate::ui_io::finish_all(&mut plain, &Ctx);
    assert!(
        plain
            .documents
            .entries
            .iter()
            .any(|e| e.path.as_ref() == Some(&loose))
    );
}

/// Open Project, then Open Folder before the project file was read: the
/// folder stays, the project result is dropped.
#[test]
fn a_project_that_lands_after_open_folder_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("p");
    let folder = dir.path().join("f");
    project(&folder, "f.txt", "f\n");
    let project_file = project(
        &root,
        "demo.sublime-project",
        r#"{"folders":[{"path":"."}]}"#,
    );
    let mut app = new_app();
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let hold = app.ui.dialogs.jobs.hold();
    app.open_project_file(&project_file, None);
    app.quick_switch_project();
    app.open_workspace(folder.clone());
    hold.release();
    crate::ui_io::finish_all(&mut app, &Ctx);

    assert_eq!(app.ui.project.active_project_path, None);
    assert!(app.ui.project.picker.is_none());
    assert_eq!(app.workspace_root(), Some(folder));
    assert!(!app.ui.project.opening);
}

/// Closing a project opens no new watcher, yet the folders that build
/// systems derive from changed: `root_changing` bumps the epoch.
#[test]
fn closing_a_project_bumps_the_folder_epoch() {
    let mut app = new_app();
    let before = app.ui.session.folder_epoch();
    app.close_project();
    assert!(app.ui.session.folder_epoch() > before);
}
