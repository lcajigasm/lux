//! Comprehensive integration tests for ST18: Sublime Text Snippet Engine.

use super::*;
use lux_core::buffer::Buffer;
use lux_platform::PlatformContext;
use lux_platform::{Key, NamedKey};
use lux_syntax::LanguageId;

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

#[test]
fn test_expand_trigger_fn_on_tab_in_rust_file() {
    let mut app = test_app("");
    app.documents.path = Some(std::path::PathBuf::from("main.rs"));
    app.set_language_mode(Some(LanguageId::Rust));

    // Type trigger word "fn"
    text(&mut app, "f");
    text(&mut app, "n");
    assert_eq!(app.buffer.text(), "fn");

    // Press Tab -> expands to function snippet
    named(&mut app, NamedKey::Tab);
    assert!(app.snippet_session.is_some());
    assert_eq!(app.buffer.text(), "fn name() {\n\t\n}");

    // Initial cursor is selecting default "name"
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "name"
    );

    // Type replacement "calculate" character by character
    for ch in ["c", "a", "l", "c", "u", "l", "a", "t", "e"] {
        text(&mut app, ch);
    }
    assert_eq!(app.buffer.text(), "fn calculate() {\n\t\n}");

    // Tab to next tabstop: $2 (parameters)
    named(&mut app, NamedKey::Tab);
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        ""
    );

    // Type parameters
    text(&mut app, "x");
    text(&mut app, ":");
    text(&mut app, " ");
    text(&mut app, "i");
    text(&mut app, "3");
    text(&mut app, "2");
    assert_eq!(app.buffer.text(), "fn calculate(x: i32) {\n\t\n}");

    // Tab to $0 (body)
    named(&mut app, NamedKey::Tab);
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        ""
    );

    // Tab past $0 exits snippet mode
    named(&mut app, NamedKey::Tab);
    assert!(app.snippet_session.is_none());
}

#[test]
fn test_snippet_mirrors_update_in_real_time_while_typing() {
    let mut app = test_app("");
    app.expand_snippet("let ${1:val} = compute($1) + $1;");
    assert!(app.snippet_session.is_some());
    assert_eq!(app.buffer.text(), "let val = compute(val) + val;");

    // Typing replaces all 3 mirrors simultaneously
    text(&mut app, "a");
    assert_eq!(app.buffer.text(), "let a = compute(a) + a;");

    text(&mut app, "b");
    assert_eq!(app.buffer.text(), "let ab = compute(ab) + ab;");

    text(&mut app, "c");
    assert_eq!(app.buffer.text(), "let abc = compute(abc) + abc;");

    // Backspace deletes 'c' from all mirrors
    named(&mut app, NamedKey::Backspace);
    assert_eq!(app.buffer.text(), "let ab = compute(ab) + ab;");

    // Advance to implicit final tabstop ($0)
    named(&mut app, NamedKey::Tab);
    assert!(app.snippet_session.as_ref().unwrap().is_on_last());

    // Advance past $0 exits snippet mode
    named(&mut app, NamedKey::Tab);
    assert!(app.snippet_session.is_none());
}

#[test]
fn test_snippet_scope_matching() {
    let mut app_rust = test_app("");
    app_rust.set_language_mode(Some(LanguageId::Rust));
    text(&mut app_rust, "f");
    text(&mut app_rust, "n");
    named(&mut app_rust, NamedKey::Tab);
    assert!(app_rust.snippet_session.is_some());
    assert!(app_rust.buffer.text().starts_with("fn name()"));

    let mut app_py = test_app("");
    app_py.set_language_mode(Some(LanguageId::Python));
    text(&mut app_py, "f");
    text(&mut app_py, "n");
    named(&mut app_py, NamedKey::Tab);
    // In python, "fn" does NOT expand to a snippet; it just inserts tab indentation
    assert!(app_py.snippet_session.is_none());
    assert!(app_py.buffer.text().starts_with("fn "));

    // In python, "def" DOES expand
    let mut app_py2 = test_app("");
    app_py2.set_language_mode(Some(LanguageId::Python));
    text(&mut app_py2, "d");
    text(&mut app_py2, "e");
    text(&mut app_py2, "f");
    named(&mut app_py2, NamedKey::Tab);
    assert!(app_py2.snippet_session.is_some());
    assert!(app_py2.buffer.text().starts_with("def name("));
}

