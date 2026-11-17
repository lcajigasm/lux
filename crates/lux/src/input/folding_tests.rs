use super::*;
use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, MouseButton, NamedKey};

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
    app.refresh_foldable_ranges();
    app
}

#[test]
fn test_folding_discovery_and_toggle() {
    let code = "fn foo() {\n    let a = 1;\n    let b = 2;\n}\n\nfn bar() {\n    let c = 3;\n}\n";
    let mut app = test_app(code);

    assert!(app.fold_state.is_line_foldable(0));
    assert!(app.fold_state.is_line_foldable(5));

    // Place cursor on line 0
    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::document_start()));
    app.fold_at_cursor();
    assert!(app.fold_state.is_line_folded(0));

    let total_visual = app.fold_state.total_visual_lines(app.buffer.len_lines());
    assert_eq!(total_visual, app.buffer.len_lines() - 2);

    // Unfold at cursor
    app.unfold_at_cursor();
    assert!(!app.fold_state.is_line_folded(0));
    assert_eq!(
        app.fold_state.total_visual_lines(app.buffer.len_lines()),
        app.buffer.len_lines()
    );
}

#[test]
fn test_arrow_keys_skip_folded_lines() {
    let code = "header\n    hidden1\n    hidden2\nfooter\n";
    let mut app = test_app(code);

    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::document_start()));
    app.fold_at_cursor();
    assert!(app.fold_state.is_line_folded(0));

    // Press ArrowDown
    app.handle_key(&KeyEvent {
        key: Key::Named(NamedKey::ArrowDown),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });

    // Should skip hidden lines 1 and 2 and land on line 3 ("footer")
    assert_eq!(app.cursors.primary().head.line, 3);

    // Press ArrowUp
    app.handle_key(&KeyEvent {
        key: Key::Named(NamedKey::ArrowUp),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });

    // Should jump back to line 0 ("header")
    assert_eq!(app.cursors.primary().head.line, 0);
}

#[test]
fn test_fold_all_and_unfold_all() {
    let code = "fn foo() {\n    let a = 1;\n}\n\nfn bar() {\n    let b = 2;\n}\n";
    let mut app = test_app(code);

    app.fold_all();
    assert!(app.fold_state.is_line_folded(0));
    assert!(app.fold_state.is_line_folded(4));

    app.unfold_all();
    assert!(!app.fold_state.is_line_folded(0));
    assert!(!app.fold_state.is_line_folded(4));
}

#[test]
fn test_fold_by_level() {
    let code = "lvl0\n    lvl1\n        lvl2\n            lvl3\n";
    let mut app = test_app(code);

    app.fold_by_level(2);
    // lvl1 is nested at level 2, should be folded
    assert!(app.fold_state.is_line_folded(1));
    // lvl0 (level 1) and lvl2 (level 3) should not be folded
    assert!(!app.fold_state.is_line_folded(0));
    assert!(!app.fold_state.is_line_folded(2));

    app.fold_by_level(3);
    // lvl2 is nested at level 3, should now be folded
    assert!(app.fold_state.is_line_folded(2));
}

#[test]
fn test_gutter_fold_toggle_via_mouse() {
    let code = "fn foo() {\n    let a = 1;\n    let b = 2;\n}\n";
    let mut app = test_app(code);

    // Set viewport dimensions
    app.viewport.height = 600.0;
    app.viewport.width = 800.0;

    let ctx = DummyContext;

    // Line 0 is at mouse_y ~ tab_bar_height + 5.0, mouse_x in gutter
    let line_h = crate::app::LINE_HEIGHT * app.scale_factor;
    app.mouse.position.x = 10.0;
    app.mouse.position.y = (app.tab_bar_height() + line_h * 0.5) as f64;

    assert!(app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left));
    assert!(app.fold_state.is_line_folded(0));

    // Click again to unfold
    assert!(app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left));
    assert!(!app.fold_state.is_line_folded(0));
}

#[test]
fn test_inline_pill_unfold_click() {
    let code = "fn foo() {\n    let a = 1;\n}\n";
    let mut app = test_app(code);

    app.viewport.height = 600.0;
    app.viewport.width = 800.0;

    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::document_start()));
    app.fold_at_cursor();
    assert!(app.fold_state.is_line_folded(0));

    // Mouse positioned at end of line 0 where pill marker is located
    let line_h = crate::app::LINE_HEIGHT * app.scale_factor;
    let digits = app.buffer.len_lines().max(1).to_string().len();
    let gutter_w = app.editor_view.reference_width(&"0".repeat(digits));
    let text_left = crate::render::text::gutter_total_width(gutter_w, app.scale_factor)
        + 6.0 * app.scale_factor;

    let content = app.buffer.line_content(0).unwrap_or_default();
    let trimmed = content.trim_end_matches(['\r', '\n']);
    let text_w = app.editor_view.reference_width(trimmed);
    let pill_x = text_left + text_w + 10.0 * app.scale_factor;

    app.mouse.position.x = pill_x as f64;
    app.mouse.position.y = (app.tab_bar_height() + line_h * 0.5) as f64;

    // Trigger editor mouse press via begin_mouse_selection
    app.begin_mouse_selection(1);

    // Pill click should have unfolded line 0
    assert!(!app.fold_state.is_line_folded(0));
}

#[test]
fn test_palette_and_menu_commands_folding() {
    let code = "fn foo() {\n    let a = 1;\n}\n\nfn bar() {\n    let b = 2;\n}\n";
    let mut app = test_app(code);
    let ctx = DummyContext;

    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::document_start()));

    // Palette fold
    crate::palette::execute_command(&mut app, &ctx, "code.fold");
    assert!(app.fold_state.is_line_folded(0));

    // Palette unfold
    crate::palette::execute_command(&mut app, &ctx, "code.unfold");
    assert!(!app.fold_state.is_line_folded(0));

    // Palette fold_all
    crate::palette::execute_command(&mut app, &ctx, "code.fold_all");
    assert!(app.fold_state.is_line_folded(0));
    assert!(app.fold_state.is_line_folded(4));

    // Palette unfold_all
    crate::palette::execute_command(&mut app, &ctx, "code.unfold_all");
    assert!(!app.fold_state.is_line_folded(0));
    assert!(!app.fold_state.is_line_folded(4));

    // Menu fold
    crate::menu::handle_menu_action(&mut app, &ctx, crate::menu::MenuAction::Fold);
    assert!(app.fold_state.is_line_folded(0));

    // Menu unfold
    crate::menu::handle_menu_action(&mut app, &ctx, crate::menu::MenuAction::Unfold);
    assert!(!app.fold_state.is_line_folded(0));

    // Menu fold all
    crate::menu::handle_menu_action(&mut app, &ctx, crate::menu::MenuAction::FoldAll);
    assert!(app.fold_state.is_line_folded(0));
    assert!(app.fold_state.is_line_folded(4));

    // Menu unfold all
    crate::menu::handle_menu_action(&mut app, &ctx, crate::menu::MenuAction::UnfoldAll);
    assert!(!app.fold_state.is_line_folded(0));
    assert!(!app.fold_state.is_line_folded(4));
}
