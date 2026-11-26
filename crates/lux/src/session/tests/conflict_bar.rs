//! The conflict bar's Reload, Overwrite and Compare Diff land through the
//! `ui_io` worker. A reload is one undo step on the live tab, and text
//! typed while an action runs is never lost or marked saved.

use crate::conflict::{ConflictAction, ConflictPrompt, handle_conflict_action};

use super::hot_exit::{Ctx, new_app, open_file, type_text, undo_key};
use super::*;

fn project(dir: &std::path::Path, text: &str) -> PathBuf {
    let path = dir.join("note.txt");
    std::fs::write(&path, text).unwrap();
    path
}

fn act(app: &mut App, path: &std::path::Path, action: ConflictAction) {
    app.ui.dialogs.conflict_prompt = Some(ConflictPrompt::new(path.to_path_buf()));
    handle_conflict_action(app, &Ctx, action);
}

/// Reload on the active tab replaces the live text, marks it saved, and
/// Cmd+Z gives back the edited text from before the reload.
#[test]
fn reload_on_the_active_tab_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "start\n");
    let mut app = new_app();
    open_file(&mut app, &file);
    type_text(&mut app, "mine ");
    std::fs::write(&file, "theirs\n").unwrap();

    act(&mut app, &file, ConflictAction::ReloadDisk);
    crate::ui_io::finish_all(&mut app, &Ctx);

    assert_eq!(app.buffer.text(), "theirs\n");
    assert!(!app.document_dirty(), "the reloaded tab reads as saved");
    undo_key(&mut app);
    assert_eq!(app.buffer.text(), "mine start\n");
    assert!(app.document_dirty());
}

/// Keys typed after clicking Reload, before the read lands, are kept and
/// the bar asks again instead of replacing them.
#[test]
fn typing_during_a_reload_keeps_the_keys_and_asks_again() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "start\n");
    let mut app = new_app();
    open_file(&mut app, &file);
    type_text(&mut app, "a");
    std::fs::write(&file, "theirs\n").unwrap();

    let hold = app.ui.dialogs.jobs.hold();
    act(&mut app, &file, ConflictAction::ReloadDisk);
    type_text(&mut app, "b");
    hold.release();
    crate::ui_io::finish_all(&mut app, &Ctx);

    assert_eq!(app.buffer.text(), "abstart\n");
    assert!(app.document_dirty());
    assert!(app.ui.dialogs.conflict_prompt.is_some(), "asks again");
}

/// Overwrite Disk while the user keeps typing: the disk gets the text from
/// the click, and the tab stays modified because it no longer matches.
#[test]
fn typing_during_an_overwrite_leaves_the_tab_modified() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "start\n");
    let mut app = new_app();
    open_file(&mut app, &file);
    type_text(&mut app, "a");

    let at_click = app.history.state_id();
    let hold = app.ui.dialogs.jobs.hold();
    act(&mut app, &file, ConflictAction::OverwriteDisk);
    type_text(&mut app, "b");
    hold.release();
    crate::ui_io::finish_all(&mut app, &Ctx);

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "astart\n");
    assert!(app.document_dirty());
    assert_eq!(app.documents.saved_version, at_click);
}

/// A failed write says so and leaves the tab modified.
#[cfg(unix)]
#[test]
fn a_failed_overwrite_is_reported_and_stays_modified() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "start\n");
    let mut app = new_app();
    open_file(&mut app, &file);
    type_text(&mut app, "a");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
    let writable = std::fs::OpenOptions::new().write(true).open(&file).is_ok();

    act(&mut app, &file, ConflictAction::OverwriteDisk);
    crate::ui_io::finish_all(&mut app, &Ctx);
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    if writable {
        return; // Running as a user that ignores file permissions.
    }

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "start\n");
    assert!(app.document_dirty());
    let message = app.documents.message.clone().unwrap_or_default();
    assert!(message.starts_with("Cannot overwrite"), "{message}");
}

/// Compare Diff on a background tab diffs that tab's text, not the
/// active tab's.
#[test]
fn compare_diff_uses_the_conflicting_tab() {
    let dir = tempfile::tempdir().unwrap();
    let file = project(dir.path(), "disk\n");
    let other = dir.path().join("other.txt");
    std::fs::write(&other, "unrelated\n").unwrap();
    let mut app = new_app();
    open_file(&mut app, &file);
    type_text(&mut app, "edited ");
    open_file(&mut app, &other);

    act(&mut app, &file, ConflictAction::CompareDiff);
    crate::ui_io::finish_all(&mut app, &Ctx);

    let diff = app.buffer.text();
    assert!(diff.contains("-disk"), "{diff}");
    assert!(diff.contains("+edited disk"), "{diff}");
    assert!(!diff.contains("unrelated"), "{diff}");
}
