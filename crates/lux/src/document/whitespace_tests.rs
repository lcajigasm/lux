//! Integration tests for automatic and on-demand whitespace cleanup (Sublime Parity ST17).

use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lux_core::multicursor::MultiCursor;
use lux_core::selection::Selection;
use lux_platform::dialog::SaveDecision;
use lux_platform::{
    Clipboard, FileDialogOptions, FileDialogs, PlatformContext, PlatformError, PlatformWindow,
    WindowAttributes,
};

use super::Request;
use crate::app::App;

struct MockDialogs {
    open: Option<PathBuf>,
    save: Option<PathBuf>,
    decision: SaveDecision,
    threads: Mutex<Vec<std::thread::ThreadId>>,
}

impl FileDialogs for MockDialogs {
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

struct TestContext {
    dialogs: Arc<MockDialogs>,
    events: Receiver<()>,
    sender: Sender<()>,
    exited: Cell<bool>,
}

impl TestContext {
    fn new(open: Option<PathBuf>, save: Option<PathBuf>, decision: SaveDecision) -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            dialogs: Arc::new(MockDialogs {
                open,
                save,
                decision,
                threads: Mutex::new(Vec::new()),
            }),
            events,
            sender,
            exited: Cell::new(false),
        }
    }

    fn complete(&self, app: &mut App) {
        self.events
            .recv_timeout(Duration::from_secs(10))
            .expect("document I/O worker should notify completion");
        app.finish_document_operation(self);
    }
}

impl Clipboard for TestContext {
    fn read_text(&self) -> Result<String, PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
    }
}

impl PlatformContext for TestContext {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        let sender = self.sender.clone();
        Arc::new(move || {
            let _ = sender.send(());
        })
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        self.dialogs.clone()
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless test".into()))
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
        panic!("unexpected timer polling in document test");
    }
}

fn test_app(temp: &tempfile::TempDir) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let settings_path = temp.path().join("settings.json");
    app.ui.settings = crate::settings::SettingsState::new(settings_path);
    app
}

