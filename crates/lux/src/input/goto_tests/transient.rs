//! Goto Anything's transient preview tab (ST-NAV-01), driven through the
//! panel keys: files that are not open are read on a worker and shown in
//! one transient tab; Escape discards it, Enter and edits keep it.

use super::*;
use crate::goto::preview;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

const FILES: [&str; 3] = ["peek_alpha.rs", "peek_beta.rs", "peek_gamma.rs"];

/// A workspace with `main.rs` open (caret on line 3) and three files
/// matching `peek` that are not open.
fn fixture(dir: &Path) -> (App, lux_ui::tabs::TabId, mpsc::Receiver<()>) {
    let main = dir.join("main.rs");
    let main_text = (1..=40).map(|i| format!("main {i}\n")).collect::<String>();
    std::fs::write(&main, &main_text).unwrap();
    for (n, name) in FILES.iter().enumerate() {
        let text = (1..=40)
            .map(|i| format!("{name} {n} {i}\n"))
            .collect::<String>();
        std::fs::write(dir.join(name), text).unwrap();
    }
    let mut app = test_app(&main_text);
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(dir.to_path_buf());
    let origin = app.documents.active_tab.unwrap();
    app.documents.path = Some(main);
    app.sync_active_document_to_entry();
    crate::goto::jump_to_line(&mut app, 3);
    let (tx, rx) = mpsc::channel();
    app.ui.dialogs.jobs.set_wake(Arc::new(move || {
        let _ = tx.send(());
    }));
    (app, origin, rx)
}

/// Blocks on the worker's wake (no timer) and applies every finished read.
fn land_reads(app: &mut App, woke: &mpsc::Receiver<()>) {
    while app.ui.dialogs.jobs.in_flight() > 0 {
        woke.recv_timeout(Duration::from_secs(10))
            .expect("the preview read never woke the editor");
        crate::ui_io::poll(app, &DummyContext);
    }
}

fn selected_file(app: &App) -> String {
    let goto = app.ui.goto_anything.as_ref().expect("panel open");
    let path = &goto.items[goto.selected_index].path;
    Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into()
}

fn active_file(app: &App) -> String {
    let path = app.documents.path.as_ref().expect("a file is shown");
    path.file_name().unwrap().to_string_lossy().into()
}

fn tab_count(app: &App) -> usize {
    app.documents.tabs.tabs().len()
}

fn caret_line(app: &App) -> usize {
    app.cursors.primary().head.line
}

#[test]
fn arrowing_through_unopened_files_shows_each_in_one_transient_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, origin, woke) = fixture(dir.path());
    let before = tab_count(&app);
    assert_eq!(caret_line(&app), 2);

    app.open_goto_anything("");
    type_str(&mut app, "peek");
    let mut seen = Vec::new();
    for step in 0..FILES.len() {
        if step > 0 {
            press_key(&mut app, NamedKey::ArrowDown);
        }
        land_reads(&mut app, &woke);
        let want = selected_file(&app);
        assert_eq!(active_file(&app), want, "step {step} shows the selection");
        assert_eq!(tab_count(&app), before + 1, "one transient tab only");
        let shown = app.documents.active_tab.unwrap();
        assert!(preview::is_transient(&app.documents, shown));
        assert!(app.buffer.text().starts_with(&want));
        seen.push(want);
    }
    seen.sort();
    assert_eq!(seen, FILES);

    press_key(&mut app, NamedKey::Escape);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(tab_count(&app), before);
    assert_eq!(app.documents.active_tab, Some(origin));
    assert_eq!(active_file(&app), "main.rs");
    assert_eq!(caret_line(&app), 2);
    assert!(preview::transient(&app.documents).is_none());
    // A discarded preview is not something Cmd+Shift+T brings back.
    assert!(!app.reopen_last_closed_tab());
}

#[test]
fn enter_keeps_the_previewed_file_as_an_ordinary_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, woke) = fixture(dir.path());
    let before = tab_count(&app);

    app.open_goto_anything("");
    type_str(&mut app, "peek_beta");
    land_reads(&mut app, &woke);
    let shown = app.documents.active_tab.unwrap();
    assert!(preview::is_transient(&app.documents, shown));

    press_key(&mut app, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.documents.active_tab, Some(shown));
    assert_eq!(active_file(&app), "peek_beta.rs");
    assert_eq!(tab_count(&app), before + 1);
    assert!(preview::transient(&app.documents).is_none());
}

#[test]
fn typing_in_a_previewed_buffer_makes_it_permanent() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, woke) = fixture(dir.path());
    let before = tab_count(&app);

    app.open_goto_anything("");
    type_str(&mut app, "peek_gamma");
    land_reads(&mut app, &woke);
    // Leave the panel without dismissing it: the preview stays transient.
    crate::focus::escape_to_editor(&mut app);
    let shown = app.documents.active_tab.unwrap();
    assert!(preview::is_transient(&app.documents, shown));

    type_str(&mut app, "x");
    assert!(app.buffer.text().starts_with('x'));
    assert!(preview::transient(&app.documents).is_none());
    assert_eq!(tab_count(&app), before + 1);

    // A later preview opens its own tab instead of replacing this one.
    app.open_goto_anything("");
    type_str(&mut app, "peek_alpha");
    land_reads(&mut app, &woke);
    assert_eq!(tab_count(&app), before + 2);
    press_key(&mut app, NamedKey::Escape);
    assert_eq!(app.documents.active_tab, Some(shown));
    assert!(app.buffer.text().starts_with('x'));
}

