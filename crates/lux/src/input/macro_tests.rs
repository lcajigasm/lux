//! Comprehensive integration tests for ST19: Keyboard Macro Recording & Playback.

use super::*;
use crate::macros::command::{MacroCommand, MoveBy, MoveTo};
use crate::macros::macro_def::Macro;
use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::{Key, NamedKey};

fn test_app(initial_text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(initial_text.as_bytes()).unwrap();
    app.cursors = MultiCursor::new(Selection::cursor(
        lux_core::position::Position::document_start(),
    ));
    app
}

fn press(app: &mut App, key: Key, text: Option<&str>) {
    app.handle_key(&KeyEvent {
        key,
        text: text.map(str::to_owned),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });
}

fn named(app: &mut App, key: NamedKey) {
    press(app, Key::Named(key), None);
}

fn text(app: &mut App, s: &str) {
    press(app, Key::Character(s.into()), Some(s));
}

fn ctrl_q(app: &mut App) {
    app.modifiers.control = true;
    press(app, Key::Character("q".to_string()), Some("q"));
    app.modifiers.control = false;
}

fn playback_macro_key(app: &mut App) {
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
        app.modifiers.shift = true;
        press(app, Key::Character("q".to_string()), Some("q"));
        app.modifiers.meta = false;
        app.modifiers.shift = false;
    } else {
        app.modifiers.control = true;
        app.modifiers.shift = true;
        press(app, Key::Character("q".to_string()), Some("q"));
        app.modifiers.control = false;
        app.modifiers.shift = false;
    }
}

#[test]
fn test_record_macro_with_typing_and_movement() {
    let mut app = test_app("hello world\n");
    assert!(!app.ui.macros.is_recording);

    // 1. Press Ctrl+Q to start recording
    ctrl_q(&mut app);
    assert!(app.ui.macros.is_recording);
    assert_eq!(app.documents.message.as_deref(), Some("● Recording Macro"));

    // 2. Type text "// "
    text(&mut app, "/");
    text(&mut app, "/");
    text(&mut app, " ");

    // 3. Move cursor down and to line start
    named(&mut app, NamedKey::ArrowDown);
    named(&mut app, NamedKey::Home);

    // 4. Press Ctrl+Q to stop recording
    ctrl_q(&mut app);
    assert!(!app.ui.macros.is_recording);
    assert!(app.ui.macros.last_macro.is_some());

    let recorded = app.ui.macros.last_macro.as_ref().unwrap();
    assert_eq!(recorded.commands.len(), 3);
    assert_eq!(
        recorded.commands[0],
        MacroCommand::InsertText {
            text: "// ".to_string(),
        }
    );
    assert_eq!(
        recorded.commands[1],
        MacroCommand::Move {
            by: MoveBy::Lines,
            forward: true,
            extend: false,
        }
    );
    assert_eq!(
        recorded.commands[2],
        MacroCommand::MoveTo {
            to: MoveTo::Bol,
            extend: false,
        }
    );
}

#[test]
fn test_playback_macro_replicates_exact_transformation() {
    let mut app = test_app("first line\nsecond line\nthird line\n");

    // Start recording at (0, 0)
    ctrl_q(&mut app);
    text(&mut app, "#");
    text(&mut app, " ");
    named(&mut app, NamedKey::ArrowDown);
    named(&mut app, NamedKey::Home);
    ctrl_q(&mut app);

    assert_eq!(app.buffer.text(), "# first line\nsecond line\nthird line\n");
    // Cursor is now on second line at column 0
    assert_eq!(app.cursors.primary().head.line, 1);
    assert_eq!(app.cursors.primary().head.column, 0);

    // Playback macro on second line
    playback_macro_key(&mut app);
    assert_eq!(
        app.buffer.text(),
        "# first line\n# second line\nthird line\n"
    );
    // Cursor advanced to third line at column 0
    assert_eq!(app.cursors.primary().head.line, 2);
    assert_eq!(app.cursors.primary().head.column, 0);

    // Playback macro on third line
    playback_macro_key(&mut app);
    assert_eq!(
        app.buffer.text(),
        "# first line\n# second line\n# third line\n"
    );
}

#[test]
fn test_playback_macro_with_multi_cursor() {
    let mut app = test_app("alpha\nbeta\ngamma\n");

    // Set 3 cursors at line starts
    let c1 = Selection::cursor(Position::from_char(&app.buffer, 0));
    let c2 = Selection::cursor(Position::from_char(&app.buffer, 6));
    let c3 = Selection::cursor(Position::from_char(&app.buffer, 11));
    app.cursors = MultiCursor::from_cursors(vec![c1, c2, c3]);
    assert_eq!(app.cursors.count(), 3);

    // Define a macro that prepends "item: "
    let m = Macro::from_commands(vec![MacroCommand::InsertText {
        text: "item: ".to_string(),
    }]);
    app.ui.macros.last_macro = Some(m);

    // Playback macro
    playback_macro_key(&mut app);

    assert_eq!(app.buffer.text(), "item: alpha\nitem: beta\nitem: gamma\n");
}

#[test]
fn test_record_and_playback_with_editor_commands() {
    let mut app = test_app("hello\n");

    // Record macro that duplicates line
    ctrl_q(&mut app);
    crate::input::dispatch::execute_keymap_command(&mut app, "duplicate_line", None);
    ctrl_q(&mut app);

    assert_eq!(app.buffer.text(), "hello\nhello\n");

    // Playback duplicates again
    playback_macro_key(&mut app);
    assert_eq!(app.buffer.text(), "hello\nhello\nhello\n");
}

#[test]
fn test_sublime_macro_file_save_and_load() {
    let temp = tempfile::tempdir().unwrap();
    let mut app = test_app("sample\n");

    // Record a macro
    ctrl_q(&mut app);
    text(&mut app, "prefix_");
    ctrl_q(&mut app);

    // Save macro as "test_macro"
    let saved_path = app
        .ui
        .macros
        .save_macro_as("test_macro", Some(temp.path()))
        .unwrap();

    assert!(saved_path.exists());
    assert_eq!(saved_path.file_name().unwrap(), "test_macro.sublime-macro");

    // Read and verify JSON structure
    let json_content = std::fs::read_to_string(&saved_path).unwrap();
    assert!(json_content.contains("\"command\": \"insert\""));
    assert!(json_content.contains("\"characters\": \"prefix_\""));

    // Load macro back into another App
    let mut app2 = test_app("world\n");
    app2.ui.macros.load_macros_from_dir(temp.path());

    assert_eq!(app2.ui.macros.saved_macros.len(), 1);
    let loaded = app2.ui.macros.saved_macros[0].clone();
    assert_eq!(loaded.name.as_deref(), Some("test_macro"));
    assert_eq!(loaded.commands.len(), 1);

    // Play loaded macro on app2
    crate::macros::playback::execute_macro(&mut app2, &loaded);
    assert_eq!(app2.buffer.text(), "prefix_world\n");
}

#[test]
fn test_macro_backspace_and_word_deletion() {
    let mut app = test_app("foo bar baz\n");
    // Move to end of first line
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&app.buffer, 11)));

    ctrl_q(&mut app);
    // Delete word backward
    crate::macros::dispatch::record_word_delete(&mut app, false);
    app.apply_edit_operation(
        app.cursors.clone(),
        lux_core::edit_ops::delete_word_backward,
    );
    ctrl_q(&mut app);

    assert_eq!(app.buffer.text(), "foo bar \n");

    // Playback deletes another word
    playback_macro_key(&mut app);
    assert_eq!(app.buffer.text(), "foo \n");
}
