//! Headless desktop session used by CI on Linux, Windows, and macOS.
//!
//! Drives the same `handle_keyboard_event` path as the window loop: open a
//! real file, type, save, open the command palette, split, and find. It does
//! not create a GPU surface, an IME candidate window, or a native file dialog.

use std::sync::{Arc, mpsc};
use std::time::Duration;

use accesskit::Role;
use lux_platform::dialog::SaveDecision;
use lux_platform::event::KeyEvent;
use lux_platform::{
    Clipboard, ElementState, FileDialogOptions, FileDialogs, Key, Modifiers, NamedKey,
    PlatformContext, PlatformError, PlatformWindow, WindowAttributes,
};

use crate::app::App;
use crate::document::Request;

struct Dialogs {
    open: Option<std::path::PathBuf>,
}

impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        self.open.clone()
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<std::path::PathBuf> {
        self.open.clone().into_iter().collect()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
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
    fn new(open: Option<std::path::PathBuf>) -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            dialogs: Arc::new(Dialogs { open }),
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
            sender.send(()).ok();
        })
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        self.dialogs.clone()
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window(
            "headless desktop pass does not open a window".into(),
        ))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        self.dialogs.as_ref()
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {
        panic!("desktop pass must not poll");
    }
}

fn press(app: &mut App, ctx: &Context, key: Key, text: Option<&str>) {
    app.handle_keyboard_event(
        ctx,
        &KeyEvent {
            key,
            text: text.map(str::to_owned),
            state: ElementState::Pressed,
            repeat: false,
        },
    );
}

fn command(app: &mut App, shift: bool) {
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        alt: false,
    };
}

fn type_text(app: &mut App, ctx: &Context, text: &str) {
    app.modifiers = Modifiers::default();
    for ch in text.chars() {
        let s = ch.to_string();
        press(app, ctx, Key::Character(s.clone()), Some(&s));
    }
}

#[test]
fn headless_session_opens_types_saves_palette_split_and_find() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.txt");
    std::fs::write(&path, "alpha beta\n").unwrap();
    let ctx = Context::new(Some(path.clone()));
    let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());

    app.request_document(&ctx, Request::Open(Some(path.clone())));
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "alpha beta\n");

    type_text(&mut app, &ctx, "gamma ");
    assert_eq!(app.buffer.text(), "gamma alpha beta\n");
    assert!(app.document_dirty());

    command(&mut app, false);
    press(&mut app, &ctx, Key::Character("s".into()), Some("s"));
    ctx.complete(&mut app);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "gamma alpha beta\n"
    );
    assert!(!app.document_dirty());

    command(&mut app, true);
    press(&mut app, &ctx, Key::Character("p".into()), Some("p"));
    assert!(app.ui.command_palette.is_some());
    type_text(&mut app, &ctx, "layout");
    let palette = app.ui.command_palette.as_ref().unwrap();
    let mut palette_ctx = lux_ui::command_palette::Context::new();
    palette_ctx.set("editorFocus", true);
    let results = palette.results(&palette_ctx);
    assert!(
        results
            .iter()
            .any(|entry| entry.command.id == "view.layout_columns_2"),
        "palette query should list the two-column layout"
    );
    app.modifiers = Modifiers::default();
    press(&mut app, &ctx, Key::Named(NamedKey::Escape), None);
    assert!(app.ui.command_palette.is_none());

    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        alt: true,
        shift: false,
    };
    press(&mut app, &ctx, Key::Character("2".into()), None);
    assert!(app.ui.split.secondary.is_some());

    command(&mut app, false);
    press(&mut app, &ctx, Key::Character("f".into()), Some("f"));
    assert!(app.ui.search_bar.is_some());
    type_text(&mut app, &ctx, "beta");
    let search = app.ui.search_bar.as_ref().unwrap();
    assert_eq!(search.find_query, "beta");
    assert!(!search.search_bar.matches().is_empty());

    let update = app.build_accessibility_tree();
    let roles: Vec<Role> = update.nodes.iter().map(|(_, node)| node.role()).collect();
    assert!(roles.contains(&Role::Window));
    assert!(roles.contains(&Role::MultilineTextInput));
    assert!(roles.contains(&Role::TextRun));
    assert!(roles.contains(&Role::Search));
    assert!(roles.contains(&Role::TextInput));
    let editor_text: String = update
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::TextRun)
        .filter_map(|(_, node)| node.value())
        .collect();
    assert!(editor_text.contains("gamma"), "{editor_text}");
    assert!(editor_text.contains("beta"), "{editor_text}");
    let query = update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::TextInput)
        .and_then(|(_, node)| node.value());
    assert_eq!(query, Some("beta"));
}
