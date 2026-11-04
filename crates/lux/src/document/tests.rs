use super::*;
use lux_platform::dialog::SaveDecision;
use lux_platform::{
    Clipboard, FileDialogOptions, FileDialogs, MouseButton, PlatformError, PlatformWindow,
    WindowAttributes,
};
use std::cell::Cell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Dialogs {
    open: Option<PathBuf>,
    save: Option<PathBuf>,
    decision: SaveDecision,
    threads: Mutex<Vec<std::thread::ThreadId>>,
}

impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        self.open.clone()
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        self.open.clone().into_iter().collect()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        self.save.clone()
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        self.decision
    }
}

struct Context {
    dialogs: Arc<Dialogs>,
    events: Receiver<()>,
    sender: Sender<()>,
    exited: Cell<bool>,
}

impl Context {
    fn new(open: Option<PathBuf>, save: Option<PathBuf>, decision: SaveDecision) -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            dialogs: Arc::new(Dialogs {
                open,
                save,
                decision,
                threads: Mutex::new(vec![]),
            }),
            events,
            sender,
            exited: Cell::new(false),
        }
    }
    fn complete(&self, app: &mut App) {
        self.events
            .recv_timeout(Duration::from_secs(10))
            .expect("worker must notify completion");
        app.finish_document_operation(self);
    }
}

impl Clipboard for Context {
    fn read_text(&self) -> Result<String, PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
    }
}

impl PlatformContext for Context {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        let sender = self.sender.clone();
        Arc::new(move || {
            sender.send(()).unwrap();
        })
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        self.dialogs.clone()
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless document test".into()))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        self.dialogs.as_ref()
    }
    fn exit(&self) {
        self.exited.set(true);
    }
    fn request_wakeup(&self, _: std::time::Instant) {
        panic!("document I/O must not use timer polling");
    }
}

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

#[test]
fn a_queued_path_opens_as_a_second_tab() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("monokai.rs");
    let second = dir.path().join("characters.txt");
    std::fs::write(&first, "fn main() {}\n").unwrap();
    std::fs::write(&second, "note\n").unwrap();
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.documents.open_queue.push(second.clone());
    app.request_document(&ctx, Request::Open(Some(first)));
    ctx.complete(&mut app);
    ctx.complete(&mut app);
    let titles: Vec<_> = app
        .documents
        .tabs
        .tabs()
        .iter()
        .map(|tab| tab.title.as_str())
        .collect();
    assert_eq!(titles, vec!["monokai.rs", "characters.txt"]);
    assert_eq!(app.documents.path.as_ref(), Some(&second));
    assert!(app.documents.open_queue.is_empty());
}

#[test]
fn reopening_the_current_file_after_confirming_save_keeps_the_saved_content() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note");
    std::fs::write(&path, "original").unwrap();
    let ctx = Context::new(None, None, SaveDecision::Save);
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(path.clone())));
    ctx.complete(&mut app);
    app.insert_at_cursor("new ");
    app.request_document(&ctx, Request::Open(Some(path.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "new original");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "new original");
    assert!(!app.document_dirty());
}

#[test]
fn open_edit_save_and_undo_use_real_files_and_worker_notifications() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("document.txt");
    std::fs::write(&path, "café\r\n").unwrap();
    let ctx = Context::new(Some(path.clone()), None, SaveDecision::Save);
    let mut app = app();
    app.request_document(&ctx, Request::Open(None));
    assert!(app.editing_blocked());
    // A native dialog can occlude the drawable without a matching winit
    // reveal event; worker completion must resume acquisition explicitly.
    app.presentation.set_hidden(true);
    ctx.complete(&mut app);
    assert!(app.presentation.ready(std::time::Instant::now()));
    assert!(app.scheduler.needs_repaint(std::time::Instant::now()));
    assert_eq!(app.buffer.text(), "café\r\n");
    assert!(!app.document_dirty());
    app.insert_at_cursor("new ");
    assert!(app.document_dirty());
    app.request_document(&ctx, Request::Save { save_as: false });
    ctx.complete(&mut app);
    assert!(!app.document_dirty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new café\r\n");
    assert_eq!(
        std::fs::read_to_string(path.with_file_name("document.txt.bak")).unwrap(),
        "café\r\n"
    );
    app.insert_at_cursor("extra ");
    assert!(app.document_dirty());
    app.history.undo(&mut app.buffer);
    assert!(!app.document_dirty());
    assert!(
        ctx.dialogs
            .threads
            .lock()
            .unwrap()
            .iter()
            .all(|id| *id != std::thread::current().id())
    );
}

