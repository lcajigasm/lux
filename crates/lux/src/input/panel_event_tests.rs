//! Keyboard-event coverage for panel IME, search toggles, and physical chords.

use lux_platform::event::KeyEvent;
use lux_platform::{
    Clipboard, ElementState, FileDialogOptions, FileDialogs, Ime, Key, Modifiers, PhysicalKey,
    PlatformContext, PlatformError, PlatformWindow, WindowAttributes,
};

use crate::app::App;

fn key(text: &str, physical: Option<PhysicalKey>, state: ElementState) -> KeyEvent {
    KeyEvent {
        key: Key::Character(text.into()),
        text: Some(text.into()),
        state,
        repeat: false,
        physical,
    }
}

fn command(app: &mut App, alt: bool) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        alt,
        shift: false,
    };
}

fn field_text(app: &App) -> String {
    if let Some(panel) = &app.ui.command_palette {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.language_picker {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.theme_picker {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.goto_anything {
        return panel.query.as_str().to_string();
    }
    if let Some(panel) = &app.ui.search_bar {
        return panel.find_query.as_str().to_string();
    }
    String::new()
}

fn open_theme(app: &mut App) {
    app.ui.theme_picker = Some(crate::theme_picker::ThemePickerState::new("Monokai"));
}

fn commit_dead_key(app: &mut App) {
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"doc\n").unwrap();
    app.modifiers = Modifiers::default();
    crate::ime_hold::observe(app, Ime::Commit("á".into()));
    app.handle_keyboard_event(
        &super::Ctx,
        &key("a", Some(PhysicalKey::Letter('a')), ElementState::Pressed),
    );
    app.handle_keyboard_event(
        &super::Ctx,
        &key("a", Some(PhysicalKey::Letter('a')), ElementState::Released),
    );
}

#[test]
fn a4_dead_key_commits_into_the_open_panel() {
    let ctx_open: [fn(&mut App); 5] = [
        |app| app.open_command_palette(""),
        |app| app.open_goto_anything(""),
        |app| crate::search::open_find(app, false),
        |app| app.open_language_picker(),
        open_theme,
    ];
    for open in ctx_open {
        let mut app = super::app();
        open(&mut app);
        commit_dead_key(&mut app);
        assert_eq!(
            field_text(&app),
            "á",
            "dead key must land once in the panel"
        );
        assert_eq!(app.buffer.text(), "doc\n");
    }
}

#[test]
fn a5_cedilla_is_search_text_through_the_keyboard_event() {
    let mut app = super::app();
    crate::search::open_find(&mut app, false);
    app.modifiers = Modifiers::default();
    app.handle_keyboard_event(
        &super::Ctx,
        &key("ç", Some(PhysicalKey::Slash), ElementState::Pressed),
    );
    let search = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(search.find_query.as_str(), "ç");
    assert!(!search.case_sensitive);
    assert!(!search.regex);
    assert!(!search.whole_word);
}

#[test]
fn a6_search_toggles_once_on_press_not_release() {
    let mut app = super::app();
    crate::search::open_find(&mut app, false);
    super::command_alt(&mut app);
    let press = key("ç", Some(PhysicalKey::Letter('c')), ElementState::Pressed);
    let release = key("ç", Some(PhysicalKey::Letter('c')), ElementState::Released);
    app.handle_keyboard_event(&super::Ctx, &press);
    assert!(app.ui.search_bar.as_ref().unwrap().case_sensitive);
    app.handle_keyboard_event(&super::Ctx, &release);
    assert!(app.ui.search_bar.as_ref().unwrap().case_sensitive);
    app.handle_keyboard_event(&super::Ctx, &press);
    assert!(!app.ui.search_bar.as_ref().unwrap().case_sensitive);
    assert_eq!(app.ui.search_bar.as_ref().unwrap().find_query.as_str(), "");
}

#[test]
fn a7_panel_field_edits_and_global_shortcuts() {
    let mut app = super::app();
    open_theme(&mut app);
    command(&mut app, false);
    app.handle_keyboard_event(
        &super::Ctx,
        &key("v", Some(PhysicalKey::Letter('v')), ElementState::Pressed),
    );
    assert_eq!(field_text(&app), "pasted");
    assert_eq!(app.buffer.text(), "");
    app.handle_keyboard_event(
        &super::Ctx,
        &key("a", Some(PhysicalKey::Letter('a')), ElementState::Pressed),
    );
    app.handle_keyboard_event(
        &super::Ctx,
        &key("x", Some(PhysicalKey::Letter('x')), ElementState::Pressed),
    );
    assert_eq!(field_text(&app), "");
    app.handle_keyboard_event(
        &super::Ctx,
        &key("z", Some(PhysicalKey::Letter('z')), ElementState::Pressed),
    );
    assert_eq!(field_text(&app), "pasted");

    app.handle_keyboard_event(
        &super::Ctx,
        &key("p", Some(PhysicalKey::Letter('p')), ElementState::Pressed),
    );
    assert!(app.ui.goto_anything.is_some());
    assert_eq!(
        app.ui.theme_picker.as_ref().unwrap().query.as_str(),
        "pasted"
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.txt");
    std::fs::write(&path, "old\n").unwrap();
    let mut app = super::app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"new\n").unwrap();
    app.documents.path = Some(path.clone());
    open_theme(&mut app);
    let io = Io::new();
    command(&mut app, false);
    io.press(
        &mut app,
        "s",
        Some(PhysicalKey::Letter('s')),
        ElementState::Pressed,
    );
    io.complete(&mut app);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
    assert_eq!(field_text(&app), "");

    let mut app = super::app();
    let second = app.documents.tabs.open("two");
    app.documents
        .entries
        .push(crate::document::DocumentEntry::new(
            second,
            None,
            lux_core::buffer::Buffer::from_bytes(b"two\n").unwrap(),
        ));
    app.documents.active_tab = Some(second);
    open_theme(&mut app);
    let open_tabs = app.documents.tabs.tabs().len();
    command(&mut app, false);
    let io = Io::new();
    io.press(
        &mut app,
        "w",
        Some(PhysicalKey::Letter('w')),
        ElementState::Pressed,
    );
    io.complete(&mut app);
    assert_eq!(app.documents.tabs.tabs().len(), open_tabs - 1);
    assert_eq!(field_text(&app), "");
}

#[test]
fn a8_option_chords_follow_the_physical_key() {
    let mut app = super::app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"x").unwrap();
    app.documents.path = Some(std::path::PathBuf::from("note.rs"));
    command(&mut app, false);
    app.handle_keyboard_event(
        &super::Ctx,
        &key("ç", Some(PhysicalKey::Slash), ElementState::Pressed),
    );
    assert!(
        app.buffer.text().starts_with("//"),
        "cmd+/ on a Spanish glyph comments, got {:?}",
        app.buffer.text()
    );

    let mut app = super::app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"x").unwrap();
    command(&mut app, false);
    app.handle_keyboard_event(
        &super::Ctx,
        &key("ç", Some(PhysicalKey::BracketRight), ElementState::Pressed),
    );
    assert_eq!(app.buffer.text(), "    x");
    app.handle_keyboard_event(
        &super::Ctx,
        &key("ç", Some(PhysicalKey::BracketLeft), ElementState::Pressed),
    );
    assert_eq!(app.buffer.text(), "x");

    let mut app = super::app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"a\n    b\n").unwrap();
    open_theme(&mut app);
    if cfg!(target_os = "macos") {
        command(&mut app, true);
        app.handle_keyboard_event(
            &super::Ctx,
            &key("ƒ", Some(PhysicalKey::Letter('f')), ElementState::Pressed),
        );
        assert!(app.ui.search_bar.as_ref().unwrap().replace_mode);
        command(&mut app, true);
        app.ui.search_bar = None;
        app.handle_keyboard_event(
            &super::Ctx,
            &key("`", Some(PhysicalKey::BracketLeft), ElementState::Pressed),
        );
        assert!(!app.fold_state.folded_ranges().is_empty());
        app.handle_keyboard_event(
            &super::Ctx,
            &key("`", Some(PhysicalKey::BracketRight), ElementState::Pressed),
        );
        assert!(app.fold_state.folded_ranges().is_empty());
        app.handle_keyboard_event(
            &super::Ctx,
            &key("ç", Some(PhysicalKey::Letter('c')), ElementState::Pressed),
        );
        assert!(!app.buffer.text().contains('ç'));
    } else {
        app.modifiers = Modifiers {
            meta: false,
            control: true,
            alt: false,
            shift: false,
        };
        app.handle_keyboard_event(
            &super::Ctx,
            &key("ç", Some(PhysicalKey::Letter('h')), ElementState::Pressed),
        );
        assert!(app.ui.search_bar.as_ref().unwrap().replace_mode);
        app.ui.search_bar = None;
        app.modifiers.shift = true;
        app.handle_keyboard_event(
            &super::Ctx,
            &key("`", Some(PhysicalKey::BracketLeft), ElementState::Pressed),
        );
        assert!(!app.fold_state.folded_ranges().is_empty());
    }
    assert_eq!(field_text(&app), "");
    assert!(!app.buffer.text().contains('ƒ'));
    assert!(!app.buffer.text().contains('`'));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("all.txt");
    std::fs::write(&path, "old\n").unwrap();
    let mut app = super::app();
    let tab = app.documents.tabs.open("all.txt");
    app.documents
        .entries
        .push(crate::document::DocumentEntry::new(
            tab,
            Some(path.clone()),
            lux_core::buffer::Buffer::from_bytes(b"old\n").unwrap(),
        ));
    app.documents.active_tab = Some(tab);
    app.documents.path = Some(path.clone());
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"fresh\n").unwrap();
    app.documents.saved_version = 1;
    open_theme(&mut app);
    command(&mut app, true);
    let io = Io::new();
    io.press(
        &mut app,
        "ß",
        Some(PhysicalKey::Letter('s')),
        ElementState::Pressed,
    );
    io.complete(&mut app);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh\n");
    assert_eq!(field_text(&app), "");
}

