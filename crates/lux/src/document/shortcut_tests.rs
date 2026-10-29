//! Unit and integration tests for tab switching shortcut isolation and chord delegation (ST-FIX-04).

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use lux_platform::dialog::SaveDecision;
use lux_platform::event::KeyEvent;
use lux_platform::{
    Clipboard, ElementState, FileDialogOptions, FileDialogs, Key, Modifiers, NamedKey,
    PlatformContext, PlatformError, PlatformWindow, WindowAttributes,
};

use super::Request;
use crate::app::App;

struct TestContext {
    sender: Sender<()>,
    events: Receiver<()>,
    exited: Cell<bool>,
}

impl TestContext {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            sender,
            events,
            exited: Cell::new(false),
        }
    }

    fn complete(&self, app: &mut App) {
        self.events
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("worker must notify completion");
        app.finish_document_operation(self);
    }
}

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        vec![]
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
}

impl Clipboard for TestContext {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
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
        Arc::new(DummyDialogs)
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless test".into()))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &DummyDialogs
    }
    fn exit(&self) {
        self.exited.set(true);
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn create_test_app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

#[test]
fn test_cmd_option_number_does_not_switch_tab_and_changes_layout() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("file1.txt");
    let file2 = dir.path().join("file2.txt");
    let file3 = dir.path().join("file3.txt");
    std::fs::write(&file1, "tab 1 content").unwrap();
    std::fs::write(&file2, "tab 2 content").unwrap();
    std::fs::write(&file3, "tab 3 content").unwrap();

    let ctx = TestContext::new();
    let mut app = create_test_app();
    app.viewport.width = 1000.0;

    app.request_document(&ctx, Request::Open(Some(file1)));
    ctx.complete(&mut app);
    let tab1_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file2)));
    ctx.complete(&mut app);
    let _tab2_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file3)));
    ctx.complete(&mut app);
    let _tab3_id = app.documents.active_tab.unwrap();

    // Switch to tab 1 initially
    assert!(app.switch_to_tab(tab1_id));
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "tab 1 content");
    assert!(app.ui.split.secondary.is_none());

    // Press Cmd+Option+2 (or Ctrl+Alt+2 on Linux)
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    app.modifiers.alt = true;
    app.modifiers.shift = false;

    let key_event = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("2".into()),
        text: None,
        repeat: false,
    };

    // document_key must return false (do not consume or switch tabs)
    assert!(!app.document_key(&ctx, &key_event));
    assert_eq!(app.documents.active_tab, Some(tab1_id));

    // Full keyboard event pipeline handles layout splitting
    app.handle_keyboard_event(&ctx, &key_event);
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.buffer.text(), "tab 1 content");
    assert!(app.ui.split.secondary.is_some());
    assert_eq!(
        app.ui.split.direction,
        crate::split::SplitDirection::Horizontal
    );
}

#[test]
fn test_cmd_k_followed_by_cmd_1_folds_code_without_altering_tab() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("doc1.txt");
    let file2 = dir.path().join("code.py");
    std::fs::write(&file1, "tab 1 text").unwrap();
    std::fs::write(&file2, "def foo():\n    a = 1\n    b = 2\n").unwrap();

    let ctx = TestContext::new();
    let mut app = create_test_app();
    app.viewport.width = 1000.0;

    app.request_document(&ctx, Request::Open(Some(file1)));
    ctx.complete(&mut app);
    let _tab1_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file2)));
    ctx.complete(&mut app);
    let tab2_id = app.documents.active_tab.unwrap();

    // Active tab is tab 2
    assert_eq!(app.documents.active_tab, Some(tab2_id));
    assert_eq!(app.fold_state.folded_ranges().len(), 0);

    // Step 1: User presses Cmd+K (or Ctrl+K)
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    app.modifiers.alt = false;
    app.modifiers.shift = false;

    let key_k = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("k".into()),
        text: None,
        repeat: false,
    };
    app.handle_keyboard_event(&ctx, &key_k);
    assert!(app.ui.pending_chord.is_some());
    assert_eq!(app.documents.active_tab, Some(tab2_id));

    // Step 2: User presses Cmd+1 (or Ctrl+1)
    let key_1 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("1".into()),
        text: None,
        repeat: false,
    };
    app.handle_keyboard_event(&ctx, &key_1);
    assert!(app.ui.pending_chord.is_none());
    assert_eq!(app.documents.active_tab, Some(tab2_id));
    assert_eq!(app.buffer.text(), "def foo():\n    a = 1\n    b = 2\n");
    // Code blocks are now folded!
    assert!(!app.fold_state.folded_ranges().is_empty());
}