#[test]
fn edits_during_save_are_not_marked_saved_by_an_older_completion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.txt");
    let ctx = Context::new(None, Some(path.clone()), SaveDecision::Save);
    let mut app = app();
    app.insert_at_cursor("snapshot");
    app.request_document(&ctx, Request::Save { save_as: false });
    assert!(!app.editing_blocked());
    app.insert_at_cursor(" + later");
    ctx.complete(&mut app);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "snapshot");
    assert_eq!(app.buffer.text(), "snapshot + later");
    assert!(app.document_dirty());
    app.history.undo(&mut app.buffer);
    assert!(!app.document_dirty());
}

#[test]
fn cancel_and_failed_save_never_close_or_discard_a_dirty_buffer() {
    let dir = tempfile::tempdir().unwrap();
    for ctx in [
        Context::new(None, None, SaveDecision::Cancel),
        Context::new(
            None,
            Some(dir.path().join("missing/child")),
            SaveDecision::Save,
        ),
    ] {
        let mut app = app();
        app.insert_at_cursor("keep me");
        app.request_document(&ctx, Request::Close);
        ctx.complete(&mut app);
        assert!(!ctx.exited.get());
        assert_eq!(app.buffer.text(), "keep me");
        assert!(app.document_dirty());
        assert!(!app.editing_blocked());
    }
}

#[test]
fn close_with_save_writes_before_exit_and_discard_does_not_write() {
    let dir = tempfile::tempdir().unwrap();
    for decision in [SaveDecision::Save, SaveDecision::Discard] {
        let path = dir.path().join(format!("{decision:?}.txt"));
        let ctx = Context::new(None, Some(path.clone()), decision);
        let mut app = app();
        app.insert_at_cursor("content");
        app.request_document(&ctx, Request::Close);
        ctx.complete(&mut app);
        assert!(ctx.exited.get());
        assert_eq!(path.exists(), decision == SaveDecision::Save);
        if decision == SaveDecision::Save {
            assert_eq!(std::fs::read(path).unwrap(), b"content");
        }
    }
}

#[test]
fn external_changes_are_preserved_and_failed_open_keeps_current_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note");
    std::fs::write(&path, "original").unwrap();
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(path.clone())));
    ctx.complete(&mut app);
    app.insert_at_cursor("edit ");
    std::fs::write(&path, "external").unwrap();
    app.request_document(&ctx, Request::Save { save_as: false });
    ctx.complete(&mut app);
    assert!(
        app.documents
            .message
            .as_ref()
            .unwrap()
            .contains("changed on disk")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "external");
    app.request_document(&ctx, Request::Open(Some(dir.path().join("absent"))));
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "edit original");
    assert_eq!(app.documents.path, Some(path));
    assert!(app.document_dirty());
}

#[test]
fn open_rejects_binary_and_enforces_large_file_readonly() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    std::fs::write(&path, b"binary\0data").unwrap();
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(path.clone())));
    ctx.complete(&mut app);
    assert!(app.documents.message.is_some());
    assert!(app.documents.path.is_none());
    std::fs::write(&path, "line\n".repeat(2_100_000)).unwrap();
    app.request_document(&ctx, Request::Open(Some(path)));
    ctx.complete(&mut app);
    assert!(app.buffer.is_huge_file_mode());
    let chars = app.buffer.len_chars();
    app.insert_at_cursor("blocked");
    assert_eq!(app.buffer.len_chars(), chars);
    assert!(!app.document_dirty());
}

