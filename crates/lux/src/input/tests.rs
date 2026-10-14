use super::*;
use lux_core::buffer::Buffer;
use lux_platform::{Ime, Modifiers, PlatformError};
use std::cell::RefCell;

#[derive(Default)]
struct TestClipboard(RefCell<String>);

impl Clipboard for TestClipboard {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(self.0.borrow().clone())
    }

    fn write_text(&self, text: &str) -> Result<(), PlatformError> {
        *self.0.borrow_mut() = text.to_owned();
        Ok(())
    }
}

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

fn clipboard_command(app: &mut App, clipboard: &dyn Clipboard, key: &str) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        ..Default::default()
    };
    app.handle_key_with_clipboard(
        &KeyEvent {
            key: Key::Character(key.into()),
            text: Some(key.into()),
            state: ElementState::Pressed,
            repeat: false,
        },
        Some(clipboard),
    );
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

#[test]
fn clipboard_copy_cut_and_paste_are_transactional_and_unicode_safe() {
    let clipboard = TestClipboard::default();
    let mut app = app("cafe\u{301} and tea");
    app.cursors = MultiCursor::new(Selection::new(
        Position::from_char(&app.buffer, 0),
        Position::from_char(&app.buffer, 5),
    ));

    clipboard_command(&mut app, &clipboard, "c");
    assert_eq!(&*clipboard.0.borrow(), "cafe\u{301}");
    assert_eq!(app.buffer.text(), "cafe\u{301} and tea");

    clipboard_command(&mut app, &clipboard, "x");
    assert_eq!(app.buffer.text(), " and tea");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "cafe\u{301} and tea");

    app.cursors = MultiCursor::new(Selection::cursor(Position::document_end(&app.buffer)));
    clipboard_command(&mut app, &clipboard, "v");
    assert_eq!(app.buffer.text(), "cafe\u{301} and teacafe\u{301}");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "cafe\u{301} and tea");
}

#[test]
fn altgr_text_is_not_swallowed_as_a_control_shortcut() {
    if cfg!(target_os = "macos") {
        return;
    }
    let mut app = app("");
    app.modifiers.control = true;
    app.modifiers.alt = true;
    text(&mut app, "€");
    assert_eq!(app.buffer.text(), "€");
}

#[test]
fn mouse_click_and_drag_create_a_directional_unicode_safe_selection() {
    let mut app = app("abc\n猫犬");
    app.mouse.position = dpi::PhysicalPosition::new(
        10_000.0,
        app.tab_bar_height() as f64 + LINE_HEIGHT as f64 + 1.0,
    );
    app.begin_mouse_selection();
    assert_eq!(
        app.cursors.primary().head,
        Position::document_end(&app.buffer)
    );

    app.mouse.position = dpi::PhysicalPosition::new(0.0, app.tab_bar_height() as f64);
    app.update_mouse_selection();
    let selection = app.cursors.primary();
    assert_eq!(selection.anchor, Position::document_end(&app.buffer));
    assert_eq!(selection.head, Position::document_start());
    assert_eq!(app.buffer.text_range(selection.char_range()), "abc\n猫犬");
}

#[test]
fn moving_across_a_long_line_reveals_the_primary_caret_horizontally() {
    let mut app = app(&"x".repeat(500));
    app.viewport.width = 120.0;
    named(&mut app, NamedKey::End);
    assert!(app.viewport.scroll_x > 0.0);
    let at_end = app.viewport.scroll_x;
    named(&mut app, NamedKey::Home);
    assert!(app.viewport.scroll_x < at_end);
}

#[test]
fn advanced_editing_shortcuts_use_core_operations_and_single_step_undo() {
    let mut app = app("one two\nthree");
    named(&mut app, NamedKey::End);
    if cfg!(target_os = "macos") {
        app.modifiers.alt = true;
    } else {
        app.modifiers.control = true;
    }
    named(&mut app, NamedKey::Backspace);
    app.modifiers = Modifiers::default();
    assert_eq!(app.buffer.text(), "one \nthree");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "one two\nthree");

    command(&mut app, "d", true);
    assert_eq!(app.buffer.text(), "one two\none two\nthree");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "one two\nthree");

    command(&mut app, "j", false);
    assert_eq!(app.buffer.text(), "one two three");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "one two\nthree");
}

