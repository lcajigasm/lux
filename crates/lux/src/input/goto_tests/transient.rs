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
    // A wake the test already took may have left a result waiting.
    crate::ui_io::poll(app, &DummyContext);
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
fn an_edit_to_the_previewed_buffer_makes_it_permanent() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, origin, woke) = fixture(dir.path());
    let before = tab_count(&app);

    app.open_goto_anything("");
    type_str(&mut app, "peek_gamma");
    land_reads(&mut app, &woke);
    let shown = app.documents.active_tab.unwrap();
    assert!(preview::is_transient(&app.documents, shown));

    // Keys go to the panel, so the edit comes through the editor directly.
    app.insert_at_cursor("x");
    assert!(app.buffer.text().starts_with('x'));
    assert!(preview::transient(&app.documents).is_none());

    // Escape still goes back to the origin, but keeps the edited tab.
    press_key(&mut app, NamedKey::Escape);
    assert_eq!(app.documents.active_tab, Some(origin));
    assert_eq!(tab_count(&app), before + 1);

    // A later preview opens its own tab instead of replacing that one.
    app.open_goto_anything("");
    type_str(&mut app, "peek_alpha");
    land_reads(&mut app, &woke);
    assert_eq!(tab_count(&app), before + 2);
    press_key(&mut app, NamedKey::Escape);
    assert_eq!(tab_count(&app), before + 1);
    app.switch_to_tab(shown);
    assert!(app.buffer.text().starts_with('x'));
}

/// The paths of `dir` the preview worker read since `from`.
fn reads_in(dir: &Path, from: usize) -> Vec<String> {
    let canonical = dir.canonicalize().unwrap();
    let reads = preview::READS.lock().unwrap();
    reads[from..]
        .iter()
        .filter(|p| p.starts_with(dir) || p.starts_with(&canonical))
        .map(|p| p.file_name().unwrap().to_string_lossy().into())
        .collect()
}

fn reads_so_far() -> usize {
    preview::READS.lock().unwrap().len()
}

#[test]
fn requests_the_selection_left_before_the_worker_took_them_are_never_read() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, woke) = fixture(dir.path());
    let from = reads_so_far();

    app.open_goto_anything("");
    let hold = app.ui.dialogs.jobs.hold();
    type_str(&mut app, "peek");
    press_key(&mut app, NamedKey::ArrowDown);
    press_key(&mut app, NamedKey::ArrowDown);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 1, "one request waits");
    let last = selected_file(&app);
    hold.release();
    land_reads(&mut app, &woke);

    assert_eq!(reads_in(dir.path(), from), std::slice::from_ref(&last));
    assert_eq!(active_file(&app), last);

    // Moving to an open file withdraws a request that has not started.
    let from = reads_so_far();
    let hold = app.ui.dialogs.jobs.hold();
    press_key(&mut app, NamedKey::ArrowUp);
    for _ in 0.."peek".len() {
        press_key(&mut app, NamedKey::Backspace);
    }
    type_str(&mut app, "main.rs");
    assert_eq!(selected_file(&app), "main.rs");
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 0);
    hold.release();
    assert!(reads_in(dir.path(), from).is_empty());
    press_key(&mut app, NamedKey::Escape);
}

#[test]
fn a_read_that_lands_after_the_selection_moved_on_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, origin, woke) = fixture(dir.path());
    let before = tab_count(&app);

    app.open_goto_anything("");
    type_str(&mut app, "peek_alpha");
    // The read finishes, but the editor has not taken its result yet.
    woke.recv_timeout(Duration::from_secs(10))
        .expect("the preview read never woke the editor");
    // Typing never waits for the read; the selection moves to the open file.
    for _ in 0.."peek_alpha".len() {
        press_key(&mut app, NamedKey::Backspace);
    }
    type_str(&mut app, "main.rs");
    assert_eq!(selected_file(&app), "main.rs");
    land_reads(&mut app, &woke);

    assert_eq!(tab_count(&app), before, "the stale read opened no tab");
    assert_eq!(app.documents.active_tab, Some(origin));
    assert!(preview::transient(&app.documents).is_none());

    // A read finished when the panel closes is dropped as well.
    for _ in 0.."main.rs".len() {
        press_key(&mut app, NamedKey::Backspace);
    }
    type_str(&mut app, "peek_beta");
    woke.recv_timeout(Duration::from_secs(10))
        .expect("the preview read never woke the editor");
    press_key(&mut app, NamedKey::Escape);
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
fn the_tab_list_marks_the_transient_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, woke) = fixture(dir.path());

    app.open_goto_anything("");
    type_str(&mut app, "peek_alpha");
    land_reads(&mut app, &woke);

    crate::tab_strip::list::open(&mut app);
    let rows = crate::tab_strip::list::rows(&app.documents, false).unwrap();
    let details: Vec<_> = rows.iter().map(crate::tab_strip::list::detail).collect();
    assert_eq!(
        details,
        [None, Some(crate::tab_strip::list::PREVIEW_DETAIL)]
    );
}

