use super::*;
use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey};

struct DummyDialogs;
impl lux_platform::FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<std::path::PathBuf> {
        vec![]
    }
    fn pick_folder(&self, _: &lux_platform::FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn save_file(&self, _: &lux_platform::FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
        lux_platform::dialog::SaveDecision::Discard
    }
}
static DUMMY_DIALOGS: DummyDialogs = DummyDialogs;

struct DummyContext;
impl lux_platform::Clipboard for DummyContext {
    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
        Ok(())
    }
}
impl lux_platform::PlatformContext for DummyContext {
    fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        std::sync::Arc::new(|| {})
    }
    fn background_dialogs(&self) -> std::sync::Arc<dyn lux_platform::FileDialogs> {
        std::sync::Arc::new(DummyDialogs)
    }
    fn file_dialogs(&self) -> &dyn lux_platform::FileDialogs {
        &DUMMY_DIALOGS
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
    fn request_wakeup(&self, _: std::time::Instant) {}
    fn exit(&self) {}
}

fn test_app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

#[test]
fn test_toggle_bookmark_via_keymap() {
    let text = "line 0\nline 1\nline 2\nline 3\nline 4\n";
    let mut app = test_app(text);
    let ctx = DummyContext;

    // Initially no bookmarks
    assert!(app.bookmarks.is_empty());

    // Place cursor on line 2
    let byte2 = app.buffer.line_start_byte(2).unwrap();
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::from_byte(
        &app.buffer,
        byte2,
    )));

    // Cmd+F2 (or Ctrl+F2)
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }

    let toggle_key = KeyEvent {
        key: Key::Named(NamedKey::F(2)),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    app.handle_key_with_clipboard(&toggle_key, Some(&ctx));
    assert!(app.bookmarks.contains(2));
    assert_eq!(app.bookmarks.len(), 1);

    // Press again to toggle off
    app.handle_key_with_clipboard(&toggle_key, Some(&ctx));
    assert!(!app.bookmarks.contains(2));
    assert!(app.bookmarks.is_empty());
}

#[test]
fn test_next_and_prev_bookmark_navigation() {
    let text = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\n";
    let mut app = test_app(text);
    let ctx = DummyContext;

    app.bookmarks.insert(1);
    app.bookmarks.insert(4);
    app.bookmarks.insert(7);

    // Cursor starts at line 0
    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::document_start()));

    let next_key = KeyEvent {
        key: Key::Named(NamedKey::F(2)),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };

    // F2 -> line 1
    app.handle_key_with_clipboard(&next_key, Some(&ctx));
    assert_eq!(app.cursors.primary().head.line, 1);

    // F2 -> line 4
    app.handle_key_with_clipboard(&next_key, Some(&ctx));
    assert_eq!(app.cursors.primary().head.line, 4);

    // F2 -> line 7
    app.handle_key_with_clipboard(&next_key, Some(&ctx));
    assert_eq!(app.cursors.primary().head.line, 7);

    // F2 -> wraps back to line 1
    app.handle_key_with_clipboard(&next_key, Some(&ctx));
    assert_eq!(app.cursors.primary().head.line, 1);

    // Shift+F2 -> jumps backward to line 7
    app.modifiers.shift = true;
    app.handle_key_with_clipboard(&next_key, Some(&ctx));
    assert_eq!(app.cursors.primary().head.line, 7);

    // Shift+F2 -> line 4
    app.handle_key_with_clipboard(&next_key, Some(&ctx));
    assert_eq!(app.cursors.primary().head.line, 4);
}

#[test]
fn test_clear_bookmarks_via_keymap() {
    let text = "line 0\nline 1\nline 2\nline 3\n";
    let mut app = test_app(text);
    let ctx = DummyContext;

    app.bookmarks.insert(0);
    app.bookmarks.insert(2);
    assert_eq!(app.bookmarks.len(), 2);

    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    app.modifiers.shift = true;

    let clear_key = KeyEvent {
        key: Key::Named(NamedKey::F(2)),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };

    app.handle_key_with_clipboard(&clear_key, Some(&ctx));
    assert!(app.bookmarks.is_empty());
}

#[test]
fn test_select_all_bookmarks_via_keymap() {
    let text = "alpha\nbravo\ncharlie\ndelta\necho\n";
    let mut app = test_app(text);
    let ctx = DummyContext;

    app.bookmarks.insert(1);
    app.bookmarks.insert(3);
    app.bookmarks.insert(4);

    app.modifiers.alt = true;
    let select_all_key = KeyEvent {
        key: Key::Named(NamedKey::F(2)),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };

    app.handle_key_with_clipboard(&select_all_key, Some(&ctx));

    let cursors = app.cursors.cursors();
    assert_eq!(cursors.len(), 3);
    assert_eq!(cursors[0].head.line, 1);
    assert_eq!(cursors[1].head.line, 3);
    assert_eq!(cursors[2].head.line, 4);
}

#[test]
fn test_bookmark_navigation_unfolds_hidden_lines() {
    let text = "header:\n    body1\n    body2\n    body3\nfooter\n";
    let mut app = test_app(text);
    app.refresh_foldable_ranges();

    // Fold lines 1..=3 under line 0
    app.fold_state.fold(0);
    assert!(app.fold_state.is_line_hidden(2));

    // Place bookmark inside hidden fold
    app.bookmarks.insert(2);

    // Primary cursor at document start
    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::document_start()));

    app.next_bookmark();

    // Jumping to bookmark unfolded the block and placed cursor at line 2
    assert!(!app.fold_state.is_line_hidden(2));
    assert_eq!(app.cursors.primary().head.line, 2);
}

#[test]
fn test_mouse_gutter_click_toggles_bookmark() {
    let text = "alpha\nbravo\ncharlie\n";
    let mut app = test_app(text);

    // Click near left gutter margin at line 1
    let tab_bar_h = app.tab_bar_height();
    app.mouse.position = dpi::PhysicalPosition::new(5.0, (tab_bar_h + LINE_HEIGHT + 2.0) as f64);
    app.begin_mouse_selection();

    assert!(app.bookmarks.contains(1));

    // Click again to toggle off
    app.begin_mouse_selection();
    assert!(!app.bookmarks.contains(1));
}
