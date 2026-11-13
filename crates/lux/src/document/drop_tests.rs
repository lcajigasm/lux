use lux_platform::dialog::SaveDecision;

use super::open_queue;
use super::tests::{Context, app};

#[test]
fn two_dropped_files_open_as_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("alpha.txt");
    let second = dir.path().join("beta.txt");
    std::fs::write(&first, "alpha\n").unwrap();
    std::fs::write(&second, "beta\n").unwrap();
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    open_queue::enqueue(&mut app, &ctx, first);
    open_queue::enqueue(&mut app, &ctx, second);
    ctx.complete(&mut app);
    ctx.complete(&mut app);
    let titles: Vec<_> = app
        .documents
        .tabs
        .tabs()
        .iter()
        .map(|tab| tab.title.as_str())
        .collect();
    assert_eq!(titles, vec!["alpha.txt", "beta.txt"]);
    assert_eq!(app.buffer.text(), "beta\n");
    assert!(app.documents.open_queue.is_empty());
}

#[test]
fn a_dropped_directory_is_reported_and_the_buffer_stays() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    let before = app.buffer.text();
    open_queue::enqueue(&mut app, &ctx, dir.path().to_path_buf());
    ctx.complete(&mut app);
    let message = app.documents.message.as_deref().unwrap_or("");
    assert!(
        message.contains("Cannot open"),
        "directory drop message was {message:?}"
    );
    assert_eq!(app.buffer.text(), before);
    assert!(app.documents.path.is_none());
}