/// Previews `peek_beta.rs` with its caret on line 30, then closes the
/// panel with `close`; the panel must leave as Escape does.
fn closing_restores_the_origin(close: impl FnOnce(&mut App)) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, origin, woke) = fixture(dir.path());
    let before = tab_count(&app);

    app.open_goto_anything("");
    type_str(&mut app, "peek_beta:30");
    land_reads(&mut app, &woke);
    assert!(preview::transient(&app.documents).is_some());
    assert_eq!(caret_line(&app), 29);

    close(&mut app);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(tab_count(&app), before);
    assert_eq!(app.documents.active_tab, Some(origin));
    assert_eq!(active_file(&app), "main.rs");
    assert!(preview::transient(&app.documents).is_none());

    // The next Cmd+P starts from the origin, not from a leftover preview.
    app.open_goto_anything("");
    assert_eq!(app.documents.active_tab, Some(origin));
    assert_eq!(tab_count(&app), before);
}

#[test]
fn a_right_click_in_the_editor_closes_the_panel_like_escape() {
    closing_restores_the_origin(|app| {
        crate::edit_menu::press_editor(app, &DummyContext, lux_platform::MouseButton::Right);
        app.mouse.context_menu = None;
    });
}

#[test]
fn escaping_focus_to_the_editor_closes_the_panel_like_escape() {
    closing_restores_the_origin(|app| {
        assert!(crate::focus::escape_to_editor(app));
        assert_eq!(caret_line(app), 2, "the origin's caret is back");
    });
}

#[test]
fn enter_with_no_result_closes_the_panel_like_escape() {
    closing_restores_the_origin(|app| {
        type_str(app, "zzzzqqqq");
        assert!(app.ui.goto_anything.as_ref().unwrap().items.is_empty());
        press_key(app, NamedKey::Enter);
    });
}

#[test]
fn reopening_the_panel_while_it_is_open_starts_from_the_origin() {
    closing_restores_the_origin(|app| {
        app.open_goto_anything("");
        assert!(app.ui.goto_anything.is_some());
        press_key(app, NamedKey::Escape);
    });
}

#[test]
fn a_reload_from_disk_keeps_the_preview_transient_and_shows_the_new_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _, woke) = fixture(dir.path());
    let (wake_tx, reloaded) = mpsc::channel();
    app.ui.session.register_waker(Arc::new(move || {
        let _ = wake_tx.send(());
    }));

    app.open_goto_anything("");
    type_str(&mut app, "peek_beta");
    land_reads(&mut app, &woke);
    let shown = app.documents.active_tab.unwrap();
    let path = app.documents.path.clone().unwrap();

    std::fs::write(&path, "changed on disk\n").unwrap();
    let change = lux_workspace::watcher::FileChange {
        path,
        kind: lux_workspace::watcher::ChangeKind::Reload,
        is_dir: false,
    };
    crate::session::disk_events::apply_changes(&mut app, vec![change]);
    // Other wakes share this waker; the reload is the one that changes the text.
    while app.buffer.text() != "changed on disk\n" {
        reloaded
            .recv_timeout(Duration::from_secs(10))
            .expect("the reload never woke the editor");
        crate::session::poll_file_watcher(&mut app);
    }
    // The frame and the session snapshot both settle; neither promotes.
    crate::goto::preview::settle(&mut app);
    assert!(preview::is_transient(&app.documents, shown));

    press_key(&mut app, NamedKey::Escape);
    assert!(preview::transient(&app.documents).is_none());
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
