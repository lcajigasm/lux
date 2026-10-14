//! Integration tests for ST16 Distraction Free Mode.

use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{Key, NamedKey, PlatformContext, PlatformError};

use crate::app::App;
use crate::menu::{MenuAction, handle_menu_action};

fn test_font() -> Vec<u8> {
    include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec()
}

struct TestDialogs;

impl lux_platform::FileDialogs for TestDialogs {
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
        lux_platform::dialog::SaveDecision::Cancel
    }
}

struct TestContext {
    dialogs: std::sync::Arc<TestDialogs>,
}

impl TestContext {
    fn new() -> Self {
        Self {
            dialogs: std::sync::Arc::new(TestDialogs),
        }
    }
}

impl PlatformContext for TestContext {
    fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        std::sync::Arc::new(|| {})
    }

    fn background_dialogs(&self) -> std::sync::Arc<dyn lux_platform::FileDialogs> {
        self.dialogs.clone()
    }

    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        self
    }

    fn file_dialogs(&self) -> &dyn lux_platform::FileDialogs {
        self.dialogs.as_ref()
    }

    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("test".into()))
    }

    fn exit(&self) {}

    fn request_wakeup(&self, _: std::time::Instant) {}
}

impl lux_platform::Clipboard for TestContext {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(String::new())
    }

    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}

#[test]
fn test_toggle_distraction_free_enters_and_exits() {
    let mut app = App::new(test_font());
    app.viewport.width = 1200.0;
    app.viewport.height = 800.0;

    // Normal mode initial chrome assertions
    assert!(!app.is_distraction_free());
    assert!(app.tab_bar_height() > 0.0);
    assert!(app.status_bar_height() > 0.0);

    // Toggle on
    app.toggle_distraction_free();
    assert!(app.is_distraction_free());
    assert_eq!(app.tab_bar_height(), 0.0);
    assert_eq!(app.status_bar_height(), 0.0);
    assert_eq!(app.sidebar_width(), 0.0);
    assert_eq!(app.minimap_width(), 0.0);

    // Toggle off restores normal mode
    app.toggle_distraction_free();
    assert!(!app.is_distraction_free());
    assert!(app.tab_bar_height() > 0.0);
    assert!(app.status_bar_height() > 0.0);
}

#[test]
fn test_distraction_free_centering_and_text_left() {
    let mut app = App::new(test_font());
    app.viewport.width = 1600.0;
    app.viewport.height = 900.0;

    let normal_text_left = app.text_left();
    assert!(normal_text_left < 100.0);

    app.enter_distraction_free();
    assert!(app.is_distraction_free());

    let df_text_left = app.text_left();
    let expected = ((app.viewport.width
        - app.distraction_free_column_chars() as f32 * app.editor_view.reference_width("0"))
        / 2.0)
        .max(24.0 * app.scale_factor);
    assert!((df_text_left - expected).abs() < 1e-4);
    assert!(df_text_left > normal_text_left);

    // Mouse hit test at text_left hits column 0
    app.mouse.position = dpi::PhysicalPosition::new(df_text_left as f64, 10.0);
    let pos = app.position_at_mouse();
    assert_eq!(pos.line, 0);
    assert_eq!(pos.column, 0);

    app.exit_distraction_free();
    assert_eq!(app.text_left(), normal_text_left);
}

#[test]
fn test_distraction_free_escape_behavior() {
    let mut app = App::new(test_font());
    app.insert_at_cursor("Hello World");

    app.enter_distraction_free();
    assert!(app.is_distraction_free());

    // Case 1: Active selection - first Escape collapses selection, remaining in distraction free mode
    let start = lux_core::position::Position::document_start();
    let end = lux_core::position::Position::from_char(&app.buffer, 5);
    app.cursors =
        lux_core::multicursor::MultiCursor::new(lux_core::selection::Selection::new(start, end));
    assert!(!app.cursors.primary().is_empty());

    let esc_event = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        state: ElementState::Pressed,
        repeat: false,
        text: None,
    };
    app.handle_key(&esc_event);
    assert!(app.cursors.primary().is_empty());
    assert!(app.is_distraction_free());

    // Case 2: Empty single cursor - next Escape exits distraction free mode
    app.handle_key(&esc_event);
    assert!(!app.is_distraction_free());
}

#[test]
fn test_distraction_free_keyboard_shortcuts() {
    let mut app = App::new(test_font());

    // Shift + F11
    let shift_f11 = KeyEvent {
        key: Key::Named(NamedKey::F(11)),
        state: ElementState::Pressed,
        repeat: false,
        text: None,
    };
    app.modifiers.shift = true;
    app.handle_key(&shift_f11);
    assert!(app.is_distraction_free());

    // Shift + F11 again exits
    app.handle_key(&shift_f11);
    assert!(!app.is_distraction_free());

    // Cmd + Ctrl + Shift + F (macOS shortcut)
    let f_key = KeyEvent {
        key: Key::Character("f".to_string()),
        state: ElementState::Pressed,
        repeat: false,
        text: Some("f".to_string()),
    };
    app.modifiers.meta = true;
    app.modifiers.control = true;
    app.modifiers.shift = true;
    app.handle_key(&f_key);
    assert!(app.is_distraction_free());

    app.handle_key(&f_key);
    assert!(!app.is_distraction_free());
}

#[test]
fn test_distraction_free_command_palette_and_keymap_dispatch() {
    let mut app = App::new(test_font());

    // Dispatch via keymap command name
    crate::input::dispatch::execute_keymap_command(&mut app, "toggle_distraction_free", None);
    assert!(app.is_distraction_free());

    crate::input::dispatch::execute_keymap_command(&mut app, "toggle_distraction_free", None);
    assert!(!app.is_distraction_free());

    // Dispatch via palette command id
    let ctx = TestContext::new();
    crate::palette::commands::execute_command(&mut app, &ctx, "view.toggle_distraction_free");
    assert!(app.is_distraction_free());

    crate::palette::commands::execute_command(&mut app, &ctx, "view.toggle_distraction_free");
    assert!(!app.is_distraction_free());
}

#[test]
fn test_distraction_free_menu_action() {
    let mut app = App::new(test_font());
    let ctx = TestContext::new();

    assert_eq!(
        MenuAction::from_tag(62),
        Some(MenuAction::ToggleDistractionFree)
    );
    assert_eq!(MenuAction::ToggleDistractionFree.tag(), 62);

    handle_menu_action(&mut app, &ctx, MenuAction::ToggleDistractionFree);
    assert!(app.is_distraction_free());

    handle_menu_action(&mut app, &ctx, MenuAction::ToggleDistractionFree);
    assert!(!app.is_distraction_free());
}