#[test]
fn opening_second_file_creates_new_tab_and_switching_preserves_state() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("file1.txt");
    let file2 = dir.path().join("file2.txt");
    std::fs::write(&file1, "content 1").unwrap();
    std::fs::write(&file2, "content 2").unwrap();

    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();

    // First file replaces the empty untitled tab
    app.request_document(&ctx, Request::Open(Some(file1.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.documents.tabs.tabs().len(), 1);
    assert_eq!(app.buffer.text(), "content 1");
    let tab1_id = app.documents.active_tab.unwrap();

    // Edit file 1
    app.insert_at_cursor("prefix ");
    assert_eq!(app.buffer.text(), "prefix content 1");
    assert!(app.document_dirty());

    // Open second file -> creates a new tab
    app.request_document(&ctx, Request::Open(Some(file2.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.documents.tabs.tabs().len(), 2);
    assert_eq!(app.buffer.text(), "content 2");
    let tab2_id = app.documents.active_tab.unwrap();
    assert_ne!(tab1_id, tab2_id);

    // Switch to tab 1 -> restores edits, dirty state, buffer
    assert!(app.switch_to_tab(tab1_id));
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "prefix content 1");
    assert!(app.document_dirty());

    // Switch to next tab (tab 2)
    assert!(app.switch_to_next_tab());
    assert_eq!(app.documents.active_tab, Some(tab2_id));
    assert_eq!(app.buffer.text(), "content 2");
    assert!(!app.document_dirty());

    // Switch to prev tab (tab 1)
    assert!(app.switch_to_prev_tab());
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "prefix content 1");
}

#[test]
fn opening_already_opened_file_switches_to_existing_tab() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("a.rs");
    let file2 = dir.path().join("b.rs");
    std::fs::write(&file1, "fn a() {}").unwrap();
    std::fs::write(&file2, "fn b() {}").unwrap();

    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();

    app.request_document(&ctx, Request::Open(Some(file1.clone())));
    ctx.complete(&mut app);
    let tab1_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file2.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.documents.tabs.tabs().len(), 2);

    // Reopen file 1 -> should activate tab 1 without adding a 3rd tab
    app.request_document(&ctx, Request::Open(Some(file1.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.documents.tabs.tabs().len(), 2);
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "fn a() {}");
}

#[test]
fn closing_tab_with_multiple_open_does_not_exit_app_and_reopen_restores_it() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("one.txt");
    let file2 = dir.path().join("two.txt");
    std::fs::write(&file1, "one").unwrap();
    std::fs::write(&file2, "two").unwrap();

    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();

    app.request_document(&ctx, Request::Open(Some(file1.clone())));
    ctx.complete(&mut app);
    let tab1_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file2.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.documents.tabs.tabs().len(), 2);

    // Close tab 2 (active)
    app.request_document(&ctx, Request::Close);
    ctx.complete(&mut app);

    // App must NOT have exited because tab 1 is still open
    assert!(!ctx.exited.get());
    assert_eq!(app.documents.tabs.tabs().len(), 1);
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "one");

    // Reopening last closed tab restores tab 2
    assert!(app.reopen_last_closed_tab());
    assert_eq!(app.documents.tabs.tabs().len(), 2);
    assert_eq!(app.buffer.text(), "two");
}

