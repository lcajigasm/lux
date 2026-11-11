//! One test per git surface the editor exposes. Each one uses a real
//! temporary repository and the same functions the worker runs.

use std::path::Path;

use lux_core::buffer::Buffer;
use lux_git::gutter::HunkKind;

use super::refresh_git_state;
use super::snapshot::{self, file_shot};
use crate::app::App;

fn note_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = lux_git::gix::init(dir.path()).expect("init");
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "alpha\n").unwrap();
    lux_git::staging::stage(&repo, Path::new("note.txt")).unwrap();
    lux_git::commit::commit(&repo, "add note", "Lux Test", "lux-test@example.com").unwrap();
    std::fs::write(&file, "alpha\nbeta\n").unwrap();
    (dir, file)
}

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

#[test]
fn gutter_marks_the_added_line() {
    let (dir, file) = note_repo();
    let shot = file_shot(Some(&file), "alpha\nbeta\n");
    assert_eq!(shot.repo_root.as_deref(), Some(dir.path()));
    assert!(
        shot.markers
            .iter()
            .any(|marker| marker.kind == HunkKind::Added),
        "{:?}",
        shot.markers
    );
}

#[test]
fn status_lists_the_changed_file() {
    let (dir, _) = note_repo();
    let status = snapshot::status_of(dir.path()).expect("status");
    assert!(
        status
            .files
            .iter()
            .any(|file| file.path.ends_with("note.txt")),
        "{:?}",
        status.files
    );
}

#[test]
fn branch_name_is_the_head() {
    let (_dir, file) = note_repo();
    let shot = file_shot(Some(&file), "alpha\nbeta\n");
    let name = shot.branch_name.expect("branch");
    assert!(name == "main" || name == "master", "{name}");
}

#[test]
fn diff_shows_the_added_line() {
    let (dir, _) = note_repo();
    let text = snapshot::diff_text(dir.path(), Path::new("note.txt")).expect("diff");
    assert!(text.lines().any(|line| line == "+beta"), "{text}");
}

#[test]
fn stage_hunk_records_that_hunk() {
    let (dir, file) = note_repo();
    let shot = file_shot(Some(&file), "alpha\nbeta\n");
    let hunk = shot.hunks[0];
    snapshot::stage_hunk_on_disk(dir.path(), &file, &hunk).expect("stage");
    let repo = lux_git::gix::open(dir.path()).unwrap();
    let index = repo.open_index().unwrap();
    let entry = index.entry_by_path("note.txt".into()).unwrap();
    let blob = repo.find_blob(entry.id).unwrap();
    assert_eq!(blob.data, b"alpha\nbeta\n");
}

#[test]
fn commit_writes_the_message() {
    let (dir, _file) = note_repo();
    let repo = lux_git::gix::open(dir.path()).unwrap();
    lux_git::staging::stage(&repo, Path::new("note.txt")).unwrap();
    drop(repo);
    snapshot::commit_index(dir.path(), "second").expect("commit");
    let repo = lux_git::gix::open(dir.path()).unwrap();
    let id = repo.head_id().unwrap();
    let commit = repo.find_commit(id.detach()).unwrap();
    let message = commit.message().unwrap();
    let title = std::str::from_utf8(message.title).unwrap_or("");
    assert!(title.starts_with("second"), "{title}");
}

#[test]
fn blame_attributes_the_line() {
    let (dir, _) = note_repo();
    let text = snapshot::blame_text(dir.path(), Path::new("note.txt")).expect("blame");
    assert!(text.contains("1+"), "{text}");
    assert!(text.chars().next().unwrap().is_ascii_hexdigit());
}

#[test]
fn history_lists_the_commit() {
    let (dir, _) = note_repo();
    let text = snapshot::history_text(dir.path(), Path::new("note.txt")).expect("history");
    assert!(text.contains("add note"), "{text}");
}

#[test]
fn create_branch_keeps_a_dirty_file_and_moves_head() {
    let (dir, file) = note_repo();
    std::fs::write(&file, "alpha\nbeta\n").unwrap();
    let name = snapshot::create_branch(dir.path(), "topic").expect("create");
    assert_eq!(name, "topic");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "alpha\nbeta\n");
    let repo = lux_git::gix::open(dir.path()).unwrap();
    let head = repo.head_name().unwrap().unwrap();
    assert_eq!(head.shorten().to_string(), "topic");
}

#[test]
fn create_branch_keeps_a_dot_and_rejects_a_space() {
    let (dir, _) = note_repo();
    let name = snapshot::create_branch(dir.path(), "release-1.2").expect("dot");
    assert_eq!(name, "release-1.2");
    let error = snapshot::create_branch(dir.path(), "bad name").unwrap_err();
    assert!(!error.is_empty(), "{error}");
}

#[test]
fn switch_branch_refuses_a_dirty_worktree() {
    let (dir, file) = note_repo();
    let repo = lux_git::gix::open(dir.path()).unwrap();
    let initial = repo.head_name().unwrap().unwrap().shorten().to_string();
    drop(repo);
    snapshot::create_branch(dir.path(), "topic").unwrap();
    std::fs::write(&file, "alpha\nbeta\n").unwrap();
    let error = snapshot::switch_branch(dir.path(), &initial).unwrap_err();
    assert!(error.contains("Commit or discard"), "{error}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "alpha\nbeta\n");
}

#[test]
fn switch_branch_checks_out_a_clean_tree() {
    let (dir, file) = note_repo();
    std::fs::write(&file, "alpha\n").unwrap();
    let repo = lux_git::gix::open(dir.path()).unwrap();
    let initial = repo.head_name().unwrap().unwrap().shorten().to_string();
    drop(repo);
    snapshot::create_branch(dir.path(), "topic").unwrap();
    std::fs::write(&file, "alpha\nonly-topic\n").unwrap();
    let repo = lux_git::gix::open(dir.path()).unwrap();
    lux_git::staging::stage(&repo, Path::new("note.txt")).unwrap();
    lux_git::commit::commit(&repo, "topic text", "Lux Test", "lux-test@example.com").unwrap();
    drop(repo);
    snapshot::switch_branch(dir.path(), &initial).expect("switch");
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(text, "alpha\n");
    snapshot::switch_branch(dir.path(), "topic").expect("back");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "alpha\nonly-topic\n"
    );
}

#[test]
fn the_worker_applies_gutter_markers() {
    let (_dir, file) = note_repo();
    let mut app = app();
    app.documents.path = Some(file);
    app.buffer = Buffer::from_bytes(b"alpha\nbeta\n").unwrap();
    refresh_git_state(&mut app);
    let mut saw = false;
    for _ in 0..100 {
        super::poll(&mut app);
        if app.ui.git.branch_name.is_some() && !app.ui.git.markers.is_empty() {
            saw = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(saw, "branch {:?}", app.ui.git.branch_name);
}
