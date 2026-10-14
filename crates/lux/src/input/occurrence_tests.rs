use super::*;
use lux_core::buffer::Buffer;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, Modifiers, NamedKey};

fn test_app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

fn command(app: &mut App, key: &str, shift: bool) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        ..Default::default()
    };
    app.handle_key(&KeyEvent {
        key: Key::Character(key.into()),
        text: Some(key.into()),
        state: ElementState::Pressed,
        repeat: false,
    });
    app.modifiers = Modifiers::default();
}

fn ctrl_cmd(app: &mut App, key: &str) {
    app.modifiers = Modifiers {
        meta: true,
        control: true,
        ..Default::default()
    };
    app.handle_key(&KeyEvent {
        key: Key::Character(key.into()),
        text: Some(key.into()),
        state: ElementState::Pressed,
        repeat: false,
    });
    app.modifiers = Modifiers::default();
}

fn alt_f3(app: &mut App) {
    app.modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    app.handle_key(&KeyEvent {
        key: Key::Named(NamedKey::F(3)),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
    });
    app.modifiers = Modifiers::default();
}

fn type_char(app: &mut App, ch: &str) {
    app.handle_key(&KeyEvent {
        key: Key::Character(ch.to_string()),
        text: Some(ch.to_string()),
        state: ElementState::Pressed,
        repeat: false,
    });
}

#[test]
fn cmd_d_expands_to_word_then_adds_subsequent_occurrences() {
    let mut a = test_app("foo bar foo baz foo");
    // Place cursor at index 1 (inside first "foo")
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 1)));

    // First Cmd+D: expands collapsed cursor to "foo"
    command(&mut a, "d", false);
    assert_eq!(a.cursors.count(), 1);
    let first = a.cursors.primary();
    assert_eq!(a.buffer.text_range(first.char_range()), "foo");
    assert_eq!(first.char_range(), 0..3);

    // Second Cmd+D: adds second "foo" (index 8..11)
    command(&mut a, "d", false);
    assert_eq!(a.cursors.count(), 2);
    let cursors = a.cursors.cursors();
    assert_eq!(cursors[0].char_range(), 0..3);
    assert_eq!(cursors[1].char_range(), 8..11);

    // Third Cmd+D: adds third "foo" (index 16..19)
    command(&mut a, "d", false);
    assert_eq!(a.cursors.count(), 3);
    assert_eq!(a.cursors.cursors()[2].char_range(), 16..19);
}

#[test]
fn cmd_k_cmd_d_skips_current_occurrence() {
    let mut a = test_app("foo bar foo baz foo");
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 1)));

    // Select first "foo"
    command(&mut a, "d", false);
    assert_eq!(a.cursors.count(), 1);

    // Select second "foo" (now 2 cursors: 0..3 and 8..11)
    command(&mut a, "d", false);
    assert_eq!(a.cursors.count(), 2);
    assert_eq!(a.cursors.cursors()[1].char_range(), 8..11);

    // Now chord Cmd+K followed by Cmd+D: skips second "foo" and advances to third "foo"
    command(&mut a, "k", false);
    assert!(a.ui.pending_chord.is_some());

    command(&mut a, "d", false);
    assert!(a.ui.pending_chord.is_none());
    assert_eq!(a.cursors.count(), 2);

    let cursors = a.cursors.cursors();
    // First cursor is still at 0..3, second cursor skipped 8..11 and is now at 16..19
    assert_eq!(cursors[0].char_range(), 0..3);
    assert_eq!(cursors[1].char_range(), 16..19);
}

#[test]
fn escape_cancels_pending_chord() {
    let mut a = test_app("hello");
    command(&mut a, "k", false);
    assert!(a.ui.pending_chord.is_some());

    a.handle_key(&KeyEvent {
        key: Key::Named(NamedKey::Escape),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
    });
    assert!(a.ui.pending_chord.is_none());
    assert!(a.ui.announcement.is_none());
}

#[test]
fn select_all_occurrences_selects_all_instances() {
    let mut a = test_app("apple banana apple orange apple");
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 2)));

    if cfg!(target_os = "macos") {
        ctrl_cmd(&mut a, "g");
    } else {
        alt_f3(&mut a);
    }

    assert_eq!(a.cursors.count(), 3);
    for s in a.cursors.cursors() {
        assert_eq!(a.buffer.text_range(s.char_range()), "apple");
    }
}

