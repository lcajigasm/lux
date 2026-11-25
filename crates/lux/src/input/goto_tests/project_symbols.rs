//! Goto Symbol in Project (`Cmd+Shift+R`) driven through the app: the
//! background index is woken by the platform waker, and keystrokes in the
//! panel only filter the snapshot it published.

use super::*;
use crate::goto::GotoState;
use crate::goto::symbols::{INDEXING_NOTICE, NO_FOLDER_NOTICE};
use lux_platform::PlatformEventHandler;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn with_waker(app: &mut App) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    app.ui.session.register_waker(Arc::new(move || {
        let _ = tx.send(());
    }));
    rx
}

/// Blocks on the waker (no timer), then runs the same handler the platform
/// loop calls on a background wake, until the open panel satisfies `done`.
fn wait_until(app: &mut App, woke: &mpsc::Receiver<()>, done: impl Fn(&GotoState) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if app.ui.goto_anything.as_ref().is_some_and(&done) {
            return;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        woke.recv_timeout(left)
            .expect("the symbol indexer never woke the editor");
        app.background_event(&DummyContext);
    }
}

fn labels(app: &App) -> Vec<String> {
    let goto = app.ui.goto_anything.as_ref().expect("panel open");
    goto.items.iter().map(|it| it.label.clone()).collect()
}

fn open_two_tabs(dir: &Path) -> (App, lux_ui::tabs::TabId, lux_ui::tabs::TabId) {
    let first = dir.join("tab1.rs");
    let second = dir.join("tab2.rs");
    let first_code = "fn start() {}\npub fn first_tab_handler() {}\n";
    let second_code = "\n\npub fn second_tab_calculator(x: i32) -> i32 { x * 2 }\n";
    std::fs::write(&first, first_code).unwrap();
    std::fs::write(&second, second_code).unwrap();

    let mut app = test_app(first_code);
    let tab1 = app
        .documents
        .active_tab
        .unwrap_or_else(|| app.documents.tabs.open("tab1.rs"));
    app.documents.active_tab = Some(tab1);
    app.documents.path = Some(first);
    let tab2 = app.documents.tabs.open("tab2.rs");
    let buffer = Buffer::from_bytes(second_code.as_bytes()).unwrap();
    app.documents
        .entries
        .push(crate::document::DocumentEntry::new(
            tab2,
            Some(second),
            buffer,
        ));
    (app, tab1, tab2)
}

#[test]
fn open_tabs_are_indexed_in_the_background_and_typing_reads_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _tab1, tab2) = open_two_tabs(dir.path());
    let woke = with_waker(&mut app);

    app.open_goto_project_symbols();
    let goto = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(
        goto.notice,
        Some(INDEXING_NOTICE),
        "the scan is not done yet"
    );
    assert!(
        goto.items.is_empty(),
        "no symbol is listed before it is indexed"
    );

    wait_until(&mut app, &woke, |g| {
        g.notice.is_none() && g.items.len() >= 3
    });
    let names = labels(&app);
    assert!(names.contains(&"first_tab_handler".to_string()));
    assert!(names.contains(&"second_tab_calculator".to_string()));

    // No folder, so no watcher: if a keystroke re-read the disk it would
    // see this new content and lose the old symbol.
    std::fs::write(dir.path().join("tab2.rs"), "fn replaced_on_disk() {}\n").unwrap();
    type_str(&mut app, "second");
    assert_eq!(labels(&app), ["second_tab_calculator"]);

    press_key(&mut app, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.documents.active_tab, Some(tab2));
    assert_eq!(app.cursors.primary().head.line, 2);
}

#[test]
fn a_workspace_is_scanned_and_watcher_changes_reach_the_panel() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(
        root.join("math.rs"),
        "pub fn compute_factorial(n: u64) -> u64 { n }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("geom.rs"),
        "\n\npub fn calculate_hypotenuse(a: f64, b: f64) -> f64 { a + b }\n",
    )
    .unwrap();

    let mut app = test_app("fn main() {}\n");
    let woke = with_waker(&mut app);
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root.clone());
    app.ui.session.init_watcher(&root);

    app.open_goto_project_symbols();
    wait_until(&mut app, &woke, |g| g.notice.is_none());
    type_str(&mut app, "hypotenuse");
    let goto = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(labels(&app), ["calculate_hypotenuse"]);
    assert_eq!(
        goto.items[0].path, "geom.rs",
        "paths are relative to the root"
    );

    press_key(&mut app, NamedKey::Enter);
    assert_eq!(app.cursors.primary().head.line, 2);
    assert!(app.documents.path.as_ref().unwrap().ends_with("geom.rs"));

    // A file created outside the editor arrives through the watcher.
    std::fs::write(root.join("fresh.rs"), "struct FreshlyWrittenType;\n").unwrap();
    app.open_goto_project_symbols();
    type_str(&mut app, "freshly");
    wait_until(&mut app, &woke, |g| g.items.len() == 1);
    assert_eq!(labels(&app), ["FreshlyWrittenType"]);
}

#[test]
fn escape_restores_the_tab_and_caret_after_a_symbol_preview() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, tab1, tab2) = open_two_tabs(dir.path());
    let woke = with_waker(&mut app);
    assert_eq!(app.cursors.primary().head.line, 0);

    app.open_goto_project_symbols();
    wait_until(&mut app, &woke, |g| g.notice.is_none());
    type_str(&mut app, "second_tab");
    assert_eq!(app.documents.active_tab, Some(tab2));
    assert_eq!(app.cursors.primary().head.line, 2);

    press_key(&mut app, NamedKey::Escape);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.documents.active_tab, Some(tab1));
    assert_eq!(app.cursors.primary().head.line, 0);
}

#[test]
fn with_nothing_to_index_the_panel_says_so() {
    let mut app = test_app("fn unsaved() {}\n");
    app.open_goto_project_symbols();
    let goto = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(goto.notice, Some(NO_FOLDER_NOTICE));
    assert!(goto.items.is_empty());
    assert!(app.ui.session.symbols.is_none());
}
