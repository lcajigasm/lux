//! Save All and tab chords through the keyboard, including a physical bracket.

use super::*;
use lux_platform::dialog::SaveDecision;
use lux_platform::event::{KeyEvent, PhysicalKey};
use lux_platform::{
    Clipboard, ElementState, FileDialogOptions, FileDialogs, Key, Modifiers, PlatformContext,
    PlatformError,
};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

struct Dialogs {
    threads: Mutex<Vec<std::thread::ThreadId>>,
}

impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
}

struct Context {
    dialogs: Arc<Dialogs>,
    events: mpsc::Receiver<()>,
    sender: mpsc::Sender<()>,
}

impl Context {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            dialogs: Arc::new(Dialogs {
                threads: Mutex::new(vec![]),
            }),
            events,
            sender,
        }
    }

    fn complete(&self, app: &mut App) {
        self.events
            .recv_timeout(Duration::from_secs(10))
            .expect("worker must notify completion");
        app.finish_document_operation(self);
    }
}

impl Clipboard for Context {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl PlatformContext for Context {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        let sender = self.sender.clone();
        Arc::new(move || {
            let _ = sender.send(());
        })
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        self.dialogs.clone()
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless".into()))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        self.dialogs.as_ref()
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

fn host_command(shift: bool, alt: bool) -> Modifiers {
    Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        alt,
    }
}

fn press(
    app: &mut App,
    ctx: &Context,
    key: Key,
    text: Option<&str>,
    physical: Option<PhysicalKey>,
) {
    app.handle_keyboard_event(
        ctx,
        &KeyEvent {
            key,
            text: text.map(str::to_owned),
            state: ElementState::Pressed,
            repeat: false,
            physical,
        },
    );
}

fn type_char(app: &mut App, ch: &str) {
    app.handle_key(&KeyEvent {
        key: Key::Character(ch.into()),
        text: Some(ch.into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    });
}

#[test]
fn option_command_s_saves_every_dirty_named_file() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("one.txt");
    let second = dir.path().join("two.txt");
    std::fs::write(&first, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    let ctx = Context::new();
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(first.clone())));
    ctx.complete(&mut app);
    let first_tab = app.documents.active_tab.unwrap();
    app.request_document(&ctx, Request::Open(Some(second.clone())));
    ctx.complete(&mut app);
    press(
        &mut app,
        &ctx,
        Key::Named(lux_platform::NamedKey::End),
        None,
        None,
    );
    type_char(&mut app, "!");
    app.switch_to_tab(first_tab);
    press(
        &mut app,
        &ctx,
        Key::Named(lux_platform::NamedKey::End),
        None,
        None,
    );
    type_char(&mut app, "?");
    app.modifiers = host_command(false, true);
    press(
        &mut app,
        &ctx,
        Key::Character("ß".into()),
        Some("ß"),
        Some(PhysicalKey::Letter('s')),
    );
    app.modifiers = Modifiers::default();
    ctx.complete(&mut app);
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "one?");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "two!");
    assert!(!app.document_dirty());
    assert!(
        app.documents
            .entries
            .iter()
            .all(|entry| entry.path.is_none() || !entry.is_dirty())
    );
}

#[test]
fn shift_command_t_reopens_the_closed_tab() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("one.txt");
    let second = dir.path().join("two.txt");
    std::fs::write(&first, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    let ctx = Context::new();
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(first)));
    ctx.complete(&mut app);
    let kept = app.documents.active_tab.unwrap();
    app.request_document(&ctx, Request::Open(Some(second)));
    ctx.complete(&mut app);
    app.request_document(&ctx, Request::Close);
    ctx.complete(&mut app);
    assert_eq!(app.documents.tabs.tabs().len(), 1);
    assert_eq!(app.buffer.text(), "one");
    app.modifiers = host_command(true, false);
    press(
        &mut app,
        &ctx,
        Key::Character("t".into()),
        Some("t"),
        Some(PhysicalKey::Letter('t')),
    );
    app.modifiers = Modifiers::default();
    assert_eq!(app.documents.tabs.tabs().len(), 2);
    assert_eq!(app.buffer.text(), "two");
    assert_ne!(app.documents.active_tab, Some(kept));
}

