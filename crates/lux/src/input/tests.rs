use super::*;
use lux_core::buffer::Buffer;
use lux_platform::{Ime, Modifiers};

fn app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

fn press(app: &mut App, key: Key, text: Option<&str>) {
    app.handle_key(&KeyEvent {
        key,
        text: text.map(str::to_owned),
        state: ElementState::Pressed,
        repeat: false,
    });
}

fn named(app: &mut App, key: NamedKey) {
    press(app, Key::Named(key), None);
}
fn text(app: &mut App, text: &str) {
    press(app, Key::Character(text.into()), Some(text));
}
fn command(app: &mut App, key: &str, shift: bool) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        ..Default::default()
    };
    text(app, key);
    app.modifiers = Modifiers::default();
}

#[test]
fn selection_replacement_restores_text_and_direction_on_undo_redo() {
    let mut app = app("cafe\u{301}!");
    named(&mut app, NamedKey::End);
    app.modifiers.shift = true;
    named(&mut app, NamedKey::ArrowLeft);
    named(&mut app, NamedKey::ArrowLeft);
    let selected = app.cursors.clone();
    assert_eq!(
        app.buffer.text_range(selected.primary().char_range()),
        "e\u{301}!"
    );
    app.modifiers.shift = false;
    text(&mut app, "é?");
    assert_eq!(app.buffer.text(), "café?");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "cafe\u{301}!");
    assert_eq!(app.cursors, selected);
    command(&mut app, "z", true);
    assert_eq!(app.buffer.text(), "café?");
}

#[test]
fn next_occurrence_types_at_every_selection_in_one_undo_step() {
    let mut app = app("cat dog cat");
    command(&mut app, "d", false);
    assert_eq!(app.cursors.count(), 1);
    assert_eq!(app.cursors.primary().char_range(), 0..3);
    command(&mut app, "d", false);
    assert_eq!(app.cursors.count(), 2);
    text(&mut app, "🐈");
    assert_eq!(app.buffer.text(), "🐈 dog 🐈");
    let after = app.cursors.clone();
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "cat dog cat");
    assert_eq!(app.cursors.count(), 2);
    command(&mut app, "z", true);
    assert_eq!(app.buffer.text(), "🐈 dog 🐈");
    assert_eq!(app.cursors, after);
}

#[test]
fn deleting_adjacent_cursors_merges_results_and_undo_restores_primary() {
    let mut app = app("abc");
    app.cursors = MultiCursor::from_cursors(vec![
        Selection::cursor(Position::from_char(&app.buffer, 2)),
        Selection::cursor(Position::from_char(&app.buffer, 1)),
    ]);
    app.reset_goal_columns();
    let original = app.cursors.clone();
    named(&mut app, NamedKey::Backspace);
    assert_eq!(app.buffer.text(), "c");
    assert_eq!(app.cursors.count(), 1);
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "abc");
    assert_eq!(app.cursors, original);
}

#[test]
fn no_op_delete_preserves_redo_and_shortcuts_do_not_insert() {
    let mut app = app("");
    text(&mut app, "hello");
    command(&mut app, "z", false);
    named(&mut app, NamedKey::Backspace);
    command(&mut app, "s", false);
    command(&mut app, "z", true);
    assert_eq!(app.buffer.text(), "hello");
}

#[test]
fn ime_enter_and_backspace_do_not_edit_until_commit() {
    let mut app = app("old");
    command(&mut app, "a", false);
    app.apply_ime(Ime::Preedit {
        text: "に".into(),
        cursor_range: Some((3, 3)),
    });
    named(&mut app, NamedKey::Backspace);
    named(&mut app, NamedKey::Enter);
    text(&mut app, "n");
    assert_eq!(app.buffer.text(), "old");
    app.apply_ime(Ime::Commit("日本".into()));
    assert_eq!(app.buffer.text(), "日本");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "old");
}

#[test]
fn vertical_motion_retains_column_across_short_lines_and_page_uses_viewport() {
    let mut app = app("abcdef\nx\nabcdef\nabcdef\nabcdef");
    named(&mut app, NamedKey::End);
    named(&mut app, NamedKey::ArrowDown);
    assert_eq!(app.cursors.primary().head.column, 1);
    named(&mut app, NamedKey::ArrowDown);
    assert_eq!(app.cursors.primary().head.column, 6);
    app.viewport.height = 2.0 * LINE_HEIGHT;
    named(&mut app, NamedKey::PageDown);
    assert_eq!(app.cursors.primary().head.line, 4);
    assert_eq!(app.cursors.primary().head.column, 6);
}

#[test]
fn named_space_and_crlf_enter_are_undoable_and_release_is_ignored() {
    let mut app = app("a\r\n");
    named(&mut app, NamedKey::Space);
    named(&mut app, NamedKey::Enter);
    assert_eq!(app.buffer.text(), " \r\na\r\n");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), " a\r\n");
    app.handle_key(&KeyEvent {
        key: Key::Character("q".into()),
        text: Some("q".into()),
        state: ElementState::Released,
        repeat: false,
    });
    assert_eq!(app.buffer.text(), " a\r\n");
}