#[test]
fn multi_cursor_typing_and_undo_in_single_step() {
    let mut a = test_app("foo bar foo");
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 0)));

    command(&mut a, "d", false);
    command(&mut a, "d", false);
    assert_eq!(a.cursors.count(), 2);

    type_char(&mut a, "x");
    assert_eq!(a.buffer.text(), "x bar x");

    command(&mut a, "z", false);
    assert_eq!(a.buffer.text(), "foo bar foo");
    assert_eq!(a.cursors.count(), 2);
}

#[test]
fn split_selection_into_lines_creates_cursor_per_line() {
    let mut a = test_app("first line\nsecond line\nthird line");
    // Select all lines from start to end
    a.cursors = MultiCursor::new(Selection::new(
        Position::document_start(),
        Position::document_end(&a.buffer),
    ));

    // Cmd+Shift+L
    command(&mut a, "l", true);
    assert_eq!(a.cursors.count(), 3);
    let cursors = a.cursors.cursors();
    assert_eq!(a.buffer.text_range(cursors[0].char_range()), "first line");
    assert_eq!(a.buffer.text_range(cursors[1].char_range()), "second line");
    assert_eq!(a.buffer.text_range(cursors[2].char_range()), "third line");
}

#[test]
fn add_cursor_above_and_below_operations() {
    let mut a = test_app("line 0\nline 1\nline 2");
    // Place single cursor at line 1, col 2
    let pos1 = lux_core::selection_ops::position_at_column(&a.buffer, 1, 2);
    a.cursors = MultiCursor::new(Selection::cursor(pos1));
    assert_eq!(a.cursors.count(), 1);

    // Add cursor above -> adds line 0, col 2
    // MultiCursor sorts in document order: line 0 at index 0, line 1 at index 1
    a.add_cursor_above();
    assert_eq!(a.cursors.count(), 2);
    assert_eq!(a.cursors.cursors()[0].head.line, 0);
    assert_eq!(a.cursors.cursors()[0].head.column, 2);
    assert_eq!(a.cursors.cursors()[1].head.line, 1);
    assert_eq!(a.cursors.cursors()[1].head.column, 2);

    // Add cursor below -> adds line 2, col 2
    a.add_cursor_below();
    assert_eq!(a.cursors.count(), 3);
    assert_eq!(a.cursors.cursors()[0].head.line, 0);
    assert_eq!(a.cursors.cursors()[1].head.line, 1);
    assert_eq!(a.cursors.cursors()[2].head.line, 2);
    assert_eq!(a.cursors.cursors()[2].head.column, 2);
}

#[test]
fn option_mouse_drag_creates_box_selection() {
    let mut a = test_app("1234\n5678\n9012");
    let anchor = lux_core::selection_ops::position_at_column(&a.buffer, 0, 1);
    let target = lux_core::selection_ops::position_at_column(&a.buffer, 2, 3);
    a.mouse.selection_anchor = Some(anchor);
    a.modifiers.alt = true;

    // Simulate mouse position at target
    // We can directly call box_select or update with position
    a.cursors = lux_core::selection_ops::box_select(&a.buffer, anchor, target);
    assert_eq!(a.cursors.count(), 3);
    let cursors = a.cursors.cursors();
    assert_eq!(a.buffer.text_range(cursors[0].char_range()), "23");
    assert_eq!(a.buffer.text_range(cursors[1].char_range()), "67");
    assert_eq!(a.buffer.text_range(cursors[2].char_range()), "01");
}

#[test]
fn parallel_multi_cursor_paste_distributes_lines_to_cursors() {
    let mut a = test_app("line1\nline2\nline3");
    // Place cursors at end of each line
    let c1 = Selection::cursor(lux_core::selection_ops::position_at_column(&a.buffer, 0, 5));
    let c2 = Selection::cursor(lux_core::selection_ops::position_at_column(&a.buffer, 1, 5));
    let c3 = Selection::cursor(lux_core::selection_ops::position_at_column(&a.buffer, 2, 5));
    a.cursors = MultiCursor::from_cursors(vec![c1, c2, c3]);
    assert_eq!(a.cursors.count(), 3);

    // Paste 3 lines -> distributed 1 line per cursor
    a.insert_at_cursor(":A\n:B\n:C");
    assert_eq!(a.buffer.text(), "line1:A\nline2:B\nline3:C");
}