#[test]
fn shift_command_brackets_cycle_tabs_without_inserting_a_glyph() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("one.txt");
    let second = dir.path().join("two.txt");
    std::fs::write(&first, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    let ctx = Context::new();
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(first)));
    ctx.complete(&mut app);
    let first_tab = app.documents.active_tab.unwrap();
    app.request_document(&ctx, Request::Open(Some(second)));
    ctx.complete(&mut app);
    let second_tab = app.documents.active_tab.unwrap();
    if cfg!(target_os = "macos") {
        app.modifiers = host_command(true, false);
        press(
            &mut app,
            &ctx,
            Key::Character("`".into()),
            Some("`"),
            Some(PhysicalKey::BracketLeft),
        );
    } else {
        app.modifiers = host_command(false, false);
        press(
            &mut app,
            &ctx,
            Key::Named(lux_platform::NamedKey::PageUp),
            None,
            None,
        );
    }
    assert_eq!(app.documents.active_tab, Some(first_tab));
    assert_eq!(app.buffer.text(), "one");
    if cfg!(target_os = "macos") {
        press(
            &mut app,
            &ctx,
            Key::Character("´".into()),
            Some("´"),
            Some(PhysicalKey::BracketRight),
        );
    } else {
        press(
            &mut app,
            &ctx,
            Key::Named(lux_platform::NamedKey::PageDown),
            None,
            None,
        );
    }
    app.modifiers = Modifiers::default();
    assert_eq!(app.documents.active_tab, Some(second_tab));
    assert_eq!(app.buffer.text(), "two");
}

#[test]
fn file_menu_save_all_writes_every_dirty_named_file() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("one.txt");
    let second = dir.path().join("two.txt");
    std::fs::write(&first, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    let ctx = Context::new();
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(first.clone())));
    ctx.complete(&mut app);
    let first_tab = app.documents.active_tab.unwrap();
    app.request_document(&ctx, Request::Open(Some(second.clone())));
    ctx.complete(&mut app);
    press(
        &mut app,
        &ctx,
        Key::Named(lux_platform::NamedKey::End),
        None,
        None,
    );
    type_char(&mut app, "!");
    app.switch_to_tab(first_tab);
    press(
        &mut app,
        &ctx,
        Key::Named(lux_platform::NamedKey::End),
        None,
        None,
    );
    type_char(&mut app, "?");
    crate::menu::clear_claim();
    crate::menu::handle_menu_action(&mut app, &ctx, crate::menu::MenuAction::SaveAll);
    ctx.complete(&mut app);
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "one?");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "two!");
}

#[test]
fn file_menu_reopen_closed_tab_restores_the_buffer() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("one.txt");
    let second = dir.path().join("two.txt");
    std::fs::write(&first, "one").unwrap();
    std::fs::write(&second, "two").unwrap();
    let ctx = Context::new();
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(first)));
    ctx.complete(&mut app);
    app.request_document(&ctx, Request::Open(Some(second)));
    ctx.complete(&mut app);
    app.request_document(&ctx, Request::Close);
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "one");
    crate::menu::clear_claim();
    crate::menu::handle_menu_action(&mut app, &ctx, crate::menu::MenuAction::ReopenClosedTab);
    assert_eq!(app.documents.tabs.tabs().len(), 2);
    assert_eq!(app.buffer.text(), "two");
}

#[test]
fn file_menu_open_recent_opens_the_stored_path() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("recent-target.txt");
    std::fs::write(&file, "recent-body").unwrap();
    let _guard = crate::menu::recent::begin_test(&dir.path().join("recent.json"));
    crate::menu::recent::note(Some(&file));
    crate::menu::recent::sync();
    let ctx = Context::new();
    let mut app = app();
    crate::menu::file::open_recent(&mut app, &ctx, 0);
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "recent-body");
    assert_eq!(app.documents.path.as_deref(), Some(file.as_path()));
}
