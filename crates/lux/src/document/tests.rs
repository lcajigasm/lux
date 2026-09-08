use super::*;
use lux_platform::{Clipboard, PlatformError, PlatformWindow, WindowAttributes};
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
