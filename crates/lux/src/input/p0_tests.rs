//! P0 keymap and edit behavior from the definition of done: Sublime chords,
//! comments without a line prefix, tree indent on Enter, and overlay scroll.

use lux_config::{ChordSequence, Platform};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey};
use lux_syntax::LanguageId;

use super::tests::{app, command, named};
use crate::overlay_wheel::scroll_open_overlay;
use lux_platform::ScrollDelta;

fn at(app: &crate::app::App, line: usize, column: usize) -> Selection {
    Selection::cursor(lux_core::selection_ops::position_at_column(
        &app.buffer,
        line,
        column,
    ))
}

#[test]
fn command_t_opens_goto_file_and_leaves_the_buffer() {
    let keymap = lux_config::keymap::presets::sublime();
    let chord: ChordSequence = "cmd+t".parse().unwrap();
    assert_eq!(
        keymap.command_for(&chord, Platform::Mac),
        Some("quick_open")
    );
    assert_eq!(
        keymap.command_for(&chord, Platform::Linux),
        Some("quick_open")
    );
    let mut app = app("alpha\n");
    app.modifiers.meta = true;
    app.handle_key(&KeyEvent {
        key: Key::Character("t".into()),
        text: Some("t".into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });
    assert!(app.ui.goto_anything.is_some());
    assert_eq!(app.buffer.text(), "alpha\n");
}

#[test]
fn sublime_mac_extra_cursors_use_ctrl_shift_arrows() {
    let keymap = lux_config::keymap::presets::sublime();
    let up: ChordSequence = "ctrl+shift+up".parse().unwrap();
    let down: ChordSequence = "ctrl+shift+down".parse().unwrap();
    assert_eq!(
        keymap.command_for(&up, Platform::Mac),
        Some("add_cursor_above")
    );
    assert_eq!(
        keymap.command_for(&down, Platform::Mac),
        Some("add_cursor_below")
    );
    let old: ChordSequence = "cmd+alt+up".parse().unwrap();
    assert_eq!(keymap.command_for(&old, Platform::Mac), None);

    let mut app = app("one\ntwo\nthree\n");
    app.cursors = MultiCursor::new(at(&app, 1, 1));
    #[cfg(target_os = "macos")]
    {
        app.modifiers.control = true;
        app.modifiers.shift = true;
    }
    #[cfg(not(target_os = "macos"))]
    {
        app.modifiers.control = true;
        app.modifiers.alt = true;
    }
    named(&mut app, NamedKey::ArrowDown);
    assert_eq!(app.cursors.count(), 2);
    assert_eq!(app.buffer.text(), "one\ntwo\nthree\n");
}

#[test]
fn html_css_markdown_and_json_comment_from_the_keyboard() {
    fn comment(text: &str, lang: LanguageId) -> String {
        let mut app = app(text);
        app.documents.entries[0].language = Some(lang);
        command(&mut app, "/", false);
        app.buffer.text()
    }
    assert_eq!(
        comment("<p>hi</p>\n", LanguageId::Html),
        "<!--<p>hi</p>-->\n"
    );
    assert_eq!(
        comment("a { color: red; }\n", LanguageId::Css),
        "/*a { color: red; }*/\n"
    );
    assert_eq!(
        comment("# Title\n", LanguageId::Markdown),
        "<!--# Title-->\n"
    );
    assert_eq!(comment("{\"a\": 1}\n", LanguageId::Json), "// {\"a\": 1}\n");
}

#[test]
fn command_slash_comments_every_language_that_has_one() {
    for lang in LanguageId::ALL {
        let mut app = app("alpha\n");
        app.documents.entries[0].language = Some(*lang);
        command(&mut app, "/", false);
        let text = app.buffer.text();
        if matches!(*lang, LanguageId::Diff | LanguageId::Regex) {
            assert_eq!(text, "alpha\n", "{lang:?}");
        } else {
            assert_ne!(text, "alpha\n", "{lang:?}");
            assert!(text.contains("alpha"), "{lang:?}: {text}");
        }
    }
}

#[test]
fn option_command_slash_toggles_a_block_comment() {
    let mut app = app("let x = 1;\n");
    app.documents.entries[0].language = Some(LanguageId::Rust);
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
        app.modifiers.alt = true;
    } else {
        app.modifiers.control = true;
        app.modifiers.shift = true;
    }
    app.handle_key(&KeyEvent {
        key: Key::Character("/".into()),
        text: Some("/".into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });
    assert_eq!(app.buffer.text(), "/*let x = 1;*/\n");
}

#[test]
fn bracket_indent_comes_from_the_keymap_and_can_be_removed() {
    let mut rebound = app("a");
    rebound
        .keymap
        .bindings
        .retain(|binding| binding.command != "indent" && binding.command != "dedent");
    command(&mut rebound, "]", false);
    assert_eq!(rebound.buffer.text(), "a");

    let mut app = app("a");
    command(&mut app, "]", false);
    assert_eq!(app.buffer.text(), "    a");
}

#[test]
fn command_left_and_ctrl_a_e_move_inside_the_line() {
    let buffer = lux_core::buffer::Buffer::from_bytes(b"    alpha\n").unwrap();
    let end = Position::from_char(&buffer, 9);
    let first = lux_core::movement::line_start(&buffer, end);
    assert_eq!(first.column, 4);
    let zero = lux_core::movement::line_start(&buffer, first);
    assert_eq!(zero.column, 0);

    let mut app = app("    alpha\n");
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&app.buffer, 9)));
    app.modifiers.meta = cfg!(target_os = "macos");
    app.modifiers.control = !cfg!(target_os = "macos");
    named(&mut app, NamedKey::ArrowLeft);
    #[cfg(target_os = "macos")]
    assert_eq!(app.cursors.primary().head.column, 4);

    #[cfg(target_os = "macos")]
    {
        app.modifiers = lux_platform::Modifiers {
            control: true,
            ..Default::default()
        };
        app.handle_key(&KeyEvent {
            key: Key::Character("a".into()),
            text: Some("a".into()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        });
        assert_eq!(app.cursors.primary().head.column, 0);
        assert_eq!(app.buffer.text(), "    alpha\n");
        app.handle_key(&KeyEvent {
            key: Key::Character("e".into()),
            text: Some("e".into()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        });
        assert_eq!(app.cursors.primary().head.column, 9);
        assert_eq!(app.buffer.text(), "    alpha\n");
    }
}

#[test]
fn enter_uses_the_tree_and_ignores_a_brace_inside_a_string() {
    let mut rust = app("fn main() {\n    let s = \"{\";");
    rust.documents.entries[0].language = Some(LanguageId::Rust);
    rust.buffer.set_indent(lux_core::buffer::Indent {
        style: lux_core::buffer::IndentStyle::Spaces,
        width: 4,
    });
    let end = Position::document_end(&rust.buffer);
    rust.cursors = MultiCursor::new(Selection::cursor(end));
    named(&mut rust, NamedKey::Enter);
    assert_eq!(rust.buffer.text(), "fn main() {\n    let s = \"{\";\n    ");

    let mut python = app("if True:");
    python.documents.entries[0].language = Some(LanguageId::Python);
    python.buffer.set_indent(lux_core::buffer::Indent {
        style: lux_core::buffer::IndentStyle::Spaces,
        width: 4,
    });
    python.cursors = MultiCursor::new(Selection::cursor(Position::document_end(&python.buffer)));
    named(&mut python, NamedKey::Enter);
    assert_eq!(python.buffer.text(), "if True:\n    ");
}

#[test]
fn multicursor_backspace_deletes_at_every_cursor() {
    let mut app = app("ab\ncd\n");
    app.cursors = MultiCursor::from_cursors(vec![at(&app, 0, 1), at(&app, 1, 1)]);
    named(&mut app, NamedKey::Backspace);
    assert_eq!(app.buffer.text(), "b\nd\n");
}

#[test]
fn goto_wheel_scrolls_the_panel_and_not_the_editor() {
    let mut app = app("alpha\n");
    app.open_goto_anything("");
    app.viewport.scroll_y = 40.0;
    if let Some(goto) = app.ui.goto_anything.as_mut() {
        goto.items = vec![item("one"), item("two"), item("three")];
        goto.selected_index = 0;
    }
    assert!(scroll_open_overlay(
        &mut app,
        ScrollDelta::Lines { x: 0.0, y: -1.0 }
    ));
    assert_eq!(app.ui.goto_anything.as_ref().unwrap().selected_index, 1);
    assert_eq!(app.viewport.scroll_y, 40.0);
    assert_eq!(app.buffer.text(), "alpha\n");
}

fn item(label: &str) -> crate::goto::GotoItem {
    crate::goto::GotoItem {
        label: label.into(),
        detail: String::new(),
        path: label.into(),
        line: None,
        col: None,
        kind: crate::goto::GotoItemKind::File {
            line: None,
            col: None,
        },
    }
}