#[test]
fn mouse_and_keyboard_tab_interactions() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("t1.txt");
    let file2 = dir.path().join("t2.txt");
    std::fs::write(&file1, "tab one").unwrap();
    std::fs::write(&file2, "tab two").unwrap();

    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.viewport.width = 800.0;

    app.request_document(&ctx, Request::Open(Some(file1)));
    ctx.complete(&mut app);
    let tab1_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file2)));
    ctx.complete(&mut app);
    let tab2_id = app.documents.active_tab.unwrap();
    assert_eq!(app.documents.active_tab, Some(tab2_id));

    // Mouse click on first tab (x = 10.0, y = 10.0 in tab bar)
    app.mouse.position = dpi::PhysicalPosition::new(10.0, 10.0);
    assert!(app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left));
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "tab one");

    // Middle click on second tab closes tab 2
    let tab_w = app.tab_width();
    app.mouse.position = dpi::PhysicalPosition::new((tab_w + 10.0) as f64, 10.0);
    assert!(app.handle_mouse_click_on_chrome(&ctx, MouseButton::Middle));
    assert_eq!(app.documents.tabs.tabs().len(), 1);
    assert_eq!(app.documents.active_tab, Some(tab1_id));

    // Keyboard shortcut to reopen last closed tab (Cmd/Ctrl+Shift+T)
    let key_event = lux_platform::event::KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("t".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    app.modifiers.shift = true;
    assert!(app.document_key(&ctx, &key_event));
    assert_eq!(app.documents.tabs.tabs().len(), 2);
    assert_eq!(app.buffer.text(), "tab two");
}

#[test]
fn tab_strip_2_quick_selection_and_context_menu_ops() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("a.txt");
    let file2 = dir.path().join("b.txt");
    let file3 = dir.path().join("c.txt");
    std::fs::write(&file1, "content A").unwrap();
    std::fs::write(&file2, "content B").unwrap();
    std::fs::write(&file3, "content C").unwrap();

    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.viewport.width = 1000.0;

    app.request_document(&ctx, Request::Open(Some(file1)));
    ctx.complete(&mut app);
    let tab1_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file2)));
    ctx.complete(&mut app);
    let tab2_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file3)));
    ctx.complete(&mut app);
    let tab3_id = app.documents.active_tab.unwrap();
    assert_eq!(app.documents.tabs.tabs().len(), 3);

    // Cmd+1 switches to tab 1
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    app.modifiers.shift = false;
    let key1 = lux_platform::event::KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("1".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    assert!(app.document_key(&ctx, &key1));
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "content A");

    // Cmd+9 switches to the last tab (tab 3)
    let key9 = lux_platform::event::KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("9".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    assert!(app.document_key(&ctx, &key9));
    assert_eq!(app.documents.active_tab, Some(tab3_id));
    assert_eq!(app.buffer.text(), "content C");

    // Right click on tab 2 opens context menu
    let tab_w = app.tab_width();
    app.mouse.position = dpi::PhysicalPosition::new((tab_w + 10.0) as f64, 10.0);
    assert!(app.handle_mouse_click_on_chrome(&ctx, MouseButton::Right));
    assert!(app.documents.tab_context_menu.is_some());
    let menu = app.documents.tab_context_menu.as_ref().unwrap();
    assert_eq!(menu.tab_id, tab2_id);

    // Pin tab 2
    app.toggle_pin_tab(tab2_id);
    assert!(
        app.documents
            .tabs
            .tabs()
            .iter()
            .find(|t| t.id == tab2_id)
            .unwrap()
            .pinned
    );
    app.toggle_pin_tab(tab2_id);
    assert!(
        !app.documents
            .tabs
            .tabs()
            .iter()
            .find(|t| t.id == tab2_id)
            .unwrap()
            .pinned
    );

    // Reorder tab 1 to position 2
    assert!(app.reorder_tab(tab1_id, 2));
    assert_eq!(app.documents.tabs.tabs()[2].id, tab1_id);

    // Close others from tab 2 closes tab 1 and tab 3
    app.close_other_tabs(&ctx, tab2_id);
    assert_eq!(app.documents.tabs.tabs().len(), 1);
    assert_eq!(app.documents.tabs.tabs()[0].id, tab2_id);
}
