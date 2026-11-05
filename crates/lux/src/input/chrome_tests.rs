//! Keyboard coverage for find, zoom, the sidebar chord, palette order, and key repeat.

use lux_config::{ChordSequence, Platform};
use lux_platform::event::{KeyEvent, PhysicalKey};
use lux_platform::{ElementState, FileDialogs, Key, Modifiers, NamedKey, PlatformContext};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::tests::app;

struct Dialogs;
impl FileDialogs for Dialogs {
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

static DIALOGS: Dialogs = Dialogs;

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
        Arc::new(Dialogs)
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

fn send(app: &mut crate::app::App, event: &KeyEvent) {
    app.handle_keyboard_event(&Ctx, event);
}

fn press(app: &mut crate::app::App, key: Key, text: Option<&str>, physical: Option<PhysicalKey>) {
    send(
        app,
        &KeyEvent {
            key,
            text: text.map(str::to_owned),
            state: ElementState::Pressed,
            repeat: false,
            physical,
        },
    );
}

fn command(shift: bool, alt: bool) -> Modifiers {
    Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        alt,
    }
}

fn chord(app: &mut crate::app::App, key: &str, shift: bool, alt: bool) {
    app.modifiers = command(shift, alt);
    press(
        app,
        Key::Character(key.into()),
        Some(key),
        Some(physical_for(key)),
    );
    app.modifiers = Modifiers::default();
}

fn physical_for(key: &str) -> PhysicalKey {
    match key {
        "[" => PhysicalKey::BracketLeft,
        "]" => PhysicalKey::BracketRight,
        "-" => PhysicalKey::Minus,
        "=" => PhysicalKey::Equal,
        "," => PhysicalKey::Comma,
        "0" => PhysicalKey::Digit(0),
        other => PhysicalKey::Letter(other.chars().next().unwrap_or('a')),
    }
}

fn counter(app: &crate::app::App) -> String {
    let search = app.ui.search_bar.as_ref().expect("find bar");
    crate::render::search::match_counter_label(
        search.find_query.is_empty(),
        search.search_bar.matches().len(),
        search.search_bar.current_index(),
        search.in_selection,
    )
}

#[test]
fn find_replace_and_project_find_open_from_the_chord() {
    let mut app = app("one two one");
    chord(&mut app, "f", false, false);
    assert!(app.ui.search_bar.is_some());
    assert!(!app.ui.search_bar.as_ref().unwrap().replace_mode);
    assert_eq!(app.buffer.text(), "one two one");

    app.ui.search_bar = None;
    if cfg!(target_os = "macos") {
        app.modifiers = Modifiers {
            meta: true,
            alt: true,
            control: false,
            shift: false,
        };
        press(
            &mut app,
            Key::Character("ƒ".into()),
            Some("ƒ"),
            Some(PhysicalKey::Letter('f')),
        );
        app.modifiers = Modifiers::default();
    } else {
        chord(&mut app, "h", false, false);
    }
    let search = app.ui.search_bar.as_ref().expect("replace");
    assert!(search.replace_mode);
    assert_eq!(app.buffer.text(), "one two one");

    app.ui.search_bar = None;
    chord(&mut app, "f", true, false);
    assert!(app.ui.project_search.is_some());
    assert_eq!(app.buffer.text(), "one two one");
}

