//! Goto Anything and build-error jumps read the target on the document
//! worker; the cursor moves when the file lands, not before.

use lux_platform::dialog::SaveDecision;

use std::path::PathBuf;

use crate::document::tests::{Context, app};

fn three_lines(dir: &tempfile::TempDir) -> PathBuf {
    let path = dir.path().join("target.rs");
    std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
    path
}

#[test]
fn goto_returns_before_the_file_is_read_and_places_the_cursor_after() {
    let dir = tempfile::tempdir().unwrap();
    let path = three_lines(&dir);
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    crate::goto::execute_goto(&mut app, &ctx, path.to_str().unwrap(), Some(3), Some(2));
    assert!(app.documents.path.is_none(), "opened on the UI thread");
    assert_eq!(app.buffer.text(), "");
    assert!(super::busy(&app), "no worker started");

    ctx.complete(&mut app);
    assert_eq!(app.documents.path.as_ref(), Some(&path));
    let head = app.cursors.primary().head;
    assert_eq!((head.line, head.column), (2, 1));
}

#[test]
fn goto_to_a_missing_file_reports_it_and_keeps_the_buffer() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("gone.rs");
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.insert_at_cursor("keep");
    crate::goto::execute_goto(&mut app, &ctx, missing.to_str().unwrap(), Some(9), None);
    ctx.complete(&mut app);
    assert_eq!(app.buffer.text(), "keep");
    assert!(app.documents.message.is_some());
}

#[test]
fn build_error_jump_opens_on_the_worker() {
    let dir = tempfile::tempdir().unwrap();
    let path = three_lines(&dir);
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    let location = lux_term::SourceLocation {
        path: path.clone(),
        line: 2,
        column: None,
    };
    app.jump_to_source_location(&location, Some(&ctx));
    assert!(app.documents.path.is_none(), "opened on the UI thread");
    assert!(super::busy(&app));

    ctx.complete(&mut app);
    assert_eq!(app.documents.path.as_ref(), Some(&path));
    assert_eq!(app.cursors.primary().head.line, 1);
}

/// A copy of the open folder elsewhere (`/Volumes/backup/tmp/fixture` for
/// a `/tmp/fixture` folder) ends with the folder's names but is another
/// file: choosing the folder's `src/main.rs` opens that file, not the
/// backup tab, and choosing it again reuses its own tab.
#[test]
fn goto_a_folder_file_does_not_switch_to_a_copy_that_ends_with_the_folder_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("tmp/fixture");
    let relative_root = root.strip_prefix("/").unwrap();
    let backup = dir.path().join("backup").join(relative_root);
    for base in [&root, &backup] {
        std::fs::create_dir_all(base.join("src")).unwrap();
        std::fs::write(base.join("src/main.rs"), "fn main() {}\n").unwrap();
    }
    let backup_file = backup.join("src/main.rs");
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    crate::goto::execute_goto(&mut app, &ctx, backup_file.to_str().unwrap(), None, None);
    ctx.complete(&mut app);
    let backup_tab = app.documents.active_tab;
    assert_eq!(app.documents.path.as_ref(), Some(&backup_file));

    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root.clone());
    crate::goto::execute_goto(&mut app, &ctx, "src/main.rs", None, None);
    ctx.complete(&mut app);
    assert_ne!(app.documents.active_tab, backup_tab, "switched to the copy");
    assert_eq!(app.documents.path.as_ref(), Some(&root.join("src/main.rs")));

    let folder_tab = app.documents.active_tab;
    let tabs = app.documents.tabs.tabs().len();
    // An open tab is switched to on the spot; no worker read starts.
    crate::goto::execute_goto(&mut app, &ctx, backup_file.to_str().unwrap(), None, None);
    assert_eq!(app.documents.active_tab, backup_tab);
    crate::goto::execute_goto(&mut app, &ctx, "src/main.rs", None, None);
    assert_eq!(
        app.documents.active_tab, folder_tab,
        "reuses the folder tab"
    );
    assert_eq!(app.documents.tabs.tabs().len(), tabs);
}

#[test]
fn an_lsp_jump_to_a_closed_file_opens_it_on_the_worker() {
    let dir = tempfile::tempdir().unwrap();
    let path = three_lines(&dir);
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    crate::lsp::jump_to_location(&mut app, Some(&ctx), &path, 2);
    assert!(app.documents.path.is_none(), "opened on the UI thread");
    assert!(super::busy(&app));

    ctx.complete(&mut app);
    assert_eq!(app.documents.path.as_ref(), Some(&path));
    assert_eq!(app.cursors.primary().head.line, 2);
}
