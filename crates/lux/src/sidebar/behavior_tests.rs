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

/// `trash` is the answer to the Move to Trash confirmation.
pub(super) struct Dialogs {
    trash: bool,
    /// The thread that showed the trash confirmation.
    pub(super) asked_on: std::sync::Mutex<Option<std::thread::ThreadId>>,
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
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
    fn confirm_trash(&self, _: &str) -> bool {
        *self.asked_on.lock().unwrap() = Some(std::thread::current().id());
        self.trash
    }
}

pub(super) struct Context {
    pub(super) dialogs: Arc<Dialogs>,
    events: Receiver<()>,
    sender: Sender<()>,
    exited: Cell<bool>,
}

impl Context {
    pub(super) fn new() -> Self {
        Self::answering_trash(true)
    }

    pub(super) fn answering_trash(trash: bool) -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            dialogs: Arc::new(Dialogs {
                trash,
                asked_on: std::sync::Mutex::new(None),
            }),
            events,
            sender,
            exited: Cell::new(false),
        }
    }

    /// Waits until the document worker has finished, then applies it.
    pub(super) fn complete(&self, app: &mut crate::app::App) {
        while crate::document::open_queue::busy(app) {
            self.next_wake();
            app.finish_document_operation(self);
        }
    }

    /// Registers this context's waker as the app's, as `resumed` does,
    /// except for the process-wide session debouncer: its write wake
    /// (from any test's edits, ~750 ms later) would reach whichever test
    /// registered last and look like that test's job finishing.
    pub(super) fn attach(&self, app: &mut crate::app::App) {
        app.ui
            .session
            .register_waker_except_debounce(self.background_waker());
    }

    /// Runs the background event on each wake, as the platform loop does,
    /// until every sidebar job and the document worker have finished. A
    /// wake from anything else (the folder watcher) does not end the wait.
    pub(super) fn wake(&self, app: &mut crate::app::App) {
        loop {
            self.next_wake();
            crate::sidebar::worker::poll(app, self);
            app.finish_document_operation(self);
            if app.ui.session.file_ops.in_flight() == 0 && !crate::document::open_queue::busy(app) {
                return;
            }
        }
    }

    fn next_wake(&self) {
        self.events
            .recv_timeout(Duration::from_secs(10))
            .expect("worker must wake the loop");
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

pub(super) fn app() -> crate::app::App {
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
    ctx.attach(&mut app);
    let notes_y = 32.0 + 28.0 + 11.0;
    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        20.0,
        notes_y,
        MouseButton::Left
    ));
    drag::release(&mut app, &ctx);
    assert!(app.chrome.focus.current() == lux_ui::focus::FocusTarget::FileTree);
    assert_eq!(
        row_names(&mut app),
        vec!["0:notes", "0:a.txt"],
        "read inline"
    );
    ctx.wake(&mut app);
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
    drag::release(&mut app, &ctx);
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

