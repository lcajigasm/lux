//! ST-SIDE-01 through the app's own entry points: right-click, the
//! context menu, the inline field's keys, and the background event that
//! applies the worker's result.

use lux_platform::{ElementState, Key, KeyEvent, MouseButton, NamedKey};

use super::behavior_tests::{Context, app, row_center, row_names};
use super::menu::{SIDEBAR_MENU_ITEM_HEIGHT, SidebarAction};
use super::*;

fn key(app: &mut crate::app::App, ctx: &Context, key: Key) {
    let event = KeyEvent {
        key,
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    assert!(handle_sidebar_key(app, ctx, &event), "key was not consumed");
}

fn type_text(app: &mut crate::app::App, ctx: &Context, text: &str) {
    for ch in text.chars() {
        key(app, ctx, Key::Character(ch.to_string()));
    }
}

/// Right-clicks the listed row at `index`, then clicks `action` in the menu.
fn menu_action(app: &mut crate::app::App, ctx: &Context, index: usize, action: SidebarAction) {
    let y = row_center(app, index);
    assert!(handle_sidebar_click(app, ctx, 30.0, y, MouseButton::Right));
    let menu = app
        .ui
        .sidebar
        .as_ref()
        .unwrap()
        .context_menu
        .clone()
        .unwrap();
    let item = menu.items.iter().position(|a| *a == action).unwrap();
    let scale = app.scale_factor;
    let item_y =
        menu.rect[1] + 4.0 * scale + (item as f32 + 0.5) * SIDEBAR_MENU_ITEM_HEIGHT * scale;
    assert!(handle_sidebar_click(
        app,
        ctx,
        menu.rect[0] + 10.0,
        item_y,
        MouseButton::Left
    ));
}

fn index_of(app: &mut crate::app::App, name: &str) -> usize {
    let names = row_names(app);
    names
        .iter()
        .position(|row| row.ends_with(&format!(":{name}")))
        .unwrap_or_else(|| panic!("{name} not in {names:?}"))
}

fn setup(dir: &std::path::Path) -> (crate::app::App, Context) {
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.to_path_buf());
    let ctx = Context::new();
    ctx.attach(&mut app);
    (app, ctx)
}

/// The TODO's acceptance: right-click a folder, New File..., type
/// `service.rs`, Enter. The file exists, has a row, and is the open tab.
#[test]
fn new_file_from_the_folder_menu_creates_lists_and_opens_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    let (mut app, ctx) = setup(dir.path());
    let src = index_of(&mut app, "src");
    menu_action(&mut app, &ctx, src, SidebarAction::NewFile);
    assert!(app.ui.sidebar.as_ref().unwrap().inline_input.is_some());
    type_text(&mut app, &ctx, "service.rs");
    key(&mut app, &ctx, Key::Named(NamedKey::Enter));
    assert!(app.ui.sidebar.as_ref().unwrap().inline_input.is_none());

    ctx.wake(&mut app);
    ctx.complete(&mut app);
    // Opening the file reveals it; `src` is read on the worker first. Other
    // workers (syntax, git) wake the loop too, so wait for the row itself.
    let service = dir.path().join("src").join("service.rs");
    assert!(service.is_file());
    for _ in 0..20 {
        if row_names(&mut app).contains(&"1:service.rs".to_string()) {
            break;
        }
        ctx.wake(&mut app);
    }
    let rows = row_names(&mut app);
    assert!(rows.contains(&"1:service.rs".to_string()), "{rows:?}");
    assert_eq!(app.documents.path.as_deref(), Some(service.as_path()));
}

