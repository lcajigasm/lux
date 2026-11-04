use super::*;
use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, FileDialogs, Key, Modifiers, NamedKey, PlatformContext};
use lux_ui::focus::FocusTarget;
use std::path::PathBuf;
use std::sync::Arc;

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<PathBuf> {
        vec![]
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

static DUMMY_DIALOGS: DummyDialogs = DummyDialogs;

struct DummyContext;
impl lux_platform::Clipboard for DummyContext {
    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
        Ok(())
    }
}
impl PlatformContext for DummyContext {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(DummyDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &DUMMY_DIALOGS
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

fn test_app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

fn send_search_key(app: &mut App, event: &KeyEvent) {
    let ctx = DummyContext;
    if app.ui.search_bar.is_some() && crate::search::handle_search_key(app, &ctx, event) {
        return;
    }
    app.handle_key(event);
}

fn type_search_str(app: &mut App, s: &str) {
    for ch in s.chars() {
        let str_val = ch.to_string();
        send_search_key(
            app,
            &KeyEvent {
                key: Key::Character(str_val.clone()),
                text: Some(str_val),
                state: ElementState::Pressed,
                repeat: false,
                physical: None,
            },
        );
    }
}

fn press_search_key(app: &mut App, key: NamedKey) {
    send_search_key(
        app,
        &KeyEvent {
            key: Key::Named(key),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );
}

#[test]
fn open_find_prepopulates_from_cursor_selection() {
    let mut app = test_app("hello world hello world");
    // Select "world" at index 6..11
    app.cursors = MultiCursor::new(Selection::new(
        Position::from_char(&app.buffer, 6),
        Position::from_char(&app.buffer, 11),
    ));

    crate::search::open_find(&mut app, false);

    assert!(app.ui.search_bar.is_some());
    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.find_query, "world");
    assert_eq!(state.search_bar.matches().len(), 2);
    assert_eq!(app.chrome.focus.current(), FocusTarget::Search);
}

#[test]
fn open_find_with_multiline_selection_sets_in_selection() {
    let mut app = test_app("line one\nline two\nline three\n");
    // Multiline selection
    app.cursors = MultiCursor::new(Selection::new(
        Position::from_char(&app.buffer, 0),
        Position::from_char(&app.buffer, 18),
    ));

    crate::search::open_find(&mut app, false);

    let state = app.ui.search_bar.as_ref().unwrap();
    assert!(state.in_selection);
    assert!(state.selection_range.is_some());
}

#[test]
fn typing_in_search_bar_updates_query_and_matches() {
    let mut app = test_app("apple banana apple cherry");
    crate::search::open_find(&mut app, false);

    type_search_str(&mut app, "apple");

    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.find_query, "apple");
    assert_eq!(state.search_bar.matches().len(), 2);

    // Backspace removes 'e'
    press_search_key(&mut app, NamedKey::Backspace);
    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.find_query, "appl");
    assert_eq!(state.search_bar.matches().len(), 2);
}

#[test]
fn cycling_matches_forward_and_backward() {
    let mut app = test_app("foo bar foo baz foo");
    crate::search::open_find(&mut app, false);
    type_search_str(&mut app, "foo");

    // Initially at match 0 (0..3)
    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.search_bar.current_index(), Some(0));
    assert_eq!(app.cursors.primary().char_range(), 0..3);

    // Enter moves to match 1 (8..11)
    press_search_key(&mut app, NamedKey::Enter);
    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.search_bar.current_index(), Some(1));
    assert_eq!(app.cursors.primary().char_range(), 8..11);

    // Enter moves to match 2 (16..19)
    press_search_key(&mut app, NamedKey::Enter);
    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.search_bar.current_index(), Some(2));
    assert_eq!(app.cursors.primary().char_range(), 16..19);

    // Enter wraps around to match 0 (0..3)
    press_search_key(&mut app, NamedKey::Enter);
    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.search_bar.current_index(), Some(0));
    assert_eq!(app.cursors.primary().char_range(), 0..3);

    // Shift+Enter cycles backward to match 2 (16..19)
    app.modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };
    press_search_key(&mut app, NamedKey::Enter);
    app.modifiers = Modifiers::default();
    let state = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(state.search_bar.current_index(), Some(2));
    assert_eq!(app.cursors.primary().char_range(), 16..19);
}

#[test]
fn find_all_creates_multicursor_and_dismisses() {
    let mut app = test_app("foo bar foo baz foo");
    crate::search::open_find(&mut app, false);
    type_search_str(&mut app, "foo");

    // Alt+Enter triggers find_all
    app.modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    press_search_key(&mut app, NamedKey::Enter);
    app.modifiers = Modifiers::default();

    assert!(app.ui.search_bar.is_none());
    assert_eq!(app.chrome.focus.current(), FocusTarget::Editor);
    assert_eq!(app.cursors.count(), 3);
    assert_eq!(app.cursors.cursors()[0].char_range(), 0..3);
    assert_eq!(app.cursors.cursors()[1].char_range(), 8..11);
    assert_eq!(app.cursors.cursors()[2].char_range(), 16..19);
}

