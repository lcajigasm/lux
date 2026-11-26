//! Hot exit end to end (ST-TAB-03): type through the key path, quit
//! through the platform quit path, restore into a fresh `App`.

use std::sync::Arc;
use std::time::Instant;

use lux_core::history::Transaction;
use lux_platform::event::{ElementState, KeyEvent, Modifiers};
use lux_platform::{FileDialogOptions, FileDialogs, Key, NamedKey, PlatformContext};
use lux_platform::{PlatformEventHandler, dialog::SaveDecision};

use super::*;
use crate::split::LayoutPreset;

struct NoDialogs;
impl FileDialogs for NoDialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        panic!("hot exit must not ask to save")
    }
    fn confirm_trash(&self, _: &str) -> bool {
        false
    }
}

static NO_DIALOGS: NoDialogs = NoDialogs;

pub(super) struct Ctx;
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
        Arc::new(NoDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &NO_DIALOGS
    }
    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        self
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
        panic!("unused")
    }
    fn request_wakeup(&self, _: Instant) {}
    fn exit(&self) {}
}

pub(super) fn new_app() -> App {
    let mut app =
        App::new(include_bytes!("../../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.viewport.width = 800.0;
    app.viewport.height = 600.0;
    app
}

fn press(app: &mut App, key: Key, text: Option<&str>) {
    app.handle_keyboard_event(
        &Ctx,
        &KeyEvent {
            key,
            text: text.map(str::to_owned),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );
}

pub(super) fn type_text(app: &mut App, text: &str) {
    app.modifiers = Modifiers::default();
    for ch in text.chars() {
        let s = ch.to_string();
        press(app, Key::Character(s.clone()), Some(&s));
    }
}

pub(super) fn undo_key(app: &mut App) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift: false,
        alt: false,
    };
    press(app, Key::Character("z".into()), Some("z"));
    app.modifiers = Modifiers::default();
}

pub(super) fn open_file(app: &mut App, path: &std::path::Path) {
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let tab = app.documents.tabs.open(&name);
    let buffer = Buffer::open(path).unwrap();
    app.documents
        .entries
        .push(DocumentEntry::new(tab, Some(path.to_path_buf()), buffer));
    app.switch_to_tab(tab);
}

fn sel(buffer: &Buffer, anchor: usize, head: usize) -> Selection {
    Selection::new(
        Position::from_char(buffer, anchor),
        Position::from_char(buffer, head),
    )
}

#[test]
fn quit_and_restore_keeps_text_selections_scroll_layout_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    std::fs::write(&a, "first line\n".repeat(80)).unwrap();
    std::fs::write(&b, "other file\n").unwrap();

    let mut app = new_app();
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);
    let untitled = app.documents.entries[0].tab_id;
    open_file(&mut app, &b);
    open_file(&mut app, &a);
    app.documents.entries.retain(|e| e.tab_id != untitled);
    app.documents.tabs.close(untitled);
    type_text(&mut app, "hello ");
    press(&mut app, Key::Named(NamedKey::Enter), None);
    type_text(&mut app, "world");
    let typed = app.buffer.text();
    assert!(typed.starts_with("hello \nworld"));
    let expected_after_undo = {
        let mut buffer = app.buffer.clone();
        let mut history = app.history.clone();
        history.undo(&mut buffer);
        buffer.text()
    };
    assert_ne!(expected_after_undo, typed);

    // Two carets: a forward selection and a backward one.
    let carets = vec![sel(&app.buffer, 0, 5), sel(&app.buffer, 20, 14)];
    app.cursors = MultiCursor::from_cursors(carets);
    let line_height = app.editor_view.metrics().line_height.max(1.0);
    app.viewport.scroll_y = 7.0 * line_height;
    app.set_layout(LayoutPreset::Columns2);
    app.ui.split.ratio = 0.3;
    let pane_tab = app.ui.split.panes[1].tab_id;
    let pane_path = app
        .documents
        .entries
        .iter()
        .find(|e| e.tab_id == pane_tab)
        .and_then(|e| e.path.clone());
    assert_eq!(pane_path.as_deref(), Some(b.as_path()));
    let saved_cursors: Vec<_> = app
        .cursors
        .cursors()
        .iter()
        .map(|s| (s.anchor.char, s.head.char))
        .collect();

    PlatformEventHandler::exiting(&mut app, &Ctx);
    let written = Session::load(&session_file).unwrap();
    assert!(
        !written.needs_recovery(),
        "quit must end the session cleanly"
    );
    let names: Vec<_> = written
        .tabs
        .iter()
        .filter_map(|t| t.history.clone())
        .collect();
    assert!(
        !names.is_empty(),
        "the edited tab's undo history was not written"
    );

    let mut fresh = new_app();
    fresh.ui.session.data_dir = Some(dir.path().join("data"));
    assert!(restore_session(&mut fresh, &session_file).unwrap());

    assert_eq!(fresh.documents.path.as_deref(), Some(a.as_path()));
    assert_eq!(fresh.buffer.text(), typed);
    assert!(fresh.document_dirty());
    let restored_cursors: Vec<_> = fresh
        .cursors
        .cursors()
        .iter()
        .map(|s| (s.anchor.char, s.head.char))
        .collect();
    assert_eq!(restored_cursors, saved_cursors);
    // Scroll is stored in lines, so it survives a different line height.
    let fresh_line = fresh.editor_view.metrics().line_height.max(1.0);
    assert_eq!(fresh.viewport.scroll_y, 7.0 * fresh_line);
    assert_eq!(fresh.ui.split.preset, LayoutPreset::Columns2);
    assert_eq!(fresh.ui.split.panes.len(), 2);
    assert!((fresh.ui.split.ratio - 0.3).abs() < 0.001);
    let restored_pane = fresh.ui.split.panes[1].tab_id;
    let restored_pane_path = fresh
        .documents
        .entries
        .iter()
        .find(|e| e.tab_id == restored_pane)
        .and_then(|e| e.path.clone());
    assert_eq!(restored_pane_path.as_deref(), Some(b.as_path()));

    undo_key(&mut fresh);
    assert_eq!(fresh.buffer.text(), expected_after_undo);
    while fresh.history.can_undo() {
        undo_key(&mut fresh);
    }
    assert_eq!(fresh.buffer.text(), "first line\n".repeat(80));
}

