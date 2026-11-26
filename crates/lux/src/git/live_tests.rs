//! The live git paths, driven the way the running editor drives them:
//! keys through `handle_keyboard_event`, disk changes through the real
//! file watcher, and results through the worker's mailbox.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lux_core::buffer::Buffer;
use lux_git::gutter::HunkKind;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, FileDialogs, Key, PlatformContext};

use super::live::GUTTER_DEBOUNCE;
use super::marks::GitMark;
use crate::app::App;

struct Dialogs;

impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
        lux_platform::dialog::SaveDecision::Discard
    }
    fn confirm_trash(&self, _: &str) -> bool {
        false
    }
}

struct Ctx;

impl lux_platform::Clipboard for Ctx {
    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
        Ok(())
    }
}

impl PlatformContext for Ctx {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(Dialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &Dialogs
    }
    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        self
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
        Err(lux_platform::PlatformError::Window(
            "headless git test".into(),
        ))
    }
    fn request_wakeup(&self, _: Instant) {}
    fn exit(&self) {}
}

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

/// A repository whose `note.txt` matches `HEAD`, at its canonical path
/// (the watcher reports canonical paths on macOS).
fn clean_repo() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().canonicalize().unwrap();
    let repo = lux_git::gix::init(&root).expect("init");
    std::fs::write(root.join("note.txt"), "alpha\n").unwrap();
    lux_git::staging::stage(&repo, Path::new("note.txt")).unwrap();
    lux_git::commit::commit(&repo, "add note", "Lux Test", "lux-test@example.com").unwrap();
    (dir, root)
}

fn type_str(app: &mut App, text: &str) {
    for ch in text.chars() {
        let token = ch.to_string();
        app.handle_keyboard_event(
            &Ctx,
            &KeyEvent {
                key: Key::Character(token.clone()),
                text: Some(token),
                state: ElementState::Pressed,
                repeat: false,
                physical: None,
            },
        );
    }
}

/// Runs the same callbacks the event loop runs when a worker wakes it,
/// until `done` holds or ten seconds pass.
fn pump_until(app: &mut App, mut done: impl FnMut(&App) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        crate::session::poll_file_watcher(app);
        super::poll(app);
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn typing_in_a_tracked_file_updates_the_gutter_after_the_debounce_without_saving() {
    let _mailbox = super::MAILBOX_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (_dir, root) = clean_repo();
    let file = root.join("note.txt");
    let mut app = app();
    app.documents.path = Some(file.clone());
    app.buffer = Buffer::from_bytes(b"alpha\n").unwrap();
    app.ui.git.repo_root = Some(root.clone());
    // The file matches HEAD: nothing to mark yet.
    assert!(app.ui.git.markers.is_empty());

    // A result may land while later keys are still queued (a slow first
    // key in a debug build is enough), so time from the first key.
    let first_key = Instant::now();
    type_str(&mut app, "beta\n");
    assert_eq!(app.buffer.text(), "beta\nalpha\n");

    let marked = pump_until(&mut app, |app| {
        app.ui.git.markers.iter().any(|m| m.kind == HunkKind::Added)
    });
    assert!(marked, "markers {:?}", app.ui.git.markers);
    assert!(
        first_key.elapsed() >= GUTTER_DEBOUNCE,
        "the diff ran before edits paused for {GUTTER_DEBOUNCE:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "alpha\n",
        "the gutter must follow the buffer, not a save"
    );
}

#[test]
fn a_new_untracked_file_appears_in_the_tree_after_a_watcher_event() {
    let _mailbox = super::MAILBOX_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (_dir, root) = clean_repo();
    let mut app = app();
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root.clone());
    app.ui.git.repo_root = Some(root.clone());
    app.ui.session.init_watcher(&root);

    let fresh = root.join("fresh.txt");
    std::fs::write(&fresh, "new\n").unwrap();

    let marked = pump_until(&mut app, |app| {
        app.ui
            .sidebar
            .as_ref()
            .is_some_and(|sidebar| sidebar.git_marks.mark_for(&fresh) == Some(GitMark::Untracked))
    });
    assert!(
        marked,
        "marks {:?}",
        app.ui.sidebar.as_ref().map(|sidebar| &sidebar.git_marks)
    );
    assert_eq!(
        crate::git::git_marks_label(&app.ui.git.changed_files, &app.ui.git.ignored),
        "fresh.txt:U"
    );
}

#[test]
fn opening_a_folder_reads_its_status_once_without_a_command() {
    use lux_platform::PlatformEventHandler;

    let _mailbox = super::MAILBOX_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (_dir, root) = clean_repo();
    std::fs::write(root.join("note.txt"), "alpha\nchanged\n").unwrap();
    std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
    std::fs::create_dir(root.join("build")).unwrap();
    let mut app = app();
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root.clone());

    // The event loop's idle callback notices the newly open folder.
    app.about_to_wait(&Ctx);
    assert!(pump_until(&mut app, |app| !app
        .ui
        .git
        .changed_files
        .is_empty()));
    assert_eq!(
        crate::git::git_marks_label(&app.ui.git.changed_files, &app.ui.git.ignored),
        ".gitignore:U,build:!,note.txt:M"
    );
    let marks = &app.ui.sidebar.as_ref().unwrap().git_marks;
    assert_eq!(
        marks.mark_for(&root.join("build/out.o")),
        Some(GitMark::Ignored)
    );
}

#[test]
fn continuous_typing_shows_marks_even_when_the_diff_is_slower_than_the_keys() {
    use std::sync::atomic::Ordering;

    struct ResetDelay;
    impl Drop for ResetDelay {
        fn drop(&mut self) {
            super::live::GUTTER_TEST_DELAY_MS.store(0, Ordering::Relaxed);
        }
    }

    let _mailbox = super::MAILBOX_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (_dir, root) = clean_repo();
    let mut app = app();
    app.documents.path = Some(root.join("note.txt"));
    app.buffer = Buffer::from_bytes(b"alpha\n").unwrap();
    app.ui.git.repo_root = Some(root.clone());
    // Each diff outlasts several keystrokes, so every result arrives after
    // newer edits were queued.
    super::live::GUTTER_TEST_DELAY_MS.store(600, Ordering::Relaxed);
    let _reset = ResetDelay;

    let start = Instant::now();
    let mut first_marks = None;
    while start.elapsed() < Duration::from_secs(4) {
        type_str(&mut app, "x");
        std::thread::sleep(Duration::from_millis(100));
        super::poll(&mut app);
        if first_marks.is_none() && !app.ui.git.markers.is_empty() {
            first_marks = Some(start.elapsed());
        }
    }
    let first = first_marks.expect("no gutter marks during four seconds of typing");
    assert!(
        first < super::live::GUTTER_MAX_WAIT + Duration::from_millis(600 + 1000),
        "marks took {first:?}"
    );
}

#[test]
fn watcher_paths_outside_the_repository_do_not_refresh_its_status() {
    let mut app = app();
    let root = PathBuf::from("/work/repo");
    app.ui.git.repo_root = Some(root.clone());
    app.ui.git.ignored = vec![PathBuf::from("target")];
    let other = PathBuf::from("/work/other-folder/a.rs");
    let built = root.join("target/debug/lux");
    let source = root.join("src/main.rs");

    assert!(!super::live::touches_repo(
        &app,
        [&other, &built].into_iter()
    ));
    assert!(super::live::touches_repo(
        &app,
        [&other, &source].into_iter()
    ));
    app.ui.git.repo_root = None;
    assert!(super::live::touches_repo(&app, [&other].into_iter()));
}
