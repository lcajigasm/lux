//! Panel keyboard, wheel, and IME routing. These drive the same functions
//! the window loop calls. They are not a substitute for pressing keys on
//! a Mac.

use lux_platform::event::KeyEvent;
use lux_platform::{
    Clipboard, ElementState, FileDialogOptions, FileDialogs, Ime, Key, Modifiers, NamedKey,
    PhysicalKey, PlatformContext, PlatformError, PlatformWindow, ScrollDelta, WindowAttributes,
};
use lux_ui::focus::FocusTarget;

use super::motion::key_event_to_chord;
use crate::app::App;
use crate::field::{edit_focused_field, route_ime};
use crate::overlay_wheel::scroll_open_overlay;
use crate::search::handle_search_key;

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

fn press(key: Key, physical: Option<PhysicalKey>) -> KeyEvent {
    KeyEvent {
        key,
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical,
    }
}

struct Clip;

impl Clipboard for Clip {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok("pasted".into())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}

struct Ctx;

impl FileDialogs for Ctx {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<std::path::PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
        lux_platform::dialog::SaveDecision::Discard
    }
}

impl PlatformContext for Ctx {
    fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        std::sync::Arc::new(|| {})
    }
    fn background_dialogs(&self) -> std::sync::Arc<dyn FileDialogs> {
        unimplemented!()
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        unimplemented!()
    }
    fn clipboard(&self) -> &dyn Clipboard {
        &Clip
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &Ctx
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn command_alt(app: &mut App) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        alt: true,
        shift: false,
    };
}

#[test]
fn physical_key_builds_us_chords_for_option_and_spanish_glyphs() {
    let fancy_f = press(Key::Character("ƒ".into()), Some(PhysicalKey::Letter('f')));
    let modifiers = Modifiers {
        meta: true,
        alt: true,
        control: false,
        shift: false,
    };
    let chord = key_event_to_chord(&fancy_f, &modifiers).unwrap();
    assert_eq!(chord.key, "f");
    assert!(chord.cmd && chord.alt && !chord.shift);

    let cedilla = press(Key::Character("ç".into()), Some(PhysicalKey::Slash));
    let command = Modifiers {
        meta: true,
        alt: false,
        control: false,
        shift: false,
    };
    assert_eq!(key_event_to_chord(&cedilla, &command).unwrap().key, "/");

    let bracket = press(Key::Character("`".into()), Some(PhysicalKey::BracketLeft));
    let option_command = Modifiers {
        meta: true,
        alt: true,
        control: false,
        shift: false,
    };
    assert_eq!(
        key_event_to_chord(&bracket, &option_command).unwrap().key,
        "["
    );
}

#[test]
fn cedilla_is_search_text_and_option_command_c_toggles_case() {
    let mut app = app();
    crate::search::open_find(&mut app, false);
    let ctx = Ctx;
    let cedilla = press(Key::Character("ç".into()), Some(PhysicalKey::Backslash));
    assert!(handle_search_key(&mut app, &ctx, &cedilla));
    let search = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(search.find_query.as_str(), "ç");
    assert!(!search.case_sensitive);

    command_alt(&mut app);
    let toggle = press(Key::Character("ç".into()), Some(PhysicalKey::Letter('c')));
    assert!(handle_search_key(&mut app, &ctx, &toggle));
    assert!(app.ui.search_bar.as_ref().unwrap().case_sensitive);
    assert_eq!(app.ui.search_bar.as_ref().unwrap().find_query.as_str(), "ç");
}

#[test]
fn palette_paste_undo_and_escape_stay_out_of_the_document() {
    let mut app = app();
    app.open_command_palette("");
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        alt: false,
        shift: false,
    };
    let paste = press(Key::Character("v".into()), Some(PhysicalKey::Letter('v')));
    assert!(edit_focused_field(&mut app, &paste, &Clip));
    assert_eq!(
        app.ui.command_palette.as_ref().unwrap().query.as_str(),
        "pasted"
    );
    assert_eq!(app.buffer.text(), "");

    let undo = press(Key::Character("z".into()), Some(PhysicalKey::Letter('z')));
    assert!(edit_focused_field(&mut app, &undo, &Clip));
    assert_eq!(app.ui.command_palette.as_ref().unwrap().query.as_str(), "");

    app.modifiers = Modifiers::default();
    let esc = press(Key::Named(NamedKey::Escape), None);
    assert!(crate::palette::handle_palette_key(&mut app, &Ctx, &esc));
    assert!(app.ui.command_palette.is_none());
    assert_eq!(app.chrome.focus.current(), FocusTarget::Editor);
}