#[test]
fn the_keyboard_creates_renames_and_trashes_and_reveal_selects_the_open_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("notes")).unwrap();
    std::fs::write(dir.path().join("notes").join("readme.txt"), "nested\n").unwrap();
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
    let ctx = Context::new();
    ctx.attach(&mut app);
    app.documents.path = Some(dir.path().join("notes").join("readme.txt"));
    crate::palette::execute_command(&mut app, &ctx, "view.reveal_active_file");
    assert_eq!(row_names(&mut app), vec!["0:notes"], "read inline");
    ctx.wake(&mut app);
    let selected = app.ui.sidebar.as_ref().unwrap().selected_node;
    let name = listed_rows(app.ui.sidebar.as_mut().unwrap().tree.as_mut().unwrap())
        .into_iter()
        .find(|row| Some(row.id) == selected)
        .map(|row| row.name);
    assert_eq!(name.as_deref(), Some("readme.txt"));
    assert_eq!(
        app.chrome.focus.current(),
        lux_ui::focus::FocusTarget::FileTree
    );

    let down = KeyEvent {
        key: Key::Named(NamedKey::ArrowUp),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(&mut app, &ctx, &down));
    let new_file = KeyEvent {
        key: Key::Character("n".into()),
        text: Some("n".into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(&mut app, &ctx, &new_file));
    assert!(app.ui.sidebar.as_ref().unwrap().inline_input.is_some());
    for ch in ["b", ".", "t", "x", "t"] {
        let key = KeyEvent {
            key: Key::Character(ch.into()),
            text: Some(ch.into()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        };
        assert!(handle_sidebar_key(&mut app, &ctx, &key));
    }
    let enter = KeyEvent {
        key: Key::Named(NamedKey::Enter),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(&mut app, &ctx, &enter));
    // The create result, then the document open it starts.
    ctx.wake(&mut app);
    ctx.complete(&mut app);
    assert!(dir.path().join("notes").join("b.txt").is_file());
    let created = listed_rows(app.ui.sidebar.as_mut().unwrap().tree.as_mut().unwrap())
        .into_iter()
        .find(|row| row.name == "b.txt")
        .map(|row| row.id);
    assert!(created.is_some());
    assert_eq!(app.ui.sidebar.as_ref().unwrap().selected_node, created);
    assert_eq!(
        app.documents.path.as_deref(),
        Some(dir.path().join("notes").join("b.txt").as_path())
    );
    let rename = KeyEvent {
        key: Key::Named(NamedKey::F(2)),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(&mut app, &ctx, &rename));
    // The base name is selected, so one letter replaces it.
    let letter = KeyEvent {
        key: Key::Character("c".into()),
        text: Some("c".into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(&mut app, &ctx, &letter));
    assert!(handle_sidebar_key(&mut app, &ctx, &enter));
    ctx.wake(&mut app);
    assert!(dir.path().join("notes").join("c.txt").is_file());
    assert!(!dir.path().join("notes").join("b.txt").exists());
    // The open tab follows the rename.
    assert_eq!(
        app.documents.path.as_deref(),
        Some(dir.path().join("notes").join("c.txt").as_path())
    );

    let delete = KeyEvent {
        key: Key::Named(NamedKey::Delete),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(&mut app, &ctx, &delete));
    ctx.wake(&mut app);
    assert!(!dir.path().join("notes").join("c.txt").exists());
    assert!(
        !row_names(&mut app)
            .iter()
            .any(|name| name.ends_with("c.txt"))
    );
    assert!(
        app.documents
            .message
            .as_deref()
            .is_some_and(|message| message.contains("Moved to trash"))
    );
}

pub(super) fn row_center(app: &crate::app::App, index: usize) -> f32 {
    let scale = app.scale_factor;
    app.tab_bar_height()
        + SIDEBAR_HEADER_HEIGHT * scale
        + (index as f32 + 0.5) * SIDEBAR_ROW_HEIGHT * scale
}

pub(super) fn row_names(app: &mut crate::app::App) -> Vec<String> {
    listed_rows(app.ui.sidebar.as_mut().unwrap().tree.as_mut().unwrap())
        .into_iter()
        .map(|row| format!("{}:{}", row.depth, row.name))
        .collect()
}

#[test]
fn dragging_a_file_onto_a_folder_moves_it_and_retargets_the_open_tab() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("notes")).unwrap();
    let source = dir.path().join("move-me.txt");
    std::fs::write(&source, "payload\n").unwrap();
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
    let ctx = Context::new();
    ctx.attach(&mut app);

    let file_y = row_center(&app, 1);
    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        40.0,
        file_y,
        MouseButton::Left
    ));
    drag::release(&mut app, &ctx);
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "payload\n");
    assert_eq!(app.documents.path.as_deref(), Some(source.as_path()));

    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        40.0,
        file_y,
        MouseButton::Left
    ));
    let notes_y = row_center(&app, 0);
    assert!(drag::update(&mut app, 40.0, notes_y));
    drag::release(&mut app, &ctx);
    assert!(source.exists(), "moved on the UI thread");
    ctx.wake(&mut app);

    let moved = dir.path().join("notes").join("move-me.txt");
    assert!(moved.is_file(), "file was not moved on disk");
    assert!(!source.exists());
    assert_eq!(std::fs::read_to_string(&moved).unwrap(), "payload\n");
    assert_eq!(app.buffer.text(), "payload\n");
    assert_eq!(app.documents.path.as_deref(), Some(moved.as_path()));
    assert_eq!(
        app.documents.tabs.active().map(|tab| tab.title.as_str()),
        Some("move-me.txt")
    );
    assert_eq!(
        row_names(&mut app),
        vec!["0:notes".to_string(), "1:move-me.txt".to_string()]
    );
    assert_eq!(app.documents.message.as_deref(), Some("Moved move-me.txt"));
}

