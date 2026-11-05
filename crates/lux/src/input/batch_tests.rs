//! Field typing, selector keys, the language picker, and word/pause undo.

use std::path::PathBuf;

use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, Modifiers, NamedKey, PhysicalKey, ScrollDelta};
use lux_syntax::LanguageId;

use crate::app::App;

fn key(text: &str, state: ElementState) -> KeyEvent {
    KeyEvent {
        key: Key::Character(text.into()),
        text: Some(text.into()),
        state,
        repeat: false,
        physical: Some(PhysicalKey::Letter(text.chars().next().unwrap_or('q'))),
    }
}

fn named(key: NamedKey, state: ElementState) -> KeyEvent {
    KeyEvent {
        key: Key::Named(key),
        text: None,
        state,
        repeat: false,
        physical: None,
    }
}

fn command(app: &mut App, shift: bool) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        alt: false,
        shift,
    };
}

fn tap(app: &mut App, event: &KeyEvent) {
    app.handle_keyboard_event(&super::Ctx, event);
}

fn query_of(app: &App) -> String {
    if let Some(panel) = &app.ui.command_palette {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.goto_anything {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.language_picker {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.theme_picker {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.snippets.picker {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.search_bar {
        return panel.find_query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.project_search {
        return panel.find_query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.project.picker {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.build.picker {
        return panel.query.as_str().to_string();
    }
    String::new()
}

fn type_q(app: &mut App) {
    tap(app, &key("q", ElementState::Pressed));
    tap(app, &key("q", ElementState::Released));
}

#[test]
fn a1_one_physical_key_types_once_in_every_field() {
    let openers: [fn(&mut App); 9] = [
        |app| app.open_command_palette(""),
        |app| app.open_goto_anything(""),
        |app| app.open_language_picker(),
        |app| {
            app.ui.theme_picker = Some(crate::theme_picker::ThemePickerState::new("Monokai"));
        },
        |app| {
            app.ui.snippets.picker =
                Some(crate::snippets::picker::SnippetPickerState::new(Vec::new()));
        },
        |app| crate::search::open_find(app, false),
        |app| crate::project_search::open_project_find(app),
        |app| {
            app.ui.project.picker = Some(crate::project::ProjectSwitchState {
                projects: Vec::new(),
                query: crate::field::QueryField::new(),
                selected: 0,
            });
        },
        |app| {
            app.ui.build.picker = Some(crate::build::BuildPickerState {
                variants: Vec::new(),
                query: crate::field::QueryField::new(),
                selected: 0,
            });
        },
    ];
    for open in openers {
        let mut app = super::app();
        open(&mut app);
        type_q(&mut app);
        assert_eq!(query_of(&app), "q");
        assert_eq!(app.buffer.text(), "");
    }
}

fn arrow(app: &mut App) {
    tap(app, &named(NamedKey::ArrowDown, ElementState::Pressed));
    tap(app, &named(NamedKey::ArrowDown, ElementState::Released));
}

#[test]
fn a2_arrows_and_enter_fire_once_per_press() {
    let mut app = super::app();
    app.open_language_picker();
    arrow(&mut app);
    assert_eq!(app.ui.language_picker.as_ref().unwrap().selected_index, 1);
    tap(&mut app, &named(NamedKey::Enter, ElementState::Pressed));
    tap(&mut app, &named(NamedKey::Enter, ElementState::Released));
    assert!(app.ui.language_picker.is_none());
    assert_eq!(app.documents.language(), Some(LanguageId::Rust));
    assert_eq!(
        app.syntax.as_ref().map(|syntax| syntax.language),
        Some(LanguageId::Rust)
    );

    let mut app = super::app();
    app.ui.theme_picker = Some(crate::theme_picker::ThemePickerState::new("Monokai"));
    arrow(&mut app);
    assert_eq!(app.ui.theme_picker.as_ref().unwrap().selected_index, 1);

    let mut app = super::app();
    app.open_command_palette("");
    arrow(&mut app);
    assert_eq!(app.ui.command_palette.as_ref().unwrap().selected_index, 1);

    let mut app = super::app();
    app.ui.build.picker = Some(crate::build::BuildPickerState {
        variants: vec![
            crate::build::BuildVariant {
                name: "One".into(),
                command: "true".into(),
                args: Vec::new(),
                file_regex: None,
            },
            crate::build::BuildVariant {
                name: "Two".into(),
                command: "true".into(),
                args: Vec::new(),
                file_regex: None,
            },
        ],
        query: crate::field::QueryField::new(),
        selected: 0,
    });
    arrow(&mut app);
    assert_eq!(app.ui.build.picker.as_ref().unwrap().selected, 1);

    let mut app = super::app();
    app.ui.project.picker = Some(crate::project::ProjectSwitchState {
        projects: vec![PathBuf::from("/tmp/one"), PathBuf::from("/tmp/two")],
        query: crate::field::QueryField::new(),
        selected: 0,
    });
    arrow(&mut app);
    assert_eq!(app.ui.project.picker.as_ref().unwrap().selected, 1);

    let mut app = super::app();
    app.ui.snippets.picker = Some(crate::snippets::picker::SnippetPickerState::new(vec![
        crate::snippets::registry::Snippet {
            name: "One".into(),
            trigger: "one".into(),
            content: "one".into(),
            scope: None,
            description: String::new(),
        },
        crate::snippets::registry::Snippet {
            name: "Two".into(),
            trigger: "two".into(),
            content: "two".into(),
            scope: None,
            description: String::new(),
        },
    ]));
    arrow(&mut app);
    assert_eq!(app.ui.snippets.picker.as_ref().unwrap().selected_index, 1);
}

#[test]
fn f4_language_picker_keyboard_scroll_fuzzy_and_live_apply() {
    let mut app = super::app();
    command(&mut app, true);
    tap(&mut app, &key("m", ElementState::Pressed));
    tap(&mut app, &key("m", ElementState::Released));
    assert!(app.ui.language_picker.is_some());
    app.ui.language_picker = None;

    let mut app = super::app();
    app.ui.status_bar.layout.language_rect = crate::status_bar::Rect {
        x: 10.0,
        y: 10.0,
        width: 40.0,
        height: 16.0,
    };
    crate::chrome::handle_status_bar_click(&mut app, 12.0, 12.0);
    assert!(app.ui.language_picker.is_some());

    let picker = app.ui.language_picker.as_mut().unwrap();
    picker.query = crate::field::QueryField::from_text("pythn");
    let ranked = picker.filtered();
    assert_eq!(ranked[0].1, "Python");
    assert!(ranked.iter().all(|(_, name)| *name != "Rust"));

    app.ui.language_picker = Some(crate::language_picker::LanguagePickerState::new());
    let scroll_before = app.viewport.scroll_y;
    assert!(crate::overlay_wheel::scroll_open_overlay(
        &mut app,
        ScrollDelta::Lines { x: 0.0, y: -1.0 }
    ));
    assert_eq!(app.ui.language_picker.as_ref().unwrap().selected_index, 1);
    assert_eq!(app.viewport.scroll_y, scroll_before);

    tap(&mut app, &named(NamedKey::Enter, ElementState::Pressed));
    tap(&mut app, &named(NamedKey::Enter, ElementState::Released));
    assert!(app.ui.language_picker.is_none());
    assert_eq!(app.documents.language(), Some(LanguageId::Rust));
    assert_eq!(
        app.syntax.as_ref().map(|syntax| syntax.language),
        Some(LanguageId::Rust)
    );
}

fn type_letter(app: &mut App, text: &str) {
    app.modifiers = Modifiers::default();
    tap(app, &key(text, ElementState::Pressed));
}

#[test]
fn c4_word_pause_undo_restores_selections() {
    let mut app = super::app();
    type_letter(&mut app, "a");
    type_letter(&mut app, "b");
    command(&mut app, false);
    tap(&mut app, &key("z", ElementState::Pressed));
    assert_eq!(app.buffer.text(), "");
    assert_eq!(app.cursors.primary().head.char, 0);

    type_letter(&mut app, "a");
    type_letter(&mut app, "b");
    app.history.note_typing_instant(
        std::time::Instant::now()
            - (lux_core::history::TYPING_PAUSE + std::time::Duration::from_millis(1)),
    );
    type_letter(&mut app, "c");
    assert_eq!(app.buffer.text(), "abc");
    command(&mut app, false);
    tap(&mut app, &key("z", ElementState::Pressed));
    assert_eq!(app.buffer.text(), "ab");
    assert_eq!(app.cursors.primary().head.char, 2);

    app.buffer = lux_core::buffer::Buffer::from_bytes(b"hello").unwrap();
    let end = Position::from_char(&app.buffer, 5);
    app.cursors =
        lux_core::multicursor::MultiCursor::new(Selection::new(Position::document_start(), end));
    type_letter(&mut app, "z");
    assert_eq!(app.buffer.text(), "z");
    command(&mut app, false);
    tap(&mut app, &key("z", ElementState::Pressed));
    assert_eq!(app.buffer.text(), "hello");
    let range = app.cursors.primary().char_range();
    assert_eq!((range.start, range.end), (0, 5));
}
