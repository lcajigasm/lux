//! ST-EDIT-03 through the app's key path: Enter indents, a typed closer or
//! Python clause dedents in the same undo step, Reindent fixes a selection,
//! and Enter on a 20,000-line file parses a window, not the buffer.

use lux_core::buffer::{Indent, IndentStyle};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey};
use lux_syntax::LanguageId;

use crate::app::App;
use crate::input::dispatch::execute_keymap_command;
use crate::input::tests::{app, named};

fn editor(text: &str, language: Option<LanguageId>) -> App {
    let mut app = app(text);
    app.documents.entries[0].language = language;
    app.buffer.set_indent(Indent {
        style: IndentStyle::Spaces,
        width: 4,
    });
    let end = Position::document_end(&app.buffer);
    app.cursors = MultiCursor::new(Selection::cursor(end));
    app
}

fn type_text(app: &mut App, text: &str) {
    for ch in text.chars() {
        app.handle_key(&KeyEvent {
            key: Key::Character(ch.to_string()),
            text: Some(ch.to_string()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        });
    }
}

fn undo(app: &mut App) {
    execute_keymap_command(app, "undo", None, None);
}

fn caret(app: &App) -> (usize, usize) {
    let head = app.cursors.primary().head;
    (head.line, head.column)
}

#[test]
fn enter_after_a_brace_indents_and_a_typed_brace_dedents_to_the_opener() {
    let mut app = editor("fn foo() {", Some(LanguageId::Rust));
    named(&mut app, NamedKey::Enter);
    assert_eq!(app.buffer.text(), "fn foo() {\n    ");
    assert_eq!(caret(&app), (1, 4));

    type_text(&mut app, "}");
    assert_eq!(app.buffer.text(), "fn foo() {\n}");
    assert_eq!(caret(&app), (1, 1));

    undo(&mut app);
    assert_eq!(app.buffer.text(), "fn foo() {\n    ");
    assert_eq!(caret(&app), (1, 4));
}

#[test]
fn a_nested_closer_aligns_with_its_own_opener() {
    let text = "fn f() {\n    if x {\n        y();\n        ";
    let mut app = editor(text, Some(LanguageId::Rust));
    type_text(&mut app, "}");
    assert_eq!(
        app.buffer.text(),
        "fn f() {\n    if x {\n        y();\n    }"
    );
}

#[test]
fn brackets_and_parens_dedent_without_a_syntax() {
    let mut app = editor("let v = [\n    1,\n    ", None);
    type_text(&mut app, "]");
    assert_eq!(app.buffer.text(), "let v = [\n    1,\n]");

    let mut app = editor("    call(\n        a,\n        ", None);
    type_text(&mut app, ")");
    assert_eq!(app.buffer.text(), "    call(\n        a,\n    )");
}

#[test]
fn a_closer_after_text_or_before_its_twin_does_not_dedent() {
    let mut app = editor("fn f() {\n    x", Some(LanguageId::Rust));
    type_text(&mut app, "}");
    assert_eq!(app.buffer.text(), "fn f() {\n    x}");

    let mut app = editor("fn f() {\n    }", Some(LanguageId::Rust));
    let before = Position::from_char(&app.buffer, app.buffer.len_chars() - 1);
    app.cursors = MultiCursor::new(Selection::cursor(before));
    type_text(&mut app, "}");
    assert_eq!(app.buffer.text(), "fn f() {\n    }");
    assert_eq!(caret(&app), (1, 5));
}

#[test]
fn python_clauses_dedent_to_their_header_on_the_colon() {
    let cases = [
        ("if x:\n    a\n    else", "if x:\n    a\nelse:"),
        ("if x:\n    a\n    elif y", "if x:\n    a\nelif y:"),
        (
            "try:\n    a\n    except ValueError",
            "try:\n    a\nexcept ValueError:",
        ),
        (
            "try:\n    a\nexcept:\n    b\n    finally",
            "try:\n    a\nexcept:\n    b\nfinally:",
        ),
        (
            "def f():\n    for i in x:\n        a\n        else",
            "def f():\n    for i in x:\n        a\n    else:",
        ),
    ];
    for (before, after) in cases {
        let mut app = editor(before, Some(LanguageId::Python));
        type_text(&mut app, ":");
        assert_eq!(app.buffer.text(), after, "typing `:` after {before:?}");
        undo(&mut app);
        assert_eq!(app.buffer.text(), before, "undo after {before:?}");
    }
}

#[test]
fn python_clauses_already_aligned_or_outside_python_stay_put() {
    let aligned = "if a:\n    if b:\n        x\n    else";
    let mut app = editor(aligned, Some(LanguageId::Python));
    type_text(&mut app, ":");
    assert_eq!(app.buffer.text(), format!("{aligned}:"));

    let mut app = editor("if x:\n    a\n    else", Some(LanguageId::Rust));
    type_text(&mut app, ":");
    assert_eq!(app.buffer.text(), "if x:\n    a\n    else:");

    let mut app = editor("if x:\n    a\n    elsewhere", Some(LanguageId::Python));
    type_text(&mut app, ":");
    assert_eq!(app.buffer.text(), "if x:\n    a\n    elsewhere:");
}

#[test]
fn reindent_command_fixes_the_selected_lines_in_one_undo_step() {
    let text = "fn f() {\nif x {\n        y();\n  }\n}\n";
    let mut app = editor(text, Some(LanguageId::Rust));
    let end = Position::document_end(&app.buffer);
    app.cursors = MultiCursor::new(Selection::new(Position::from_char(&app.buffer, 0), end));
    execute_keymap_command(&mut app, "reindent", None, None);
    assert_eq!(
        app.buffer.text(),
        "fn f() {\n    if x {\n        y();\n    }\n}\n"
    );
    undo(&mut app);
    assert_eq!(app.buffer.text(), text);
}

#[test]
fn reindent_with_several_carets_fixes_each_line_in_one_undo_step() {
    let text = "fn f() {\nlet a = 1;\nlet b = 2;\n}";
    let mut app = editor(text, Some(LanguageId::Rust));
    let at = |app: &App, char| Selection::cursor(Position::from_char(&app.buffer, char));
    app.cursors = MultiCursor::from_cursors(vec![at(&app, 9), at(&app, 20)]);
    execute_keymap_command(&mut app, "reindent", None, None);
    assert_eq!(
        app.buffer.text(),
        "fn f() {\n    let a = 1;\n    let b = 2;\n}"
    );
    undo(&mut app);
    assert_eq!(app.buffer.text(), text);
}

#[test]
fn reindent_keeps_the_caret_on_its_text() {
    let mut app = editor("fn f() {\nlet x = 1;\n}", Some(LanguageId::Rust));
    let at = Position::from_char(&app.buffer, 13);
    app.cursors = MultiCursor::new(Selection::cursor(at));
    execute_keymap_command(&mut app, "reindent", None, None);
    assert_eq!(app.buffer.text(), "fn f() {\n    let x = 1;\n}");
    assert_eq!(caret(&app), (1, 8));
}

#[test]
fn reindent_without_rules_reports_and_changes_nothing() {
    let mut app = editor("a\n      b\n", None);
    execute_keymap_command(&mut app, "reindent", None, None);
    assert_eq!(app.buffer.text(), "a\n      b\n");
    assert!(
        app.documents
            .message
            .as_deref()
            .unwrap()
            .contains("Reindent")
    );
}

#[test]
fn enter_on_a_twenty_thousand_line_file_parses_only_the_enclosing_item() {
    let source: String = (0..5_000)
        .map(|i| format!("fn f{i}() {{\n    let x = {i};\n    x + 1;\n}}\n"))
        .collect();
    let mut app = editor(&source, Some(LanguageId::Rust));
    assert_eq!(app.buffer.len_lines(), 20_001);
    let line = 10_001;
    let end_of_line = app.buffer.rope().line_to_char(line + 1) - 1;
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        end_of_line,
    )));
    super::PARSED_BYTES.with(|bytes| bytes.set(0));
    named(&mut app, NamedKey::Enter);
    let parsed = super::PARSED_BYTES.with(std::cell::Cell::get);
    assert!(parsed > 0, "Enter must use the tree");
    assert!(
        parsed < 400,
        "Enter parsed {parsed} bytes of a {}-byte buffer",
        source.len()
    );
    assert_eq!(app.buffer.line_content(line + 1).unwrap(), "    ");
    assert_eq!(caret(&app), (line + 1, 4));
}

#[test]
fn enter_indents_after_column_zero_clauses_and_comments() {
    let cases = [
        ("if x:\n    a\nelse:", LanguageId::Python),
        ("if x:\n    a\nelse:\n    b", LanguageId::Python),
        ("def f():\n    a\n# note\n    b", LanguageId::Python),
        ("fn f() {\n    a();\n// note\n    b();", LanguageId::Rust),
    ];
    for (text, lang) in cases {
        let mut app = editor(text, Some(lang));
        named(&mut app, NamedKey::Enter);
        assert_eq!(
            app.buffer.text(),
            format!("{text}\n    "),
            "Enter after {text:?}"
        );
    }
}