fn cmd_shift_space(app: &mut App) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift: true,
        ..Default::default()
    };
    app.handle_key(&KeyEvent {
        key: Key::Named(NamedKey::Space),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
    });
    app.modifiers = Modifiers::default();
}

#[test]
fn cmd_l_expands_to_line_and_extends_sequentially() {
    let mut a = test_app("line one\nline two\nline three\n");
    // Place cursor in middle of first line
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 3)));

    // First Cmd+L: selects line one with newline (0..9)
    command(&mut a, "l", false);
    assert_eq!(a.cursors.count(), 1);
    assert_eq!(a.cursors.primary().char_range(), 0..9);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "line one\n"
    );

    // Second Cmd+L: extends to line two (0..18)
    command(&mut a, "l", false);
    assert_eq!(a.cursors.primary().char_range(), 0..18);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "line one\nline two\n"
    );

    // Third Cmd+L: extends to line three (0..29)
    command(&mut a, "l", false);
    assert_eq!(a.cursors.primary().char_range(), 0..29);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "line one\nline two\nline three\n"
    );
}

#[test]
fn cmd_shift_space_expands_to_scope_quotes_and_brackets() {
    let mut a = test_app("foo(\"hello world\")");
    // Cursor at 'w' (char 11)
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 11)));

    // 1st Cmd+Shift+Space: selects inside quotes ("hello world")
    cmd_shift_space(&mut a);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "hello world"
    );

    // 2nd Cmd+Shift+Space: selects including quotes (\"hello world\")
    cmd_shift_space(&mut a);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "\"hello world\""
    );

    // 3rd Cmd+Shift+Space: selects outer parentheses
    cmd_shift_space(&mut a);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "(\"hello world\")"
    );
}

#[test]
fn cmd_shift_a_expands_to_tag() {
    let mut a = test_app("<button>Click me</button>");
    // Cursor at 'm' in "Click me" (char 14)
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 14)));

    // 1st Cmd+Shift+A: selects interior "Click me"
    command(&mut a, "a", true);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "Click me"
    );

    // 2nd Cmd+Shift+A: selects complete button tag
    command(&mut a, "a", true);
    assert_eq!(
        a.buffer.text_range(a.cursors.primary().char_range()),
        "<button>Click me</button>"
    );
}

#[test]
fn swap_line_up_and_down_moves_lines_in_buffer() {
    let mut a = test_app("line 0\nline 1\nline 2\n");
    // Place cursor on line 1
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 8)));

    // Swap line up
    a.apply_edit_operation(a.cursors.clone(), lux_core::edit_ops::move_line_up);
    assert_eq!(a.buffer.text(), "line 1\nline 0\nline 2\n");

    // Swap line down (back)
    a.apply_edit_operation(a.cursors.clone(), lux_core::edit_ops::move_line_down);
    assert_eq!(a.buffer.text(), "line 0\nline 1\nline 2\n");
}

#[test]
fn duplicate_line_and_selection_via_cmd_shift_d() {
    let mut a = test_app("line 0\nline 1\n");
    // Empty cursor on line 0 -> duplicates line
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 2)));
    command(&mut a, "d", true);
    assert_eq!(a.buffer.text(), "line 0\nline 0\nline 1\n");

    // Non-empty selection -> duplicates selection contiguously
    let mut b = test_app("hello world");
    b.cursors = MultiCursor::new(Selection::new(
        Position::from_char(&b.buffer, 0),
        Position::from_char(&b.buffer, 5),
    )); // selects "hello"
    command(&mut b, "d", true);
    assert_eq!(b.buffer.text(), "hellohello world");
}

#[test]
fn join_lines_shortcut_merges_lines() {
    let mut a = test_app("first\n    second\n");
    a.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a.buffer, 2)));

    // Cmd+J (or Ctrl+J on Win/Linux)
    command(&mut a, "j", false);
    assert_eq!(a.buffer.text(), "first second\n");
}