#[test]
fn test_save_trims_trailing_whitespace_when_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let file_path = temp.path().join("dirty_whitespace.txt");
    fs::write(&file_path, "alpha   \nbeta\t\t\ngamma\n").unwrap();

    let mut app = test_app(&temp);
    let ctx = TestContext::new(Some(file_path.clone()), None, SaveDecision::Discard);
    app.request_document(&ctx, Request::Open(Some(file_path.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "alpha   \nbeta\t\t\ngamma\n");

    // Enable trimTrailingWhitespace
    app.update_setting(
        "editor.trimTrailingWhitespace",
        serde_json::Value::Bool(true),
    );

    // Perform save
    app.request_document(&ctx, Request::Save { save_as: false });
    ctx.complete(&mut app);

    // Verify disk content and in-memory buffer are trimmed
    let on_disk = fs::read_to_string(&file_path).unwrap();
    assert_eq!(on_disk, "alpha\nbeta\ngamma\n");
    assert_eq!(app.buffer.text(), "alpha\nbeta\ngamma\n");
    assert!(!app.document_dirty());

    // Undo restores the exact trailing whitespace
    let restored = app.history.undo(&mut app.buffer);
    assert!(restored.is_some());
    assert_eq!(app.buffer.text(), "alpha   \nbeta\t\t\ngamma\n");
    assert!(app.document_dirty());

    // Redo re-trims cleanly
    let redone = app.history.redo(&mut app.buffer);
    assert!(redone.is_some());
    assert_eq!(app.buffer.text(), "alpha\nbeta\ngamma\n");
    assert!(!app.document_dirty());
}

#[test]
fn test_save_does_not_trim_when_disabled() {
    let temp = tempfile::tempdir().unwrap();
    let file_path = temp.path().join("keep_whitespace.txt");
    fs::write(&file_path, "alpha   \n").unwrap();

    let mut app = test_app(&temp);
    let ctx = TestContext::new(Some(file_path.clone()), None, SaveDecision::Discard);
    app.request_document(&ctx, Request::Open(Some(file_path.clone())));
    ctx.complete(&mut app);

    // Explicitly disable trimTrailingWhitespace
    app.update_setting(
        "editor.trimTrailingWhitespace",
        serde_json::Value::Bool(false),
    );

    app.request_document(&ctx, Request::Save { save_as: false });
    ctx.complete(&mut app);

    let on_disk = fs::read_to_string(&file_path).unwrap();
    assert_eq!(on_disk, "alpha   \n");
    assert_eq!(app.buffer.text(), "alpha   \n");
}

#[test]
fn test_save_ensures_final_newline_when_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let file_path = temp.path().join("no_newline.txt");
    fs::write(&file_path, "hello world").unwrap();

    let mut app = test_app(&temp);
    let ctx = TestContext::new(Some(file_path.clone()), None, SaveDecision::Discard);
    app.request_document(&ctx, Request::Open(Some(file_path.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "hello world");

    // Ensure insertFinalNewline is true
    app.update_setting("editor.insertFinalNewline", serde_json::Value::Bool(true));

    app.request_document(&ctx, Request::Save { save_as: false });
    ctx.complete(&mut app);

    let on_disk = fs::read_to_string(&file_path).unwrap();
    assert_eq!(on_disk, "hello world\n");
    assert_eq!(app.buffer.text(), "hello world\n");
    assert!(!app.document_dirty());
}

#[test]
fn test_save_does_not_insert_final_newline_when_disabled() {
    let temp = tempfile::tempdir().unwrap();
    let file_path = temp.path().join("keep_no_newline.txt");
    fs::write(&file_path, "hello world").unwrap();

    let mut app = test_app(&temp);
    let ctx = TestContext::new(Some(file_path.clone()), None, SaveDecision::Discard);
    app.request_document(&ctx, Request::Open(Some(file_path.clone())));
    ctx.complete(&mut app);

    // Disable insertFinalNewline
    app.update_setting("editor.insertFinalNewline", serde_json::Value::Bool(false));

    app.request_document(&ctx, Request::Save { save_as: false });
    ctx.complete(&mut app);

    let on_disk = fs::read_to_string(&file_path).unwrap();
    assert_eq!(on_disk, "hello world");
    assert_eq!(app.buffer.text(), "hello world");
}

#[test]
fn test_save_preserves_whitespace_on_cursor_line() {
    let temp = tempfile::tempdir().unwrap();
    let file_path = temp.path().join("preserve_caret.txt");
    fs::write(&file_path, "line 0   \nline 1   \nline 2   \n").unwrap();

    let mut app = test_app(&temp);
    let ctx = TestContext::new(Some(file_path.clone()), None, SaveDecision::Discard);
    app.request_document(&ctx, Request::Open(Some(file_path.clone())));
    ctx.complete(&mut app);

    // Place cursor on line 1
    let pos = lux_core::movement::position_at_line_column(&app.buffer, 1, 2);
    app.cursors = MultiCursor::new(Selection::cursor(pos));
    assert_eq!(app.cursors.primary().head.line, 1);

    // Configure trimTrailingWhitespace = true and trimTrailingWhitespacePreserveCursor = true
    app.update_setting(
        "editor.trimTrailingWhitespace",
        serde_json::Value::Bool(true),
    );
    app.update_setting(
        "editor.trimTrailingWhitespacePreserveCursor",
        serde_json::Value::Bool(true),
    );

    app.request_document(&ctx, Request::Save { save_as: false });
    ctx.complete(&mut app);

    let on_disk = fs::read_to_string(&file_path).unwrap();
    assert_eq!(on_disk, "line 0\nline 1   \nline 2\n");
    assert_eq!(app.buffer.text(), "line 0\nline 1   \nline 2\n");
}

#[test]
fn test_manual_whitespace_commands_and_palette_dispatch() {
    let temp = tempfile::tempdir().unwrap();
    let mut app = test_app(&temp);
    let ctx = TestContext::new(None, None, SaveDecision::Discard);
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"foo   \nbar").unwrap();

    // Palette command: file.trim_trailing_whitespace
    crate::palette::commands::execute_command(&mut app, &ctx, "file.trim_trailing_whitespace");
    assert_eq!(app.buffer.text(), "foo\nbar");

    // Palette command: file.ensure_final_newline
    crate::palette::commands::execute_command(&mut app, &ctx, "file.ensure_final_newline");
    assert_eq!(app.buffer.text(), "foo\nbar\n");

    // Undo ensure_final_newline
    app.history.undo(&mut app.buffer);
    assert_eq!(app.buffer.text(), "foo\nbar");

    // Undo trim_trailing_whitespace
    app.history.undo(&mut app.buffer);
    assert_eq!(app.buffer.text(), "foo   \nbar");
}