#[test]
fn option_command_f_resolves_from_the_physical_key_on_mac() {
    let keymap = lux_config::keymap::presets::sublime();
    let modifiers = Modifiers {
        meta: true,
        alt: true,
        control: false,
        shift: false,
    };
    let event = KeyEvent {
        key: Key::Character("ƒ".into()),
        text: Some("ƒ".into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: Some(PhysicalKey::Letter('f')),
    };
    let chord = super::motion::key_event_to_chord(&event, &modifiers).unwrap();
    assert_eq!(chord.key, "f");
    let found = keymap.command_for(&ChordSequence(vec![chord]), Platform::Mac);
    assert_eq!(found, Some("replace"));

    let glyph = KeyEvent {
        physical: None,
        ..event
    };
    let missed = super::motion::key_event_to_chord(&glyph, &modifiers).unwrap();
    assert_ne!(missed.key, "f");
    assert!(
        keymap
            .command_for(&ChordSequence(vec![missed]), Platform::Mac)
            .is_none()
    );
}

#[test]
fn mac_shift_command_bracket_uses_the_physical_key() {
    let keymap = lux_config::keymap::presets::sublime();
    let modifiers = Modifiers {
        meta: true,
        shift: true,
        alt: false,
        control: false,
    };
    let event = KeyEvent {
        key: Key::Character("`".into()),
        text: Some("`".into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: Some(PhysicalKey::BracketLeft),
    };
    let chord = super::motion::key_event_to_chord(&event, &modifiers).unwrap();
    assert_eq!(chord.key, "[");
    assert!(chord.cmd && chord.shift && !chord.alt);
    let found = keymap.command_for(&ChordSequence(vec![chord]), Platform::Mac);
    assert_eq!(found, Some("prev_tab"));
}

#[test]
fn find_counter_reports_n_of_m_and_advances() {
    let mut app = app("one two one");
    chord(&mut app, "f", false, false);
    for ch in ["o", "n", "e"] {
        press(&mut app, Key::Character(ch.into()), Some(ch), None);
    }
    assert_eq!(counter(&app), "1 of 2 matches");
    if cfg!(target_os = "macos") {
        chord(&mut app, "g", false, false);
    } else {
        app.modifiers = Modifiers::default();
        press(&mut app, Key::Named(NamedKey::F(3)), None, None);
    }
    assert_eq!(counter(&app), "2 of 2 matches");
    assert_eq!(app.buffer.text(), "one two one");
}

#[test]
fn move_line_chord_swaps_the_current_line() {
    let mut app = app("alpha\nbeta\n");
    press(&mut app, Key::Named(NamedKey::ArrowDown), None, None);
    if cfg!(target_os = "macos") {
        app.modifiers = Modifiers {
            meta: true,
            control: true,
            alt: false,
            shift: false,
        };
    } else {
        app.modifiers = Modifiers {
            meta: false,
            control: true,
            alt: false,
            shift: true,
        };
    }
    press(&mut app, Key::Named(NamedKey::ArrowUp), None, None);
    app.modifiers = Modifiers::default();
    assert_eq!(app.buffer.text(), "beta\nalpha\n");
}

#[test]
fn sidebar_chord_toggles_and_command_b_stays_build() {
    let mut app = app("alpha");
    assert!(app.ui.sidebar.is_none());
    chord(&mut app, "k", false, false);
    chord(&mut app, "b", false, false);
    assert!(app.ui.sidebar.as_ref().is_some_and(|s| s.visible));
    chord(&mut app, "k", false, false);
    chord(&mut app, "b", false, false);
    assert!(app.ui.sidebar.as_ref().is_some_and(|s| !s.visible));
    assert_eq!(app.buffer.text(), "alpha");

    let keymap = lux_config::keymap::presets::sublime();
    let host = if cfg!(target_os = "macos") {
        Platform::Mac
    } else {
        Platform::Linux
    };
    let build = if cfg!(target_os = "macos") {
        "cmd+b"
    } else {
        "ctrl+b"
    };
    let seq: ChordSequence = build.parse().unwrap();
    assert_eq!(keymap.command_for(&seq, host), Some("build"));
}

#[test]
fn font_zoom_changes_metrics_and_reset_restores_16() {
    let mut app = app("abcd");
    let before = app.editor_view.metrics().font_size;
    let width = app
        .editor_view
        .build_frame(
            &app.buffer,
            &app.cursors,
            &app.viewport,
            Duration::ZERO,
            Some(&app.fold_state),
        )
        .lines[0]
        .layout
        .width();
    chord(&mut app, "=", false, false);
    assert!((app.editor_view.metrics().font_size - (before + 1.0)).abs() < 0.01);
    let wider = app
        .editor_view
        .build_frame(
            &app.buffer,
            &app.cursors,
            &app.viewport,
            Duration::ZERO,
            Some(&app.fold_state),
        )
        .lines[0]
        .layout
        .width();
    assert!(wider > width + 0.5);
    chord(&mut app, "-", false, false);
    assert!((app.editor_view.metrics().font_size - before).abs() < 0.01);
    for _ in 0..20 {
        chord(&mut app, "-", false, false);
    }
    assert!((app.editor_view.metrics().font_size - 8.0).abs() < 0.01);
    chord(&mut app, "0", false, false);
    assert!((app.editor_view.metrics().font_size - before).abs() < 0.01);
    assert_eq!(app.buffer.text(), "abcd");
    app.zoom_font(4.0);
    app.set_display_scale(2.0);
    assert!((app.editor_view.metrics().font_size - (before + 4.0) * 2.0).abs() < 0.01);
}

#[test]
fn settings_chord_opens_the_panel_and_escape_closes_it() {
    let mut app = app("alpha");
    chord(&mut app, ",", false, false);
    assert!(app.ui.settings.panel.is_some());
    assert_eq!(app.buffer.text(), "alpha");
    press(&mut app, Key::Named(NamedKey::Escape), None, None);
    assert!(app.ui.settings.panel.is_none());
    assert_eq!(app.buffer.text(), "alpha");
}

#[test]
fn a_repeat_inserts_once_and_a_release_inserts_nothing() {
    let mut app = app("");
    press(&mut app, Key::Character("a".into()), Some("a"), None);
    send(
        &mut app,
        &KeyEvent {
            key: Key::Character("a".into()),
            text: Some("a".into()),
            state: ElementState::Pressed,
            repeat: true,
            physical: None,
        },
    );
    send(
        &mut app,
        &KeyEvent {
            key: Key::Character("a".into()),
            text: Some("a".into()),
            state: ElementState::Released,
            repeat: true,
            physical: None,
        },
    );
    assert_eq!(app.buffer.text(), "aa");
}

#[test]
fn palette_shows_shortcuts_and_floats_the_last_command() {
    let mut app = app("beta\nalpha\n");
    press(&mut app, Key::Named(NamedKey::ArrowDown), None, None);
    chord(&mut app, "p", true, false);
    let shortcuts = app.ui.command_palette.as_ref().unwrap().shortcuts.clone();
    let save = shortcuts.get("file.save").map(String::as_str);
    if cfg!(target_os = "macos") {
        assert_eq!(save, Some("⌘S"));
        assert_eq!(
            shortcuts.get("view.toggle_side_bar").map(String::as_str),
            Some("⌘K ⌘B")
        );
    } else {
        assert_eq!(save, Some("⌃S"));
        assert_eq!(
            shortcuts.get("view.toggle_side_bar").map(String::as_str),
            Some("⌃K ⌃B")
        );
    }
    for ch in "Swap Line Up".chars() {
        let token = ch.to_string();
        press(&mut app, Key::Character(token.clone()), Some(&token), None);
    }
    let palette = app.ui.command_palette.as_ref().unwrap();
    let ctx = lux_ui::command_palette::Context::new();
    let results = palette.results(&ctx);
    assert_eq!(results[0].command.id, "edit.swap_line_up");
    press(&mut app, Key::Named(NamedKey::Enter), None, None);
    assert_eq!(app.buffer.text(), "alpha\nbeta\n");
    chord(&mut app, "p", true, false);
    let again = app.ui.command_palette.as_ref().unwrap();
    let top = again.results(&lux_ui::command_palette::Context::new());
    assert_eq!(top[0].command.id, "edit.swap_line_up");
}