#[test]
fn replace_current_and_replace_all_flow() {
    let mut app = test_app("foo bar foo baz foo");
    crate::search::open_find(&mut app, true);
    type_search_str(&mut app, "foo");

    // Tab to Replace field
    press_search_key(&mut app, NamedKey::Tab);
    assert_eq!(
        app.ui.search_bar.as_ref().unwrap().focused_field,
        crate::search::SearchField::Replace
    );

    type_search_str(&mut app, "qux");
    assert_eq!(app.ui.search_bar.as_ref().unwrap().replace_text, "qux");

    // Enter replaces current match (first "foo" -> "qux")
    press_search_key(&mut app, NamedKey::Enter);
    assert_eq!(app.buffer.text(), "qux bar foo baz foo");
    // Next match should now be active
    assert_eq!(app.cursors.primary().char_range(), 8..11);

    // Alt+Enter in replace field triggers replace_all
    app.modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    press_search_key(&mut app, NamedKey::Enter);
    app.modifiers = Modifiers::default();
    assert_eq!(app.buffer.text(), "qux bar qux baz qux");
}

#[test]
fn in_selection_scoping_filters_matches() {
    let mut app = test_app("aaa bbb aaa bbb aaa");
    // Select range 4..15 ("bbb aaa bbb")
    let range = 4..15;
    crate::search::open_find(&mut app, false);
    type_search_str(&mut app, "aaa");

    // Without in_selection: 3 matches
    assert_eq!(
        app.ui
            .search_bar
            .as_ref()
            .unwrap()
            .search_bar
            .matches()
            .len(),
        3
    );

    // Toggle in_selection
    app.ui
        .search_bar
        .as_mut()
        .unwrap()
        .toggle_in_selection(&app.buffer, Some(range));

    // Scoped matches: only 1 (index 8..11)
    assert_eq!(
        app.ui
            .search_bar
            .as_ref()
            .unwrap()
            .search_bar
            .matches()
            .len(),
        1
    );
    assert_eq!(
        app.ui.search_bar.as_ref().unwrap().search_bar.matches()[0].char_range(),
        8..11
    );
}

#[test]
fn toggle_options_shortcuts() {
    let mut app = test_app("Foo foo food");
    crate::search::open_find(&mut app, false);
    type_search_str(&mut app, "foo");

    // Default: case-insensitive substring -> matches "Foo", "foo", "foo" in "food" = 3
    assert_eq!(
        app.ui
            .search_bar
            .as_ref()
            .unwrap()
            .search_bar
            .matches()
            .len(),
        3
    );

    // Toggle case sensitive
    app.modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    send_search_key(
        &mut app,
        &KeyEvent {
            key: Key::Character("c".into()),
            text: Some("c".into()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );
    app.modifiers = Modifiers::default();
    assert!(app.ui.search_bar.as_ref().unwrap().case_sensitive);
    // Matches "foo" and "foo" in "food" = 2
    assert_eq!(
        app.ui
            .search_bar
            .as_ref()
            .unwrap()
            .search_bar
            .matches()
            .len(),
        2
    );

    // Toggle whole word
    app.modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    send_search_key(
        &mut app,
        &KeyEvent {
            key: Key::Character("w".into()),
            text: Some("w".into()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );
    app.modifiers = Modifiers::default();
    assert!(app.ui.search_bar.as_ref().unwrap().whole_word);
    // Matches only standalone "foo" = 1
    assert_eq!(
        app.ui
            .search_bar
            .as_ref()
            .unwrap()
            .search_bar
            .matches()
            .len(),
        1
    );
}

#[test]
fn escape_dismisses_search_bar_and_restores_editor_focus() {
    let mut app = test_app("test content");
    crate::search::open_find(&mut app, false);
    assert_eq!(app.chrome.focus.current(), FocusTarget::Search);

    press_search_key(&mut app, NamedKey::Escape);

    assert!(app.ui.search_bar.is_none());
    assert_eq!(app.chrome.focus.current(), FocusTarget::Editor);
}

#[test]
fn search_click_interaction_dismisses_or_navigates() {
    let mut app = test_app("hello world hello");
    crate::search::open_find(&mut app, true);
    type_search_str(&mut app, "hello");

    if let Some(s) = app.ui.search_bar.as_mut() {
        s.layout.bar_rect = [0.0, 500.0, 800.0, 64.0];
        s.layout.close_btn_rect = [770.0, 505.0, 20.0, 20.0];
        s.layout.next_btn_rect = [600.0, 505.0, 30.0, 20.0];
    }

    // Click next button
    let clicked_next = crate::search::handle_search_click(&mut app, 610.0, 515.0);
    assert!(clicked_next);
    assert_eq!(
        app.ui
            .search_bar
            .as_ref()
            .unwrap()
            .search_bar
            .current_index(),
        Some(1)
    );

    // Click close button
    let clicked_close = crate::search::handle_search_click(&mut app, 775.0, 515.0);
    assert!(clicked_close);
    assert!(app.ui.search_bar.is_none());
    assert_eq!(app.chrome.focus.current(), FocusTarget::Editor);
}