#[test]
fn a_file_changed_on_disk_after_quit_restores_without_its_stale_history() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("clean.txt");
    std::fs::write(&file, "abc").unwrap();

    let mut app = new_app();
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);
    open_file(&mut app, &file);
    // An edit that was then saved: clean, but with undo history.
    let edit = lux_core::edit::Edit::insert(3, "d");
    edit.apply(&mut app.buffer);
    app.history
        .record(Transaction::new(vec![edit], vec![], vec![]));
    std::fs::write(&file, "abcd").unwrap();
    app.documents.saved_version = app.history.state_id();
    assert!(!app.document_dirty());

    PlatformEventHandler::exiting(&mut app, &Ctx);
    let mut fresh = new_app();
    assert!(restore_session(&mut fresh, &session_file).unwrap());
    assert!(fresh.history.can_undo(), "unchanged file keeps its undo");

    std::fs::write(&file, "changed elsewhere").unwrap();
    let mut fresh = new_app();
    assert!(restore_session(&mut fresh, &session_file).unwrap());
    assert_eq!(fresh.buffer.text(), "changed elsewhere");
    assert!(!fresh.history.can_undo(), "stale history must be discarded");
    assert!(!fresh.document_dirty());
}

#[test]
fn a_legacy_project_session_is_read_and_later_written_to_the_data_dir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let file = root.join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();
    let mut legacy = Session::new();
    legacy.folders.push(root.clone());
    legacy.tabs.push(TabState {
        path: Some(file.clone()),
        title: "main.rs".into(),
        pinned: false,
        cursors: vec![CursorState::caret(3)],
        scroll_line: 0,
        language: None,
        draft_text: None,
        is_dirty: false,
        history: None,
    });
    let legacy_file = root.join(SESSION_FILE_NAME);
    legacy.end_cleanly(&legacy_file).unwrap();

    let mut app = new_app();
    assert!(restore_session(&mut app, &legacy_file).unwrap());
    assert_eq!(app.cursors.primary().head.char, 3);
    let target = crate::session::snapshot::current_file(&app);
    assert_ne!(target, legacy_file);
    assert_eq!(target, location::session_file(Some(&root)));
    assert!(target.starts_with(location::data_dir()));
}

#[test]
fn a_burst_of_changes_wakes_the_loop_once_quiet_and_writes_off_thread() {
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    static DEBOUNCE: super::debounce::Debounce = super::debounce::Debounce::new();
    DEBOUNCE.note_change();
    assert!(!DEBOUNCE.take_due(), "no waker, no loop to wake");
    DEBOUNCE.register_waker(Arc::new(move || {
        let _ = tx.lock().unwrap().send(());
    }));
    let started = Instant::now();
    for _ in 0..20 {
        DEBOUNCE.note_change();
    }
    rx.recv_timeout(Duration::from_secs(5))
        .expect("the debounce never woke the loop");
    assert!(started.elapsed() >= super::debounce::QUIET);
    assert!(DEBOUNCE.take_due());
    assert!(!DEBOUNCE.take_due(), "one wake per quiet period");
    assert!(rx.try_recv().is_err(), "a burst wakes the loop once");

    let dir = tempfile::tempdir().unwrap();
    let mut app = new_app();
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);
    type_text(&mut app, "draft");
    super::snapshot::submit(&mut app);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !session_file.exists() {
        assert!(
            Instant::now() < deadline,
            "the worker never wrote the session"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(session) = Session::load(&session_file)
            && session.tabs.first().and_then(|t| t.draft_text.as_deref()) == Some("draft")
        {
            assert!(
                session.needs_recovery(),
                "a debounced write is not a clean exit"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the draft never reached the session"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Restore time for 20 tabs, each 2,000 lines with an undo history.
/// Recorded in docs/PERFORMANCE.md; not gated (CLAUDE.md rule 3).
#[test]
fn restore_twenty_tabs_reports_its_time() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = new_app();
    app.ui.session.data_dir = Some(dir.path().join("data"));
    let session_file = crate::session::snapshot::current_file(&app);
    let text = "let value = compute(42); // a typical line of code\n".repeat(2_000);
    for i in 0..20 {
        let path = dir.path().join(format!("file{i}.rs"));
        std::fs::write(&path, &text).unwrap();
        open_file(&mut app, &path);
        type_text(&mut app, "edit ");
    }
    PlatformEventHandler::exiting(&mut app, &Ctx);

    let mut fresh = new_app();
    let start = Instant::now();
    assert!(restore_session(&mut fresh, &session_file).unwrap());
    let elapsed = start.elapsed();
    eprintln!("restore of 20 tabs took {elapsed:?}");
    assert_eq!(fresh.documents.entries.len(), 20);
    assert!(
        fresh
            .documents
            .entries
            .iter()
            .all(|e| e.history.can_undo() || e.tab_id == fresh.documents.active_tab.unwrap())
    );
    assert!(fresh.history.can_undo());
}
