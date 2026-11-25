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