#[test]
fn a_read_that_lands_after_the_selection_moved_on_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, origin, woke) = fixture(dir.path());
    let before = tab_count(&app);

    app.open_goto_anything("");
    let hold = app.ui.dialogs.jobs.hold();
    type_str(&mut app, "peek_alpha");
    assert!(app.ui.dialogs.jobs.in_flight() >= 1, "the read is queued");
    // Typing never waits for the read; the selection moves to the open file.
    for _ in 0.."peek_alpha".len() {
        press_key(&mut app, NamedKey::Backspace);
    }
    type_str(&mut app, "main.rs");
    assert_eq!(selected_file(&app), "main.rs");
    hold.release();
    land_reads(&mut app, &woke);

    assert_eq!(tab_count(&app), before, "the stale read opened no tab");
    assert_eq!(app.documents.active_tab, Some(origin));
    assert!(preview::transient(&app.documents).is_none());

    // A read still in flight when the panel closes is dropped as well.
    let hold = app.ui.dialogs.jobs.hold();
    for _ in 0.."main.rs".len() {
        press_key(&mut app, NamedKey::Backspace);
    }
    type_str(&mut app, "peek_beta");
    press_key(&mut app, NamedKey::Escape);
    hold.release();
    land_reads(&mut app, &woke);
    assert_eq!(tab_count(&app), before);
    assert_eq!(app.documents.active_tab, Some(origin));
}

#[test]
fn escape_restores_the_caret_of_an_open_tab_the_preview_moved() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, origin, _woke) = fixture(dir.path());
    let other = dir.path().join(FILES[0]);
    let tab = app.documents.tabs.open(FILES[0]);
    let buffer = Buffer::open(&other).unwrap();
    app.documents
        .entries
        .push(crate::document::DocumentEntry::new(
            tab,
            Some(other),
            buffer,
        ));
    app.switch_to_tab(origin);

    app.open_goto_anything("");
    type_str(&mut app, "peek_alpha:30");
    assert_eq!(app.documents.active_tab, Some(tab));
    assert_eq!(caret_line(&app), 29);

    press_key(&mut app, NamedKey::Escape);
    assert_eq!(app.documents.active_tab, Some(origin));
    assert_eq!(caret_line(&app), 2);
    app.switch_to_tab(tab);
    assert_eq!(caret_line(&app), 0, "the previewed tab's own caret is back");
}

#[test]
fn the_session_does_not_keep_a_transient_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, woke) = fixture(dir.path());
    app.ui.session.data_dir = Some(dir.path().join("data"));

    app.open_goto_anything("");
    type_str(&mut app, "peek_beta");
    land_reads(&mut app, &woke);
    assert!(preview::transient(&app.documents).is_some());

    crate::session::save_session(&mut app, false).unwrap();
    let root = app.ui.sidebar.as_ref().unwrap().workspace_root.clone();
    let file =
        lux_workspace::session_store::session_file(&dir.path().join("data"), root.as_deref());
    let session = lux_workspace::session::Session::load(&file).unwrap();
    let names: Vec<_> = session
        .tabs
        .iter()
        .filter_map(|t| t.path.as_ref()?.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .collect();
    assert_eq!(names, ["main.rs"]);
    assert_eq!(session.active_tab, Some(0));
}

#[test]
fn double_clicking_the_transient_tab_keeps_it_and_the_tab_list_marks_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, woke) = fixture(dir.path());
    app.viewport.width = 1600.0;

    app.open_goto_anything("");
    type_str(&mut app, "peek_alpha");
    land_reads(&mut app, &woke);
    crate::focus::escape_to_editor(&mut app);
    let shown = app.documents.active_tab.unwrap();

    crate::tab_strip::list::open(&mut app);
    let rows = crate::tab_strip::list::rows(&app.documents, false).unwrap();
    let details: Vec<_> = rows.iter().map(crate::tab_strip::list::detail).collect();
    assert_eq!(
        details,
        [None, Some(crate::tab_strip::list::PREVIEW_DETAIL)]
    );
    app.documents.strip.list = None;

    let strip = crate::tab_strip::layout(&app);
    app.mouse.position.x = (strip.left + strip.tab_width * 1.5) as f64;
    app.mouse.position.y = (app.tab_bar_height() * 0.5) as f64;
    let click = |app: &mut App| {
        app.handle_mouse_click_on_chrome(&DummyContext, lux_platform::MouseButton::Left);
        app.documents.strip.drag = None;
    };
    click(&mut app);
    assert!(
        preview::is_transient(&app.documents, shown),
        "one click keeps it transient"
    );
    click(&mut app);
    assert!(preview::transient(&app.documents).is_none());
    assert_eq!(app.documents.active_tab, Some(shown));
}

#[test]
fn a_huge_file_is_not_previewed_and_the_panel_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, origin, woke) = fixture(dir.path());
    let threshold = lux_core::huge_file::DEFAULT_THRESHOLD_BYTES as usize;
    std::fs::write(dir.path().join("peek_huge.log"), vec![b'a'; threshold]).unwrap();
    let before = tab_count(&app);

    app.open_goto_anything("");
    type_str(&mut app, "peek_huge");
    land_reads(&mut app, &woke);
    assert_eq!(tab_count(&app), before);
    assert_eq!(app.documents.active_tab, Some(origin));
    let notice = app.ui.goto_anything.as_ref().unwrap().notice;
    assert_eq!(notice, Some(preview::TOO_LARGE_NOTICE));
}
