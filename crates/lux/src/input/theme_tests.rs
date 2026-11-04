use std::sync::Arc;

use crate::app::App;
use crate::menu::{MenuAction, handle_menu_action};
use crate::palette::execute_command;
use crate::theme_picker::handle_theme_picker_key;
use lux_core::buffer::Buffer;
use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{FileDialogOptions, FileDialogs, Key, NamedKey, PlatformContext};

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<std::path::PathBuf> {
        vec![]
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
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
impl PlatformContext for DummyContext {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(DummyDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
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

fn test_app() -> App {
    let font_bytes = include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec();
    App::new(font_bytes)
}

fn key_event(key: Key) -> KeyEvent {
    KeyEvent {
        state: ElementState::Pressed,
        key,
        text: None,
        repeat: false,
        physical: None,
    }
}

#[test]
fn test_sublime_themes_switch_ui_and_syntax() {
    let mut app = test_app();
    app.buffer = Buffer::from_bytes(
        b"fn calculate(val: i32) -> String {\n    println!(\"Result: {}\", val * 42);\n}\n",
    )
    .unwrap();
    app.set_language_mode(Some(lux_syntax::LanguageId::Rust));
    assert!(app.syntax.is_some());

    // 1. Monokai
    app.set_theme("monokai");
    assert_eq!(app.chrome.theme.name, "Monokai");
    assert_eq!(
        app.chrome.theme.background,
        lux_ui::Color::rgb(0x27, 0x28, 0x22)
    );
    assert_eq!(
        app.chrome.theme.foreground,
        lux_ui::Color::rgb(0xF8, 0xF8, 0xF2)
    );
    assert_eq!(
        app.chrome.theme.accent,
        lux_ui::Color::rgb(0xF9, 0x26, 0x72)
    );
    assert_eq!(app.chrome.theme.muted, lux_ui::Color::rgb(0x75, 0x71, 0x5E));

    let syn = app.syntax.as_ref().unwrap();
    assert_eq!(syn.theme.name, "Monokai");
    assert_eq!(
        syn.theme.color_for_scope("keyword"),
        lux_syntax::theme::Color::rgb(0xF9, 0x26, 0x72)
    );
    assert_eq!(
        syn.theme.color_for_scope("string"),
        lux_syntax::theme::Color::rgb(0xE6, 0xDB, 0x74)
    );
    assert_eq!(
        syn.theme.color_for_scope("function"),
        lux_syntax::theme::Color::rgb(0xA6, 0xE2, 0x2E)
    );
    assert_eq!(
        syn.theme.color_for_scope("number"),
        lux_syntax::theme::Color::rgb(0xAE, 0x81, 0xFF)
    );
    assert_eq!(
        syn.theme.color_for_scope("comment"),
        lux_syntax::theme::Color::rgb(0x75, 0x71, 0x5E)
    );

    // 2. Mariana
    app.set_theme("mariana");
    assert_eq!(app.chrome.theme.name, "Mariana");
    assert_eq!(
        app.chrome.theme.background,
        lux_ui::Color::rgb(0x30, 0x38, 0x41)
    );
    assert_eq!(
        app.chrome.theme.foreground,
        lux_ui::Color::rgb(0xD8, 0xDE, 0xE9)
    );
    assert_eq!(
        app.chrome.theme.accent,
        lux_ui::Color::rgb(0x5F, 0xB3, 0xB3)
    );

    let syn = app.syntax.as_ref().unwrap();
    assert_eq!(syn.theme.name, "Mariana");
    assert_eq!(
        syn.theme.color_for_scope("keyword"),
        lux_syntax::theme::Color::rgb(0xC5, 0x94, 0xC5)
    );
    assert_eq!(
        syn.theme.color_for_scope("string"),
        lux_syntax::theme::Color::rgb(0x99, 0xC7, 0x94)
    );
    assert_eq!(
        syn.theme.color_for_scope("function"),
        lux_syntax::theme::Color::rgb(0x66, 0x99, 0xCC)
    );

    // 3. Breakers
    app.set_theme("breakers");
    assert_eq!(app.chrome.theme.name, "Breakers");
    assert_eq!(
        app.chrome.theme.background,
        lux_ui::Color::rgb(0x0C, 0x10, 0x14)
    );
    assert_eq!(
        app.chrome.theme.foreground,
        lux_ui::Color::rgb(0xFF, 0xFF, 0xFF)
    );
    assert_eq!(
        app.chrome.theme.accent,
        lux_ui::Color::rgb(0x00, 0xD0, 0xD0)
    );

    let syn = app.syntax.as_ref().unwrap();
    assert_eq!(syn.theme.name, "Breakers");
    assert_eq!(
        syn.theme.color_for_scope("keyword"),
        lux_syntax::theme::Color::rgb(0xFF, 0x6B, 0x8B)
    );
    assert_eq!(
        syn.theme.color_for_scope("string"),
        lux_syntax::theme::Color::rgb(0x5A, 0xF7, 0x8E)
    );

    // 4. Celeste
    app.set_theme("celeste");
    assert_eq!(app.chrome.theme.name, "Celeste");
    assert_eq!(
        app.chrome.theme.background,
        lux_ui::Color::rgb(0xF8, 0xF9, 0xFA)
    );
    assert_eq!(
        app.chrome.theme.foreground,
        lux_ui::Color::rgb(0x21, 0x25, 0x29)
    );
    assert_eq!(
        app.chrome.theme.accent,
        lux_ui::Color::rgb(0x00, 0x7A, 0xCC)
    );

    let syn = app.syntax.as_ref().unwrap();
    assert_eq!(syn.theme.name, "Celeste");
    assert_eq!(
        syn.theme.color_for_scope("function"),
        lux_syntax::theme::Color::rgb(0x40, 0x78, 0xF2)
    );
}

#[test]
fn test_theme_picker_modal_navigation_and_preview() {
    let mut app = test_app();
    let ctx = DummyContext;
    app.chrome.theme = lux_ui::Theme::LUX_DARK;

    app.open_theme_picker();
    assert!(app.ui.theme_picker.is_some());
    let picker = app.ui.theme_picker.as_ref().unwrap();
    assert_eq!(picker.original_theme_name, "Lux Dark");

    // Navigate down - live preview should update
    handle_theme_picker_key(&mut app, &ctx, &key_event(Key::Named(NamedKey::ArrowDown)));
    assert!(
        app.chrome.theme.name != "Lux Dark"
            || app.ui.theme_picker.as_ref().unwrap().selected_index > 0
    );

    // Navigate up
    handle_theme_picker_key(&mut app, &ctx, &key_event(Key::Named(NamedKey::ArrowUp)));

    // Escape - rollback to original theme and close
    handle_theme_picker_key(&mut app, &ctx, &key_event(Key::Named(NamedKey::Escape)));
    assert!(app.ui.theme_picker.is_none());
    assert_eq!(app.chrome.theme.name, "Lux Dark");
}

#[test]
fn test_theme_picker_filter_and_confirm() {
    let mut app = test_app();
    let ctx = DummyContext;
    app.chrome.theme = lux_ui::Theme::LUX_DARK;

    app.open_theme_picker();

    // Type "mono" to filter Monokai
    for c in ["m", "o", "n", "o"] {
        handle_theme_picker_key(&mut app, &ctx, &key_event(Key::Character(c.to_string())));
    }

    let picker = app.ui.theme_picker.as_ref().unwrap();
    assert_eq!(picker.filtered(), vec!["Monokai"]);
    // Live preview should be Monokai
    assert_eq!(app.chrome.theme.name, "Monokai");

    // Press Enter to confirm
    handle_theme_picker_key(&mut app, &ctx, &key_event(Key::Named(NamedKey::Enter)));
    assert!(app.ui.theme_picker.is_none());
    assert_eq!(app.chrome.theme.name, "Monokai");
    assert_eq!(app.active_syntax_theme().name, "Monokai");
}

#[test]
fn test_preferences_color_theme_palette_commands() {
    let mut app = test_app();
    let ctx = DummyContext;

    assert!(app.ui.theme_picker.is_none());
    execute_command(&mut app, &ctx, "preferences.color_theme");
    assert!(app.ui.theme_picker.is_some());

    app.ui.theme_picker = None;
    execute_command(&mut app, &ctx, "workbench.action.selectTheme");
    assert!(app.ui.theme_picker.is_some());
}

#[test]
fn test_menu_action_select_color_theme() {
    let mut app = test_app();
    let ctx = DummyContext;

    assert!(app.ui.theme_picker.is_none());
    handle_menu_action(&mut app, &ctx, MenuAction::SelectColorTheme);
    assert!(app.ui.theme_picker.is_some());
}

#[test]
fn test_set_language_mode_preserves_active_theme() {
    let mut app = test_app();
    app.set_theme("mariana");
    assert_eq!(app.chrome.theme.name, "Mariana");

    app.buffer = Buffer::from_bytes(b"def hello():\n    print('hi')\n").unwrap();
    app.set_language_mode(Some(lux_syntax::LanguageId::Python));

    let syn = app.syntax.as_ref().expect("syntax state must be created");
    assert_eq!(syn.theme.name, "Mariana");
    assert_eq!(
        syn.theme.color_for_scope("keyword"),
        lux_syntax::theme::Color::rgb(0xC5, 0x94, 0xC5)
    );
}