#[test]
fn rename_selects_the_base_name_and_the_caret_keys_edit_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("service.rs"), "fn a() {}\n").unwrap();
    let (mut app, ctx) = setup(dir.path());
    let row = index_of(&mut app, "service.rs");
    menu_action(&mut app, &ctx, row, SidebarAction::Rename);
    {
        let field = &app
            .ui
            .sidebar
            .as_ref()
            .unwrap()
            .inline_input
            .as_ref()
            .unwrap()
            .field;
        assert_eq!(field.selected_text(), "service");
    }
    // End, then back over the extension: "service|.rs" -> "service_v2.rs".
    key(&mut app, &ctx, Key::Named(NamedKey::End));
    for _ in 0..3 {
        key(&mut app, &ctx, Key::Named(NamedKey::ArrowLeft));
    }
    type_text(&mut app, &ctx, "_v2");
    key(&mut app, &ctx, Key::Named(NamedKey::Home));
    key(&mut app, &ctx, Key::Named(NamedKey::Delete));
    type_text(&mut app, &ctx, "S");
    key(&mut app, &ctx, Key::Named(NamedKey::Enter));
    ctx.wake(&mut app);
    assert!(dir.path().join("Service_v2.rs").is_file());
    assert!(!dir.path().join("service.rs").exists());
    assert!(row_names(&mut app).contains(&"0:Service_v2.rs".to_string()));
}

#[test]
fn a_rename_onto_an_existing_name_reports_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    std::fs::write(dir.path().join("b.txt"), "b").unwrap();
    let (mut app, ctx) = setup(dir.path());
    let row = index_of(&mut app, "a.txt");
    menu_action(&mut app, &ctx, row, SidebarAction::Rename);
    type_text(&mut app, &ctx, "b");
    key(&mut app, &ctx, Key::Named(NamedKey::Enter));
    ctx.wake(&mut app);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
        "b"
    );
    assert!(dir.path().join("a.txt").is_file());
    let message = app.documents.message.clone().unwrap_or_default();
    assert!(message.contains("Cannot rename"), "{message}");
    assert_eq!(row_names(&mut app), vec!["0:a.txt", "0:b.txt"]);
}

#[test]
fn delete_asks_on_the_worker_and_a_no_keeps_the_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("keep.txt"), "k").unwrap();
    let mut app = app();
    app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
    let ctx = Context::answering_trash(false);
    ctx.attach(&mut app);
    let row = index_of(&mut app, "keep.txt");
    menu_action(&mut app, &ctx, row, SidebarAction::Delete);
    ctx.wake(&mut app);
    let asked_on = ctx
        .dialogs
        .asked_on
        .lock()
        .unwrap()
        .expect("confirmation shown");
    assert_ne!(asked_on, std::thread::current().id());
    assert!(dir.path().join("keep.txt").is_file());
    assert_eq!(row_names(&mut app), vec!["0:keep.txt"]);
}

#[test]
fn delete_confirmed_trashes_off_the_ui_thread_and_drops_the_row() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("gone.txt"), "g").unwrap();
    std::fs::write(dir.path().join("keep.txt"), "k").unwrap();
    let (mut app, ctx) = setup(dir.path());
    let row = index_of(&mut app, "gone.txt");
    menu_action(&mut app, &ctx, row, SidebarAction::Delete);
    ctx.wake(&mut app);
    let asked_on = ctx
        .dialogs
        .asked_on
        .lock()
        .unwrap()
        .expect("confirmation shown");
    assert_ne!(asked_on, std::thread::current().id());
    assert!(!dir.path().join("gone.txt").exists());
    assert_eq!(row_names(&mut app), vec!["0:keep.txt"]);
}

/// The menu on a row of a second open folder acts on that folder's file,
/// not on the header folder's node with the same id.
#[test]
fn duplicate_in_a_second_folder_copies_that_folders_file() {
    let alpha = tempfile::tempdir().unwrap();
    let beta = tempfile::tempdir().unwrap();
    std::fs::write(alpha.path().join("alpha.txt"), "alpha").unwrap();
    std::fs::write(beta.path().join("beta.txt"), "beta").unwrap();
    let (mut app, ctx) = setup(alpha.path());
    assert!(app.add_sidebar_folder(beta.path().to_path_buf()));
    ctx.wake(&mut app);
    // Rows: alpha.txt, the beta title, beta.txt.
    menu_action(&mut app, &ctx, 2, SidebarAction::Duplicate);
    ctx.wake(&mut app);
    assert_eq!(
        std::fs::read_to_string(beta.path().join("beta copy.txt")).unwrap(),
        "beta"
    );
    assert!(!alpha.path().join("alpha copy.txt").exists());
    let label = tree_label(app.ui.sidebar.as_ref().unwrap());
    assert!(label.contains("1:beta copy.txt"), "{label}");
}