#[test]
fn i1_keyboard_event_press_release_ime_cedilla_glyph_and_shortcut() {
    let mut app = super::app();
    app.handle_keyboard_event(
        &super::Ctx,
        &key("b", Some(PhysicalKey::Letter('b')), ElementState::Pressed),
    );
    app.handle_keyboard_event(
        &super::Ctx,
        &key("b", Some(PhysicalKey::Letter('b')), ElementState::Released),
    );
    assert_eq!(app.buffer.text(), "b");

    command(&mut app, false);
    app.handle_keyboard_event(
        &super::Ctx,
        &key("p", Some(PhysicalKey::Letter('p')), ElementState::Pressed),
    );
    app.handle_keyboard_event(
        &super::Ctx,
        &key("p", Some(PhysicalKey::Letter('p')), ElementState::Released),
    );
    assert!(app.ui.goto_anything.is_some());
    assert_eq!(app.buffer.text(), "b");

    let mut app = super::app();
    open_theme(&mut app);
    commit_dead_key(&mut app);
    assert_eq!(field_text(&app), "á");
    assert_eq!(app.buffer.text(), "doc\n");
    command(&mut app, false);
    app.handle_keyboard_event(
        &super::Ctx,
        &key("p", Some(PhysicalKey::Letter('p')), ElementState::Pressed),
    );
    assert!(app.ui.goto_anything.is_some());
    assert_eq!(app.ui.theme_picker.as_ref().unwrap().query.as_str(), "á");

    let mut app = super::app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"x").unwrap();
    open_theme(&mut app);
    if cfg!(target_os = "macos") {
        command(&mut app, true);
        app.handle_keyboard_event(
            &super::Ctx,
            &key("ƒ", Some(PhysicalKey::Letter('f')), ElementState::Pressed),
        );
    } else {
        command(&mut app, false);
        app.handle_keyboard_event(
            &super::Ctx,
            &key("ç", Some(PhysicalKey::Letter('h')), ElementState::Pressed),
        );
    }
    assert!(app.ui.search_bar.as_ref().unwrap().replace_mode);
    assert_eq!(app.ui.theme_picker.as_ref().unwrap().query.as_str(), "");
    assert!(!app.buffer.text().contains('ƒ'));

    let mut app = super::app();
    crate::search::open_find(&mut app, false);
    app.modifiers = Modifiers::default();
    app.handle_keyboard_event(
        &super::Ctx,
        &key("ç", Some(PhysicalKey::Backslash), ElementState::Pressed),
    );
    assert_eq!(app.ui.search_bar.as_ref().unwrap().find_query.as_str(), "ç");
}

struct Io {
    events: std::sync::mpsc::Receiver<()>,
    sender: std::sync::mpsc::Sender<()>,
}

impl Io {
    fn new() -> Self {
        let (sender, events) = std::sync::mpsc::channel();
        Self { events, sender }
    }

    fn press(&self, app: &mut App, text: &str, physical: Option<PhysicalKey>, state: ElementState) {
        app.handle_keyboard_event(self, &key(text, physical, state));
    }

    fn complete(&self, app: &mut App) {
        self.events
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("document worker notifies");
        app.finish_document_operation(self);
    }
}

impl Clipboard for Io {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok("pasted".into())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}

struct IoDialogs;

impl FileDialogs for IoDialogs {
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

impl PlatformContext for Io {
    fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        let sender = self.sender.clone();
        std::sync::Arc::new(move || {
            sender.send(()).ok();
        })
    }
    fn background_dialogs(&self) -> std::sync::Arc<dyn FileDialogs> {
        std::sync::Arc::new(IoDialogs)
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        unimplemented!()
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &IoDialogs
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {}
}