#[test]
fn test_cmd_k_followed_by_digit_without_cmd_folds_code() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("code.py");
    std::fs::write(&file, "def foo():\n    a = 1\n    b = 2\n").unwrap();

    let ctx = TestContext::new();
    let mut app = create_test_app();
    app.viewport.width = 1000.0;

    app.request_document(&ctx, Request::Open(Some(file)));
    ctx.complete(&mut app);
    let tab_id = app.documents.active_tab.unwrap();

    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    app.modifiers.alt = false;
    app.modifiers.shift = false;

    let key_k = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("k".into()),
        text: None,
        repeat: false,
    };
    app.handle_keyboard_event(&ctx, &key_k);
    assert!(app.ui.pending_chord.is_some());

    // Release modifiers before pressing 1
    app.modifiers = Modifiers::default();
    let key_1 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("1".into()),
        text: None,
        repeat: false,
    };
    app.handle_keyboard_event(&ctx, &key_1);
    assert!(app.ui.pending_chord.is_none());
    assert_eq!(app.documents.active_tab, Some(tab_id));
    assert!(!app.fold_state.folded_ranges().is_empty());
}

#[test]
fn test_cmd_k_escape_cancels_chord() {
    let ctx = TestContext::new();
    let mut app = create_test_app();

    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    let key_k = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("k".into()),
        text: None,
        repeat: false,
    };
    app.handle_keyboard_event(&ctx, &key_k);
    assert!(app.ui.pending_chord.is_some());

    app.modifiers = Modifiers::default();
    let escape = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Named(NamedKey::Escape),
        text: None,
        repeat: false,
    };
    app.handle_keyboard_event(&ctx, &escape);
    assert!(app.ui.pending_chord.is_none());
}

#[test]
fn test_tab_switching_strictly_requires_exclusive_modifiers() {
    let dir = tempfile::tempdir().unwrap();
    let file1 = dir.path().join("a.txt");
    let file2 = dir.path().join("b.txt");
    std::fs::write(&file1, "A").unwrap();
    std::fs::write(&file2, "B").unwrap();

    let ctx = TestContext::new();
    let mut app = create_test_app();
    app.viewport.width = 1000.0;

    app.request_document(&ctx, Request::Open(Some(file1)));
    ctx.complete(&mut app);
    let tab1_id = app.documents.active_tab.unwrap();

    app.request_document(&ctx, Request::Open(Some(file2)));
    ctx.complete(&mut app);
    let tab2_id = app.documents.active_tab.unwrap();

    assert!(app.switch_to_tab(tab1_id));

    let key2 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("2".into()),
        text: None,
        repeat: false,
    };

    // Case 1: Shift is pressed (Cmd+Shift+2) -> must NOT switch tab
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift: true,
        alt: false,
    };
    assert!(!app.document_key(&ctx, &key2));
    assert_eq!(app.documents.active_tab, Some(tab1_id));

    // Case 2: Alt is pressed (Cmd+Alt+2) -> must NOT switch tab
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift: false,
        alt: true,
    };
    assert!(!app.document_key(&ctx, &key2));
    assert_eq!(app.documents.active_tab, Some(tab1_id));

    // Case 3: On macOS, Control is pressed (Cmd+Ctrl+2 or Ctrl+2) -> must NOT switch tab
    if cfg!(target_os = "macos") {
        app.modifiers = Modifiers {
            meta: false,
            control: true,
            shift: false,
            alt: false,
        };
        assert!(!app.document_key(&ctx, &key2));
        assert_eq!(app.documents.active_tab, Some(tab1_id));
    }

    // Case 4: Strictly exclusive command modifier -> DOES switch tab
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift: false,
        alt: false,
    };
    assert!(app.document_key(&ctx, &key2));
    assert_eq!(app.documents.active_tab, Some(tab2_id));
    assert_eq!(app.buffer.text(), "B");
}
