use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use super::*;

fn app_on(path: &Path, text: &[u8]) -> App {
    let mut app =
        App::new(include_bytes!("../../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let buffer = Buffer::from_bytes(text).unwrap();
    app.documents.entries[0].path = Some(path.to_path_buf());
    app.documents.entries[0].buffer = buffer.clone();
    app.documents.entries[0].disk_snapshot = Some(buffer.clone());
    app.documents.path = Some(path.to_path_buf());
    app.documents.disk_snapshot = Some(buffer.clone());
    app.buffer = buffer;
    app
}

fn change(path: &Path, kind: ChangeKind) -> FileChange {
    FileChange {
        path: path.to_path_buf(),
        kind,
        is_dir: false,
    }
}

/// The read is held on its worker: handling the Reload returns with the
/// old text still in place, and the new text arrives with the wake.
#[test]
fn a_reload_returns_before_reading_and_applies_when_the_read_arrives() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "disk\n").unwrap();
    let mut app = app_on(&file, b"disk\n");
    let (wake_tx, woke) = mpsc::channel();
    app.ui.session.register_waker(Arc::new(move || {
        let _ = wake_tx.send(());
    }));
    let (release, hold) = mpsc::channel();
    app.ui.session.reloads.hold = Some(hold);

    std::fs::write(&file, "fresh\n").unwrap();
    apply_changes(&mut app, vec![change(&file, ChangeKind::Reload)]);
    assert_eq!(app.buffer.text(), "disk\n");
    assert!(woke.try_recv().is_err(), "nothing was read yet");

    release.send(()).unwrap();
    woke.recv_timeout(Duration::from_secs(5))
        .expect("the read wakes the loop");
    poll_file_watcher(&mut app);
    assert_eq!(app.buffer.text(), "fresh\n");
    assert_eq!(app.documents.entries[0].buffer.text(), "fresh\n");
    assert_eq!(
        app.documents.disk_snapshot.as_ref().map(Buffer::text),
        Some("fresh\n".to_string())
    );
    assert!(!app.document_dirty());
}

/// A read that lands after the tab was edited leaves the edit alone.
#[test]
fn a_reload_that_arrives_after_an_edit_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "fresh\n").unwrap();
    let mut app = app_on(&file, b"disk\n");
    let (wake_tx, woke) = mpsc::channel();
    app.ui.session.register_waker(Arc::new(move || {
        let _ = wake_tx.send(());
    }));
    apply_changes(&mut app, vec![change(&file, ChangeKind::Reload)]);
    app.documents.saved_version = app.history.state_id().wrapping_add(1);
    woke.recv_timeout(Duration::from_secs(5))
        .expect("the read wakes the loop");
    poll_file_watcher(&mut app);
    assert_eq!(app.buffer.text(), "disk\n");
}

/// A file deleted elsewhere keeps its tab, now modified, with no disk
/// baseline, so the next save writes it back instead of failing.
#[test]
fn a_removed_open_file_is_marked_modified() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    let mut app = app_on(&file, b"kept\n");
    assert!(!app.document_dirty());
    assert!(apply_changes(
        &mut app,
        vec![change(&file, ChangeKind::Removed)]
    ));
    assert!(app.document_dirty());
    assert!(app.documents.disk_snapshot.is_none());
    assert!(app.documents.entries[0].is_dirty());
    assert_eq!(app.documents.path.as_deref(), Some(file.as_path()));
    assert_eq!(app.buffer.text(), "kept\n");
}

/// The source of a sidebar rename in flight is reported removed by the
/// watcher. The tab waits for the rename instead of turning modified.
#[test]
fn the_source_of_a_rename_in_flight_is_not_marked_removed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    let mut app = app_on(&file, b"kept\n");
    app.ui.session.file_ops.renames.begin(file.clone());
    apply_changes(&mut app, vec![change(&file, ChangeKind::Removed)]);
    assert!(!app.document_dirty());
    assert!(app.documents.disk_snapshot.is_some());
}

/// Reloads `file` (already rewritten on disk) into `app` and waits for it.
pub(crate) fn reload_and_wait(app: &mut App, file: &Path) {
    let (wake_tx, woke) = mpsc::channel();
    app.ui.session.register_waker(Arc::new(move || {
        let _ = wake_tx.send(());
    }));
    apply_changes(app, vec![change(file, ChangeKind::Reload)]);
    woke.recv_timeout(Duration::from_secs(5))
        .expect("the read wakes the loop");
    poll_file_watcher(app);
}

/// A clean tab with undo history, reloaded from disk: undo first brings
/// back the text from before the reload, then the edits before it, each
/// against the text it was recorded on.
#[test]
fn undo_after_a_reload_restores_the_previous_text_not_garbage() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "one\n").unwrap();
    let mut app = app_on(&file, b"one\n");
    let caret = |buffer: &Buffer, at| Selection::cursor(Position::from_char(buffer, at));
    let before = caret(&app.buffer, 3);
    let edit = Edit::insert(3, " more");
    edit.apply(&mut app.buffer);
    let after = caret(&app.buffer, 8);
    app.history
        .record(Transaction::new(vec![edit], vec![before], vec![after]));
    std::fs::write(&file, "one more\n").unwrap();
    app.documents.saved_version = app.history.state_id();
    assert!(!app.document_dirty());

    std::fs::write(&file, "a different, longer file\nwith two lines\n").unwrap();
    reload_and_wait(&mut app, &file);
    assert_eq!(
        app.buffer.text(),
        "a different, longer file\nwith two lines\n"
    );
    assert!(!app.document_dirty());

    crate::input::dispatch::execute_keymap_command(&mut app, "undo", None, None);
    assert_eq!(app.buffer.text(), "one more\n");
    assert!(app.document_dirty(), "the pre-reload text is not on disk");
    crate::input::dispatch::execute_keymap_command(&mut app, "undo", None, None);
    assert_eq!(app.buffer.text(), "one\n");
    crate::input::dispatch::execute_keymap_command(&mut app, "redo", None, None);
    crate::input::dispatch::execute_keymap_command(&mut app, "redo", None, None);
    assert_eq!(
        app.buffer.text(),
        "a different, longer file\nwith two lines\n"
    );
    assert!(!app.document_dirty());
}

/// The same for a tab in the background: its own history gets the step.
#[test]
fn a_background_tab_reload_is_one_undo_step_too() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "hello world\n").unwrap();
    let mut app = app_on(&file, b"hello world\n");
    let other = app.documents.tabs.open("other");
    app.documents
        .entries
        .push(crate::document::DocumentEntry::new(
            other,
            None,
            Buffer::from_bytes(b"").unwrap(),
        ));
    app.switch_to_tab(other);

    std::fs::write(&file, "hello brave world\n").unwrap();
    reload_and_wait(&mut app, &file);
    let entry = &mut app.documents.entries[0];
    assert_eq!(entry.buffer.text(), "hello brave world\n");
    assert!(!entry.is_dirty());
    assert!(entry.history.undo(&mut entry.buffer).is_some());
    assert_eq!(entry.buffer.text(), "hello world\n");
}
