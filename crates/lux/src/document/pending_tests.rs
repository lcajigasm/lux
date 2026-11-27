use lux_platform::dialog::SaveDecision;

use crate::document::Request;
use crate::document::tests::{Context, app};

fn title(app: &crate::app::App, tab: lux_ui::tabs::TabId) -> String {
    let tab = app.documents.tabs.tabs().iter().find(|t| t.id == tab);
    tab.expect("tab exists").title.clone()
}

/// Two tabs: A is an edited Untitled document, B is a file on disk. A is
/// active.
fn two_tabs(dir: &std::path::Path, ctx: &Context) -> (crate::app::App, [lux_ui::tabs::TabId; 2]) {
    let mut app = app();
    let a = app.documents.active_tab.unwrap();
    app.insert_at_cursor("draft in A");
    let other = dir.join("b.txt");
    std::fs::write(&other, "file B").unwrap();
    app.request_document(ctx, Request::Open(Some(other)));
    ctx.complete(&mut app);
    let b = app.documents.active_tab.unwrap();
    assert_ne!(a, b);
    assert!(app.switch_to_tab(a));
    (app, [a, b])
}

#[test]
fn a_save_as_answered_after_switching_tabs_lands_on_the_tab_that_asked() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("saved-a.txt");
    let ctx = Context::new(None, Some(target.clone()), SaveDecision::Discard);
    let (mut app, [a, b]) = two_tabs(dir.path(), &ctx);

    app.request_document(&ctx, Request::Save { save_as: true });
    // The sheet is up; the event loop keeps running and the user moves on.
    assert!(app.switch_to_tab(b));
    ctx.complete(&mut app);

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "draft in A");
    assert_eq!(app.documents.active_tab, Some(b), "the user stays on B");
    assert_eq!(app.buffer.text(), "file B");
    assert_eq!(
        app.documents.path.as_deref(),
        Some(dir.path().join("b.txt").as_path())
    );
    assert!(!app.document_dirty());
    assert_eq!(title(&app, b), "b.txt");

    assert_eq!(title(&app, a), "saved-a.txt");
    let entry = app
        .documents
        .entries
        .iter()
        .find(|e| e.tab_id == a)
        .unwrap();
    assert_eq!(entry.path.as_deref(), Some(target.as_path()));
    assert!(!entry.is_dirty(), "A is marked saved, not B");
    assert!(app.switch_to_tab(a));
    assert_eq!(app.buffer.text(), "draft in A");
    assert!(!app.document_dirty());
}

#[test]
fn the_tab_whose_dialog_is_up_cannot_be_closed_from_another_tab() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("saved-a.txt");
    let ctx = Context::new(None, Some(target.clone()), SaveDecision::Discard);
    let (mut app, [a, b]) = two_tabs(dir.path(), &ctx);

    app.request_document(&ctx, Request::Save { save_as: true });
    assert!(app.switch_to_tab(b));
    app.close_tab(&ctx, a);
    assert!(!app.close_clean_tab_now(a));
    assert!(app.documents.entries.iter().any(|e| e.tab_id == a));

    ctx.complete(&mut app);
    assert_eq!(title(&app, a), "saved-a.txt");
    assert_eq!(app.documents.active_tab, Some(b));
}

#[test]
fn an_external_edit_reloads_the_open_file_while_the_open_dialog_is_up() {
    let dir = tempfile::tempdir().unwrap();
    // FSEvents reports resolved paths (`/private/var/...` for a temp dir).
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let file = root.join("a.txt");
    let picked = root.join("b.txt");
    std::fs::write(&file, "before the sheet\n").unwrap();
    std::fs::write(&picked, "file B").unwrap();
    let ctx = Context::new(Some(picked.clone()), None, SaveDecision::Discard);
    let mut app = app();
    app.request_document(&ctx, Request::Open(Some(file.clone())));
    ctx.complete(&mut app);
    let a = app.documents.active_tab.unwrap();
    crate::sidebar::watch::watch_and_wait(&mut app, &root);

    // Cmd+O: the panel's worker is in flight until its result is applied.
    app.request_document(&ctx, Request::Open(None));
    assert!(app.documents.pending.is_some());
    std::fs::write(&file, "changed behind the sheet\n").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while app.buffer.text() != "changed behind the sheet\n" {
        assert!(
            std::time::Instant::now() < deadline,
            "the reload never landed while the dialog was pending"
        );
        crate::session::poll_file_watcher(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(app.documents.pending.is_some(), "the dialog is still up");
    assert!(!app.document_dirty());

    // The panel's answer still opens its file, beside the reloaded tab.
    ctx.complete(&mut app);
    assert!(app.documents.pending.is_none());
    assert_eq!(app.buffer.text(), "file B");
    assert_eq!(app.documents.path.as_deref(), Some(picked.as_path()));
    let entry = app
        .documents
        .entries
        .iter()
        .find(|e| e.tab_id == a)
        .unwrap();
    assert_eq!(entry.buffer.text(), "changed behind the sheet\n");
    assert!(!entry.is_dirty());
}
