use super::*;
use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey};

fn test_app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

fn type_char(app: &mut App, ch: char) {
    let s = ch.to_string();
    app.handle_key(&KeyEvent {
        key: Key::Character(s.clone()),
        text: Some(s),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });
}

fn press_backspace(app: &mut App) {
    app.handle_key(&KeyEvent {
        key: Key::Named(NamedKey::Backspace),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });
}

#[test]
fn test_auto_pair_open_delimiter() {
    let mut app = test_app("");

    // Type '(' -> buffer becomes "()" with cursor in between
    type_char(&mut app, '(');
    assert_eq!(app.buffer.text(), "()");
    assert_eq!(app.cursors.primary().head.char, 1);

    // Type '[' -> buffer becomes "([])" with cursor in between
    type_char(&mut app, '[');
    assert_eq!(app.buffer.text(), "([])");
    assert_eq!(app.cursors.primary().head.char, 2);

    // Type '{' -> buffer becomes "([{}])" with cursor in between
    type_char(&mut app, '{');
    assert_eq!(app.buffer.text(), "([{}])");
    assert_eq!(app.cursors.primary().head.char, 3);

    // Type '"' -> buffer becomes "([{\"\"}])"
    type_char(&mut app, '"');
    assert_eq!(app.buffer.text(), "([{\"\"}])");
    assert_eq!(app.cursors.primary().head.char, 4);
}

#[test]
fn test_smart_step_over() {
    let mut app = test_app("");

    type_char(&mut app, '(');
    assert_eq!(app.buffer.text(), "()");
    assert_eq!(app.cursors.primary().head.char, 1);

    // Typing closing ')' steps over rather than duplicating
    type_char(&mut app, ')');
    assert_eq!(app.buffer.text(), "()");
    assert_eq!(app.cursors.primary().head.char, 2);

    // Smart step-over on quotes
    type_char(&mut app, '"');
    assert_eq!(app.buffer.text(), "()\"\"");
    assert_eq!(app.cursors.primary().head.char, 3);

    type_char(&mut app, '"');
    assert_eq!(app.buffer.text(), "()\"\"");
    assert_eq!(app.cursors.primary().head.char, 4);
}

#[test]
fn test_wrap_selection_with_delimiters() {
    let mut app = test_app("hello world");

    // Select "world" (char 6..11)
    let start = Position::from_char(&app.buffer, 6);
    let end = Position::from_char(&app.buffer, 11);
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::new(start, end));

    // Type '(' -> wraps selection
    type_char(&mut app, '(');
    assert_eq!(app.buffer.text(), "hello (world)");

    // Select "world" again (now 7..12)
    let start = Position::from_char(&app.buffer, 7);
    let end = Position::from_char(&app.buffer, 12);
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::new(start, end));

    // Type '"' -> wraps selection with quotes
    type_char(&mut app, '"');
    assert_eq!(app.buffer.text(), "hello (\"world\")");
}

#[test]
fn test_smart_backspace() {
    let mut app = test_app("");

    type_char(&mut app, '(');
    assert_eq!(app.buffer.text(), "()");
    assert_eq!(app.cursors.primary().head.char, 1);

    // Backspace between empty pair deletes both delimiters
    press_backspace(&mut app);
    assert_eq!(app.buffer.text(), "");
    assert_eq!(app.cursors.primary().head.char, 0);

    // Test with curly braces
    type_char(&mut app, '{');
    assert_eq!(app.buffer.text(), "{}");
    press_backspace(&mut app);
    assert_eq!(app.buffer.text(), "");

    // Test with quotes
    type_char(&mut app, '"');
    assert_eq!(app.buffer.text(), "\"\"");
    press_backspace(&mut app);
    assert_eq!(app.buffer.text(), "");

    // Non-empty pair backspaces normally
    type_char(&mut app, '(');
    type_char(&mut app, 'x');
    assert_eq!(app.buffer.text(), "(x)");
    press_backspace(&mut app);
    assert_eq!(app.buffer.text(), "()");
}

#[test]
fn test_single_quote_apostrophe_guard() {
    let mut app = test_app("don");
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        3,
    )));

    // Typing single quote after alphanumeric does not auto-close
    type_char(&mut app, '\'');
    assert_eq!(app.buffer.text(), "don'");
}

#[test]
fn test_jump_to_matching_bracket() {
    let mut app = test_app("fn test(a: i32) { return; }");

    // Position touching '(' at char 7
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        7,
    )));

    app.jump_to_matching_bracket();
    // Jumps to ')' at char 14
    assert_eq!(app.cursors.primary().head.char, 14);

    app.jump_to_matching_bracket();
    // Jumps back to '(' at char 7
    assert_eq!(app.cursors.primary().head.char, 7);

    // Position touching '{' at char 16
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        16,
    )));

    app.jump_to_matching_bracket();
    // Jumps to '}' at char 26
    assert_eq!(app.cursors.primary().head.char, 26);
}

#[test]
fn test_bracket_matching_highlights_in_frame() {
    let mut app = test_app("(hello)");
    let viewport = app.viewport;
    let cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        0,
    )));

    let frame = app.editor_view.build_frame(
        &app.buffer,
        &cursors,
        &viewport,
        std::time::Duration::from_millis(100),
        None,
    );

    // Frame should contain 2 bracket highlight rects (touching '(' and matching ')')
    assert_eq!(frame.matching_brackets.len(), 2);
}
