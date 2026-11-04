use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use lux_platform::dialog::SaveDecision;
use lux_platform::{
    Clipboard, FileDialogOptions, FileDialogs, MouseButton, PlatformContext, PlatformError,
    PlatformWindow, WindowAttributes,
};
use lux_platform::{ElementState, Key, KeyEvent, NamedKey};
use std::sync::mpsc::{self, Receiver, Sender};

use super::*;

struct Dialogs;

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
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
}

struct Context {
    dialogs: Arc<Dialogs>,
    events: Receiver<()>,
    sender: Sender<()>,
    exited: Cell<bool>,
}

impl Context {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            dialogs: Arc::new(Dialogs),
            events,
            sender,
            exited: Cell::new(false),
        }
    }

    fn complete(&self, app: &mut crate::app::App) {
        self.events
            .recv_timeout(Duration::from_secs(10))
            .expect("worker must notify completion");
        app.finish_document_operation(self);
    }
}

impl Clipboard for Context {
    fn read_text(&self) -> Result<String, PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
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
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless sidebar test".into()))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        self.dialogs.as_ref()
    }
    fn exit(&self) {
        self.exited.set(true);
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn app() -> crate::app::App {
    crate::app::App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

#[test]
fn clicking_a_file_row_opens_it_and_focuses_the_tree() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("notes")).unwrap();
    std::fs::write(dir.path().join("notes").join("readme.txt"), "nested\n").unwrap();
    std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
    let ctx = Context::new();
    let notes_y = 32.0 + 28.0 + 11.0;
    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        20.0,
        notes_y,
        MouseButton::Left
    ));
    assert!(app.chrome.focus.current() == lux_ui::focus::FocusTarget::FileTree);
    let names: Vec<_> = listed_rows(app.ui.sidebar.as_mut().unwrap().tree.as_mut().unwrap())
        .iter()
        .map(|row| row.name.clone())
        .collect();
    assert_eq!(names, vec!["notes", "readme.txt", "a.txt"]);

    let file_y = 32.0 + 28.0 + 22.0 + 11.0;
    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        30.0,
        file_y,
        MouseButton::Left
    ));
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "nested\n");
    assert_eq!(
        app.documents.tabs.active().map(|tab| tab.title.as_str()),
        Some("readme.txt")
    );
}

#[test]
fn arrow_down_selects_the_first_listed_row() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("b.txt"), "").unwrap();
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
    app.chrome
        .focus
        .set_focus(lux_ui::focus::FocusTarget::FileTree);
    let ctx = Context::new();
    let event = KeyEvent {
        key: Key::Named(NamedKey::ArrowDown),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(&mut app, &ctx, &event));
    let selected = app.ui.sidebar.as_ref().unwrap().selected_node;
    let name = listed_rows(app.ui.sidebar.as_mut().unwrap().tree.as_mut().unwrap())
        .into_iter()
        .find(|row| Some(row.id) == selected)
        .map(|row| row.name);
    assert_eq!(name.as_deref(), Some("a.txt"));
}