#[test]
fn selected_lines_indent_and_dedent_through_tab() {
    let mut app = app("    one\n    two");
    app.cursors = MultiCursor::new(Selection::new(
        Position::document_start(),
        Position::document_end(&app.buffer),
    ));
    app.modifiers.shift = true;
    named(&mut app, NamedKey::Tab);
    app.modifiers = Modifiers::default();
    assert_eq!(app.buffer.text(), "one\ntwo");
    named(&mut app, NamedKey::Tab);
    assert_eq!(app.buffer.text(), "    one\n    two");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "one\ntwo");
}

#[test]
fn comment_shortcut_uses_the_open_documents_language_and_undoes_cleanly() {
    let mut app = app("fn one() {}\nfn two() {}");
    app.documents.path = Some("sample.rs".into());
    app.cursors = MultiCursor::new(Selection::new(
        Position::document_start(),
        Position::document_end(&app.buffer),
    ));
    command(&mut app, "/", false);
    assert_eq!(app.buffer.text(), "// fn one() {}\n// fn two() {}");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "fn one() {}\nfn two() {}");
}

#[test]
fn accessibility_selection_moves_the_real_unicode_caret_range() {
    use accesskit::{NodeId, TextPosition, TextSelection};

    let mut app = app("éx\n猫犬");
    let selection = app
        .editor_view
        .selection_from_accessibility(
            &app.buffer,
            TextSelection {
                anchor: TextPosition {
                    node: NodeId(1),
                    character_index: 1,
                },
                focus: TextPosition {
                    node: NodeId(2),
                    character_index: 1,
                },
            },
        )
        .unwrap();
    assert_eq!(selection.anchor.byte, "é".len());
    assert_eq!(selection.head.byte, "éx\n猫".len());
}

#[test]
fn snippet_expansion_and_tabstop_navigation() {
    let mut app = app("");
    app.expand_snippet("fn ${1:name}() -> ${2:Result<(), ()>} {\n    $0\n}");
    assert!(app.snippet_session.is_some());
    // Initial selection should be at placeholder 1: "name"
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "name"
    );

    // Type replacement for placeholder 1
    text(&mut app, "my_func");
    assert!(app.buffer.text().starts_with("fn my_func()"));

    // Tab to next placeholder: "Result<(), ()>"
    named(&mut app, NamedKey::Tab);
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "Result<(), ()>"
    );

    // Shift-Tab back to previous placeholder
    app.modifiers.shift = true;
    named(&mut app, NamedKey::Tab);
    app.modifiers.shift = false;
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "my_func"
    );

    // Tab forward: to $2, to $0, then past $0 to exit session
    named(&mut app, NamedKey::Tab);
    named(&mut app, NamedKey::Tab);
    named(&mut app, NamedKey::Tab);
    assert!(app.snippet_session.is_none());

    // Undo should cleanly revert
    command(&mut app, "z", false);
}

#[test]
fn keymap_dispatch_executes_bound_commands() {
    let mut app = app("first line\n");
    // Using default Sublime keymap: duplicate line (Cmd/Ctrl + Shift + D)
    command(&mut app, "d", true);
    assert_eq!(app.buffer.text(), "first line\nfirst line\n");

    // Select all (Cmd/Ctrl + A)
    command(&mut app, "a", false);
    assert_eq!(
        app.cursors.primary().char_range(),
        0..app.buffer.len_chars()
    );
}

#[test]
fn syntax_synchronization_on_open_and_edit() {
    let mut app = app("fn test() { let x = 42; }");
    app.documents.path = Some("test.rs".into());
    app.update_syntax_on_open();
    assert!(app.syntax.is_some());

    // Edit the buffer and check syntax sync
    text(&mut app, "// comment\n");
    assert!(app.syntax.is_some());
    if let Some(syntax) = &app.syntax {
        assert!(syntax.source.contains("// comment\n"));
    }
}

#[test]
fn option_click_adds_secondary_cursor() {
    let mut app = app("hello world\nsecond line\n");
    assert_eq!(app.cursors.count(), 1);

    app.modifiers.alt = true;
    app.mouse.position = dpi::PhysicalPosition::new(
        10_000.0,
        app.tab_bar_height() as f64 + LINE_HEIGHT as f64 + 1.0,
    );
    app.begin_mouse_selection();
    assert_eq!(app.cursors.count(), 2);
}