#[test]
fn test_sublime_snippet_file_loading_and_expansion() {
    let temp = tempfile::tempdir().unwrap();
    let snippet_file = temp.path().join("my_macro.sublime-snippet");
    std::fs::write(
        &snippet_file,
        r#"<snippet>
    <content><![CDATA[macro_rules! ${1:mac_name} {
	($2) => {
		$0
	};
}]]></content>
    <tabTrigger>mac</tabTrigger>
    <scope>source.rust</scope>
    <description>Macro definition</description>
</snippet>"#,
    )
    .unwrap();

    let mut app = test_app("");
    app.ui.snippets.registry.load_from_dir(temp.path());
    app.set_language_mode(Some(LanguageId::Rust));

    text(&mut app, "m");
    text(&mut app, "a");
    text(&mut app, "c");
    named(&mut app, NamedKey::Tab);

    assert!(app.snippet_session.is_some());
    assert!(app.buffer.text().contains("macro_rules! mac_name"));
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "mac_name"
    );
}

#[test]
fn test_json_snippet_file_loading_and_expansion() {
    let temp = tempfile::tempdir().unwrap();
    let json_file = temp.path().join("test_snippets.json");
    std::fs::write(
        &json_file,
        r#"{
            "Print Debug": {
                "prefix": "dbg",
                "body": "dbg!(${1:expr});$0",
                "scope": "source.rust",
                "description": "Debug print macro"
            }
        }"#,
    )
    .unwrap();

    let mut app = test_app("");
    app.ui.snippets.registry.load_from_dir(temp.path());
    app.set_language_mode(Some(LanguageId::Rust));

    text(&mut app, "d");
    text(&mut app, "b");
    text(&mut app, "g");
    named(&mut app, NamedKey::Tab);

    assert!(app.snippet_session.is_some());
    assert_eq!(app.buffer.text(), "dbg!(expr);");
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "expr"
    );
}

#[test]
fn test_shift_tab_navigates_backwards_through_tabstops() {
    let mut app = test_app("");
    app.expand_snippet("start ${1:first} mid ${2:second} end $0");
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "first"
    );

    // Tab forward to second
    named(&mut app, NamedKey::Tab);
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "second"
    );

    // Shift+Tab back to first
    app.modifiers.shift = true;
    named(&mut app, NamedKey::Tab);
    app.modifiers.shift = false;
    assert_eq!(
        app.buffer.text_range(app.cursors.primary().char_range()),
        "first"
    );
}

#[test]
fn test_escape_dismisses_snippet_session() {
    let mut app = test_app("");
    app.expand_snippet("fn ${1:name}() { $0 }");
    assert!(app.snippet_session.is_some());

    named(&mut app, NamedKey::Escape);
    assert!(app.snippet_session.is_none());
    assert_eq!(app.buffer.text(), "fn name() {  }");
}

struct DummyContext;
impl PlatformContext for DummyContext {
    fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        std::sync::Arc::new(|| {})
    }
    fn background_dialogs(&self) -> std::sync::Arc<dyn lux_platform::FileDialogs> {
        panic!("dialogs not needed")
    }
    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn lux_platform::FileDialogs {
        panic!("dialogs not needed")
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
        Err(lux_platform::PlatformError::Window("dummy".into()))
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {}
}

impl lux_platform::Clipboard for DummyContext {
    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
        Ok(())
    }
}

#[test]
fn test_snippet_picker_modal_and_palette_dispatch() {
    let mut app = test_app("");
    let ctx = DummyContext;

    // Open snippet picker
    crate::snippets::dispatch::open_snippet_picker(&mut app);
    assert!(app.ui.snippets.picker.is_some());

    // Type query "lorem"
    let picker_key = KeyEvent {
        key: Key::Character("lorem".to_string()),
        text: Some("lorem".to_string()),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    crate::snippets::handle_snippet_picker_key(&mut app, &ctx, &picker_key);
    assert_eq!(app.ui.snippets.picker.as_ref().unwrap().query, "lorem");

    // Hit Enter to insert
    let enter_key = KeyEvent {
        key: Key::Named(NamedKey::Enter),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    crate::snippets::handle_snippet_picker_key(&mut app, &ctx, &enter_key);
    assert!(app.ui.snippets.picker.is_none());
    assert!(app.buffer.text().starts_with("Lorem ipsum"));
}