#[test]
fn ime_commit_and_wheel_go_to_the_open_panel() {
    let mut app = app();
    app.open_command_palette("");
    app.viewport.scroll_y = 80.0;
    assert!(scroll_open_overlay(
        &mut app,
        ScrollDelta::Lines { x: 0.0, y: -2.0 }
    ));
    assert_eq!(app.viewport.scroll_y, 80.0);
    assert!(app.ui.command_palette.as_ref().unwrap().selected_index > 0);

    assert!(route_ime(&mut app, &Ime::Commit("á".into())));
    assert_eq!(app.ui.command_palette.as_ref().unwrap().query.as_str(), "á");
    assert_eq!(app.buffer.text(), "");
}

#[test]
fn a_command_chord_drops_the_grave_committed_ahead_of_its_modifiers() {
    let mut app = app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"alpha\n").unwrap();
    let ctx = Ctx;
    // Spanish ISO: physical `[` is a dead grave. The commit is queued
    // before Option+Command is visible, then the chord follows.
    let chords = [
        PhysicalKey::Letter('f'),
        PhysicalKey::Slash,
        PhysicalKey::BracketLeft,
        PhysicalKey::BracketRight,
        PhysicalKey::Letter('s'),
        PhysicalKey::Letter('c'),
    ];
    for physical in chords {
        app.buffer = lux_core::buffer::Buffer::from_bytes(b"alpha\n").unwrap();
        app.ui.search_bar = None;
        app.ui.snippets.picker = None;
        crate::ime_hold::observe(&mut app, Ime::Commit("`".into()));
        assert_eq!(app.buffer.text(), "alpha\n");
        let alt = physical != PhysicalKey::Slash;
        app.modifiers = Modifiers {
            meta: cfg!(target_os = "macos"),
            control: cfg!(not(target_os = "macos")),
            alt: alt && cfg!(target_os = "macos"),
            shift: false,
        };
        let key = press(Key::Character("`".into()), Some(physical));
        app.handle_keyboard_event(&ctx, &key);
        // Control+] is indent. Option+Command+] on macOS is unfold, so the
        // line stays put. Either way the queued grave is not inserted.
        let expected = if physical == PhysicalKey::BracketRight && cfg!(not(target_os = "macos")) {
            "    alpha\n"
        } else {
            "alpha\n"
        };
        assert_eq!(app.buffer.text(), expected, "{physical:?}");
        assert!(!app.buffer.text().contains('`'), "{physical:?}");
    }
}

#[test]
fn a_plain_ime_commit_lands_when_its_key_is_released() {
    let mut app = app();
    crate::ime_hold::observe(&mut app, Ime::Commit("á".into()));
    assert_eq!(app.buffer.text(), "");
    let release = KeyEvent {
        key: Key::Character("a".into()),
        text: None,
        state: ElementState::Released,
        repeat: false,
        physical: Some(PhysicalKey::Letter('a')),
    };
    app.handle_keyboard_event(&Ctx, &release);
    assert_eq!(app.buffer.text(), "á");
}

#[test]
fn completion_popup_owns_the_wheel() {
    let mut app = app();
    app.viewport.scroll_y = 50.0;
    app.ui.lsp.completion_popup = Some(crate::lsp::CompletionPopupState::showing(vec![
        lux_lsp::async_lsp::lsp_types::CompletionItem {
            label: "alpha".into(),
            ..Default::default()
        },
        lux_lsp::async_lsp::lsp_types::CompletionItem {
            label: "beta".into(),
            ..Default::default()
        },
    ]));
    assert!(scroll_open_overlay(
        &mut app,
        ScrollDelta::Lines { x: 0.0, y: -1.0 }
    ));
    assert_eq!(
        app.ui.lsp.completion_popup.as_ref().unwrap().selected_index,
        1
    );
    assert_eq!(app.viewport.scroll_y, 50.0);
}

#[path = "panel_event_tests.rs"]
mod panel_event_tests;
