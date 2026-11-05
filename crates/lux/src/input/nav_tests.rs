//! Keyboard coverage for the remaining P0 palette, goto, and edit chords.

use lux_config::{ChordSequence, Platform};
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, FileDialogs, Key, Modifiers, NamedKey, PlatformContext};
use std::path::PathBuf;
use std::sync::Arc;

use super::tests::{app, command, named};

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
        lux_platform::dialog::SaveDecision::Discard
    }
}

static DIALOGS: DummyDialogs = DummyDialogs;

struct Ctx;
impl lux_platform::Clipboard for Ctx {
    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
        Ok(())
    }
}
impl PlatformContext for Ctx {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(DummyDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &DIALOGS
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

fn press(app: &mut crate::app::App, key: Key, text: Option<&str>) {
    app.handle_keyboard_event(
        &Ctx,
        &KeyEvent {
            key,
            text: text.map(str::to_owned),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );
}

fn type_str(app: &mut crate::app::App, text: &str) {
    for ch in text.chars() {
        let token = ch.to_string();
        press(app, Key::Character(token.clone()), Some(&token));
    }
}

fn command_key(app: &mut crate::app::App, key: &str, shift: bool) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        ..Modifiers::default()
    };
    press(app, Key::Character(key.into()), Some(key));
    app.modifiers = Modifiers::default();
}

fn ctrl(app: &mut crate::app::App, key: &str) {
    app.modifiers.control = true;
    press(app, Key::Character(key.into()), Some(key));
    app.modifiers = Modifiers::default();
}

fn command_named(app: &mut crate::app::App, key: NamedKey, shift: bool) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        ..Modifiers::default()
    };
    press(app, Key::Named(key), None);
    app.modifiers = Modifiers::default();
}

#[test]
fn sublime_chords_resolve_for_the_remaining_p0_commands() {
    let keymap = lux_config::keymap::presets::sublime();
    let mac = [
        ("cmd+shift+p", "command_palette"),
        ("cmd+r", "goto_symbol"),
        ("cmd+shift+r", "goto_symbol_in_project"),
        ("ctrl+g", "goto_line"),
        ("cmd+d", "select_next_occurrence"),
        ("ctrl+cmd+g", "select_all_occurrences"),
        ("cmd+shift+l", "split_selection_into_lines"),
        ("cmd+l", "expand_to_line"),
        ("cmd+shift+d", "duplicate_line"),
        ("cmd+enter", "insert_line_after"),
        ("cmd+shift+enter", "insert_line_before"),
        ("cmd+y", "redo"),
        ("cmd+e", "use_selection_for_find"),
    ];
    for (chord, command_name) in mac {
        let parsed: ChordSequence = chord.parse().unwrap();
        assert_eq!(
            keymap.command_for(&parsed, Platform::Mac),
            Some(command_name),
            "{chord}"
        );
    }
    let linux_line: ChordSequence = "ctrl+g".parse().unwrap();
    assert_eq!(
        keymap.command_for(&linux_line, Platform::Linux),
        Some("goto_line")
    );
}

#[test]
fn command_shift_p_filters_the_palette_and_undoes() {
    let mut app = app("");
    for ch in ["h", "e", "l", "l", "o"] {
        press(&mut app, Key::Character(ch.into()), Some(ch));
    }
    assert_eq!(app.buffer.text(), "hello");
    command_key(&mut app, "p", true);
    assert!(app.ui.command_palette.is_some());
    type_str(&mut app, "undo");
    let palette = app.ui.command_palette.as_ref().unwrap();
    let results = palette.results(&lux_ui::command_palette::Context::new());
    assert_eq!(results[0].command.id, "edit.undo");
    press(&mut app, Key::Named(NamedKey::Enter), None);
    assert!(app.ui.command_palette.is_none());
    assert_eq!(app.buffer.text(), "");
}

#[test]
fn command_r_ctrl_g_and_suffixes_preview_from_the_keyboard() {
    let mut app = app("fn beta() {}\n\nfn alpha() {}\n");
    app.documents.path = Some(PathBuf::from("sample.rs"));
    app.set_language_mode(Some(lux_syntax::LanguageId::Rust));
    command_key(&mut app, "r", false);
    let goto = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(goto.query.as_str(), "@");
    assert!(goto.items.iter().any(|item| item.label == "alpha"));
    type_str(&mut app, "alpha");
    press(&mut app, Key::Named(NamedKey::Enter), None);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.cursors.primary().head.line, 2);

    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::document_start()));
    ctrl(&mut app, "g");
    assert_eq!(app.ui.goto_anything.as_ref().unwrap().query.as_str(), ":");
    type_str(&mut app, "1");
    assert_eq!(app.cursors.primary().head.line, 0);
    press(&mut app, Key::Named(NamedKey::Escape), None);
    assert!(app.ui.goto_anything.is_none());

    command_key(&mut app, "p", false);
    type_str(&mut app, "#alpha");
    let labels: Vec<_> = app
        .ui
        .goto_anything
        .as_ref()
        .unwrap()
        .items
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(labels.iter().any(|label| label.contains("alpha")));
    press(&mut app, Key::Named(NamedKey::Enter), None);
    assert_eq!(app.cursors.primary().head.line, 2);
}

#[test]
fn command_enter_inserts_a_line_and_command_y_redoes_a_word() {
    let mut app = app("alpha\nbeta\n");
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        1,
    )));
    command_named(&mut app, NamedKey::Enter, false);
    assert_eq!(app.buffer.text(), "alpha\n\nbeta\n");
    assert_eq!(app.cursors.primary().head.line, 1);

    command_named(&mut app, NamedKey::Enter, true);
    assert_eq!(app.buffer.text(), "alpha\n\n\nbeta\n");
    assert_eq!(app.cursors.primary().head.line, 1);

    let mut typed = super::tests::app("");
    for ch in ["h", "i", " ", "y", "o"] {
        press(&mut typed, Key::Character(ch.into()), Some(ch));
    }
    command(&mut typed, "z", false);
    assert_eq!(typed.buffer.text(), "hi");
    command_key(&mut typed, "y", false);
    assert_eq!(typed.buffer.text(), "hi yo");
}

#[test]
fn command_e_uses_the_word_under_the_cursor_for_find() {
    let mut app = app("one two two");
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        9,
    )));
    command_key(&mut app, "e", false);
    let query = app.ui.search_bar.as_ref().unwrap().find_query.as_str();
    assert_eq!(query, "two");
    assert_eq!(app.buffer.text(), "one two two");
}

#[test]
fn a_typed_word_and_its_backspaces_are_one_undo_each() {
    let mut app = app("");
    for ch in ["h", "e", "l", "l", "o"] {
        press(&mut app, Key::Character(ch.into()), Some(ch));
    }
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "");
    for ch in ["h", "e", "l", "l", "o"] {
        press(&mut app, Key::Character(ch.into()), Some(ch));
    }
    for _ in 0..5 {
        named(&mut app, NamedKey::Backspace);
    }
    assert_eq!(app.buffer.text(), "");
    command(&mut app, "z", false);
    assert_eq!(app.buffer.text(), "hello");
}