#[test]
fn dropping_onto_an_existing_name_leaves_both_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("notes")).unwrap();
    std::fs::write(dir.path().join("notes").join("move-me.txt"), "there\n").unwrap();
    std::fs::write(dir.path().join("move-me.txt"), "here\n").unwrap();
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
    let ctx = Context::new();
    ctx.attach(&mut app);

    // notes, then move-me.txt. The folder already holds that name.
    let file_y = row_center(&app, 1);
    let notes_y = row_center(&app, 0);
    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        40.0,
        file_y,
        MouseButton::Left
    ));
    assert!(drag::update(&mut app, 40.0, notes_y));
    drag::release(&mut app, &ctx);
    ctx.wake(&mut app);
    assert!(
        app.documents
            .message
            .as_deref()
            .is_some_and(|message| message.contains("already exists"))
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("move-me.txt")).unwrap(),
        "here\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("notes").join("move-me.txt")).unwrap(),
        "there\n"
    );
}

#[test]
fn dragging_a_folder_onto_its_child_does_not_move_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("parent").join("child")).unwrap();
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
    let ctx = Context::new();
    ctx.attach(&mut app);
    let parent_y = row_center(&app, 0);
    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        20.0,
        parent_y,
        MouseButton::Left
    ));
    drag::release(&mut app, &ctx);
    ctx.wake(&mut app);
    assert_eq!(
        row_names(&mut app),
        vec!["0:parent".to_string(), "1:child".to_string()]
    );
    let child_y = row_center(&app, 1);
    assert!(handle_sidebar_click(
        &mut app,
        &ctx,
        30.0,
        parent_y,
        MouseButton::Left
    ));
    assert!(drag::update(&mut app, 30.0, child_y));
    drag::release(&mut app, &ctx);
    assert!(dir.path().join("parent").join("child").is_dir());
    assert!(
        !dir.path()
            .join("parent")
            .join("child")
            .join("parent")
            .exists()
    );
    assert_eq!(
        row_names(&mut app),
        vec!["0:parent".to_string(), "1:child".to_string()]
    );
}

/// Opening a folder and adding a second one show their headers at once and
/// read the first level on the file-ops worker.
#[test]
fn opening_and_adding_folders_read_them_on_the_worker() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    std::fs::write(alpha.path().join("a.txt"), "a").unwrap();
    std::fs::write(beta.path().join("b.txt"), "b").unwrap();
    let mut app = app();
    let ctx = Context::new();
    ctx.attach(&mut app);

    reads::open_root(&mut app, alpha.path().to_path_buf());
    assert_eq!(app.workspace_root(), Some(alpha.path().to_path_buf()));
    assert!(row_names(&mut app).is_empty(), "read inline");
    ctx.wake(&mut app);
    assert_eq!(row_names(&mut app), vec!["0:a.txt"]);

    assert!(app.add_sidebar_folder(beta.path().to_path_buf()));
    let label = tree_label(app.ui.sidebar.as_ref().unwrap());
    assert!(!label.contains("b.txt"), "read inline: {label}");
    ctx.wake(&mut app);
    let label = tree_label(app.ui.sidebar.as_ref().unwrap());
    assert!(label.contains("1:b.txt"), "{label}");
}

/// A folder that cannot be listed is dropped once the read fails, as a
/// failed synchronous open always was.
#[test]
#[cfg(unix)]
fn an_unreadable_folder_closes_the_sidebar_when_its_read_fails() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&locked).is_ok() {
        // Running as root: permissions do not stop the read.
        return;
    }
    let mut app = app();
    let ctx = Context::new();
    ctx.attach(&mut app);
    reads::open_root(&mut app, locked.clone());
    assert!(app.ui.sidebar.is_some());
    ctx.wake(&mut app);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(app.ui.sidebar.is_none());
}