#[test]
fn tab_inserts_spaces_to_next_tabstop_or_tabs() {
    let mut a = app("");
    a.buffer.set_indent(lux_core::buffer::Indent {
        style: lux_core::buffer::IndentStyle::Spaces,
        width: 4,
    });
    named(&mut a, NamedKey::Tab);
    assert_eq!(a.buffer.text(), "    ");
    assert_eq!(a.cursors.primary().head.column, 4);

    text(&mut a, "ab");
    named(&mut a, NamedKey::Tab);
    assert_eq!(a.buffer.text(), "    ab  ");
    assert_eq!(a.cursors.primary().head.column, 8);

    let mut a_tabs = app("");
    a_tabs.buffer.set_indent(lux_core::buffer::Indent {
        style: lux_core::buffer::IndentStyle::Tabs,
        width: 1,
    });
    named(&mut a_tabs, NamedKey::Tab);
    assert_eq!(a_tabs.buffer.text(), "\t");
}

#[test]
fn enter_auto_indents_and_splits_braces() {
    let mut a1 = app("    let x = 1;");
    a1.cursors = MultiCursor::new(Selection::cursor(Position::document_end(&a1.buffer)));
    named(&mut a1, NamedKey::Enter);
    assert_eq!(a1.buffer.text(), "    let x = 1;\n    ");
    assert_eq!(a1.cursors.primary().head.line, 1);
    assert_eq!(a1.cursors.primary().head.column, 4);

    let mut a_block = app("fn main() {");
    a_block.buffer.set_indent(lux_core::buffer::Indent {
        style: lux_core::buffer::IndentStyle::Spaces,
        width: 4,
    });
    a_block.cursors = MultiCursor::new(Selection::cursor(Position::document_end(&a_block.buffer)));
    named(&mut a_block, NamedKey::Enter);
    assert_eq!(a_block.buffer.text(), "fn main() {\n    ");
    assert_eq!(a_block.cursors.primary().head.line, 1);
    assert_eq!(a_block.cursors.primary().head.column, 4);

    let mut a_pair = app("fn main() {}");
    a_pair.buffer.set_indent(lux_core::buffer::Indent {
        style: lux_core::buffer::IndentStyle::Spaces,
        width: 4,
    });
    a_pair.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&a_pair.buffer, 11)));
    named(&mut a_pair, NamedKey::Enter);
    assert_eq!(a_pair.buffer.text(), "fn main() {\n    \n}");
    assert_eq!(a_pair.cursors.primary().head.line, 1);
    assert_eq!(a_pair.cursors.primary().head.column, 4);
}

#[test]
fn toggle_word_wrap_shortcut_and_view_update() {
    let mut a = app("hello world");
    assert!(!a.editor_view.metrics().word_wrap);

    // Option+Z toggles word wrap
    let event = lux_platform::event::KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("z".into()),
        text: None,
        repeat: false,
    };
    a.modifiers.alt = true;
    a.handle_key_with_clipboard(&event, None);
    assert!(a.editor_view.metrics().word_wrap);

    // Pressing Option+Z again toggles it off
    a.handle_key_with_clipboard(&event, None);
    assert!(!a.editor_view.metrics().word_wrap);
}

#[test]
fn split_editor_shortcuts_and_cycling() {
    let mut a = app("hello world");
    assert!(a.ui.split.secondary.is_none());

    let event_split = lux_platform::event::KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("2".into()),
        text: None,
        repeat: false,
    };
    if cfg!(target_os = "macos") {
        a.modifiers.meta = true;
    } else {
        a.modifiers.control = true;
    }
    a.modifiers.alt = true;
    a.handle_key_with_clipboard(&event_split, None);
    assert!(a.ui.split.secondary.is_some());
    assert_eq!(
        a.ui.split.direction,
        crate::split::SplitDirection::Horizontal
    );
    assert_eq!(a.ui.split.active_pane, 0);

    let event_f6 = lux_platform::event::KeyEvent {
        state: ElementState::Pressed,
        key: Key::Named(NamedKey::F(6)),
        text: None,
        repeat: false,
    };
    a.modifiers.meta = false;
    a.modifiers.control = false;
    a.modifiers.alt = false;
    a.handle_key_with_clipboard(&event_f6, None);
    assert_eq!(a.ui.split.active_pane, 1);

    let event_close = lux_platform::event::KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("w".into()),
        text: None,
        repeat: false,
    };
    if cfg!(target_os = "macos") {
        a.modifiers.meta = true;
    } else {
        a.modifiers.control = true;
    }
    a.modifiers.alt = true;
    a.handle_key_with_clipboard(&event_close, None);
    assert!(a.ui.split.secondary.is_none());
}