/// Opens `path` through the document worker and waits for it.
fn open(app: &mut crate::app::App, ctx: &Context, path: &std::path::Path) {
    app.request_document(
        ctx,
        crate::document::Request::Open(Some(path.to_path_buf())),
    );
    ctx.complete(app);
    assert_eq!(app.documents.path.as_deref(), Some(path));
}

/// Holds the next sidebar job on its worker until the sender sends.
fn hold_jobs(app: &mut crate::app::App) -> std::sync::mpsc::Sender<()> {
    let (release, hold) = std::sync::mpsc::channel();
    app.ui.session.file_ops.hold = Some(hold);
    release
}

/// Starts renaming `a.rs` to `b.rs` from the menu, then asks to save the
/// open `a.rs` while the rename is still held on the worker.
fn save_during_rename(app: &mut crate::app::App, ctx: &Context) -> std::sync::mpsc::Sender<()> {
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"new\n").unwrap();
    let release = hold_jobs(app);
    let row = index_of(app, "a.rs");
    menu_action(app, ctx, row, SidebarAction::Rename);
    type_text(app, ctx, "b");
    key(app, ctx, Key::Named(NamedKey::Enter));
    app.request_document(ctx, crate::document::Request::Save { save_as: false });
    assert!(!crate::document::open_queue::busy(app), "the save waits");
    assert_eq!(app.ui.session.file_ops.renames.waiting(), 1);
    release
}

fn finish(app: &mut crate::app::App, ctx: &Context, release: std::sync::mpsc::Sender<()>) {
    release.send(()).unwrap();
    // Each wake runs the background event, as the platform loop does,
    // until the rename has reported and the waiting save has finished.
    while app.ui.session.file_ops.renames.waiting() > 0 || crate::document::open_queue::busy(app) {
        ctx.wake(app);
    }
}

/// Review finding: a save right after a rename of the open file used to
/// write the old path. It now waits for the rename and writes the new one.
#[test]
fn a_save_during_a_rename_writes_the_renamed_file() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.rs");
    let b = dir.path().join("b.rs");
    std::fs::write(&a, "old\n").unwrap();
    let (mut app, ctx) = setup(dir.path());
    open(&mut app, &ctx, &a);
    let release = save_during_rename(&mut app, &ctx);
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "old\n");
    assert!(!b.exists());

    finish(&mut app, &ctx, release);
    assert!(!a.exists(), "the old path was not recreated");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "new\n");
    assert_eq!(app.documents.path.as_deref(), Some(b.as_path()));
    assert!(!app.document_dirty());
}

/// A rename that fails leaves the tab on the old path, and the waiting
/// save writes there.
#[test]
fn a_save_during_a_failed_rename_writes_the_old_path() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.rs");
    let b = dir.path().join("b.rs");
    std::fs::write(&a, "old\n").unwrap();
    std::fs::write(&b, "taken\n").unwrap();
    let (mut app, ctx) = setup(dir.path());
    open(&mut app, &ctx, &a);
    let release = save_during_rename(&mut app, &ctx);

    finish(&mut app, &ctx, release);
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "new\n");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "taken\n");
    assert_eq!(app.documents.path.as_deref(), Some(a.as_path()));
    let message = app.documents.message.clone().unwrap_or_default();
    assert!(!message.contains("Cannot check"), "{message}");
}

/// Trashing the open file keeps its tab, marked modified like a file
/// deleted by another program, and Save writes it back.
#[test]
fn trashing_the_open_file_marks_its_tab_modified() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("gone.txt");
    std::fs::write(&gone, "text\n").unwrap();
    let (mut app, ctx) = setup(dir.path());
    open(&mut app, &ctx, &gone);
    assert!(!app.document_dirty());
    let row = index_of(&mut app, "gone.txt");
    menu_action(&mut app, &ctx, row, SidebarAction::Delete);
    ctx.wake(&mut app);
    assert!(!gone.exists());
    assert!(app.document_dirty());
    assert!(app.documents.disk_snapshot.is_none());
    assert_eq!(app.documents.path.as_deref(), Some(gone.as_path()));

    app.request_document(&ctx, crate::document::Request::Save { save_as: false });
    ctx.complete(&mut app);
    assert_eq!(std::fs::read_to_string(&gone).unwrap(), "text\n");
    assert!(!app.document_dirty());
}
