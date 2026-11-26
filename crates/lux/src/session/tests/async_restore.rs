//! Restoring at launch never reads on the UI thread: `restore::start`
//! returns before the session file is read, the window stays usable, and
//! tabs open as their reads land, with the order, active tab, carets,
//! scroll, split layout and undo history of a synchronous restore.

use lux_platform::{PlatformContext, PlatformEventHandler};

use super::hot_exit::{Ctx, new_app, open_file, type_text};
use super::*;
use crate::split::LayoutPreset;

/// Quits an app with `b` (pinned), then `a` (edited, active), a split,
/// and returns the data directory and the text typed into `a`.
fn quit_with_two_tabs(dir: &Path) -> (PathBuf, PathBuf, PathBuf, String) {
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    std::fs::write(&a, "alpha\n".repeat(40)).unwrap();
    std::fs::write(&b, "beta\n").unwrap();
    let data = dir.join("data");
    let mut app = new_app();
    app.ui.session.data_dir = Some(data.clone());
    let untitled = app.documents.entries[0].tab_id;
    open_file(&mut app, &b);
    let b_tab = app.documents.active_tab.unwrap();
    app.documents.tabs.pin(b_tab);
    open_file(&mut app, &a);
    app.documents.entries.retain(|e| e.tab_id != untitled);
    app.documents.tabs.close(untitled);
    type_text(&mut app, "typed ");
    let line_height = app.editor_view.metrics().line_height.max(1.0);
    app.viewport.scroll_y = 5.0 * line_height;
    app.set_layout(LayoutPreset::Columns2);
    let typed = app.buffer.text();
    PlatformEventHandler::exiting(&mut app, &Ctx);
    (data, a, b, typed)
}

fn tab_paths(app: &App) -> Vec<Option<PathBuf>> {
    app.documents
        .tabs
        .tabs()
        .iter()
        .map(|tab| {
            app.documents
                .entries
                .iter()
                .find(|e| e.tab_id == tab.id)
                .and_then(|e| e.path.clone())
        })
        .collect()
}

#[test]
fn launch_restore_returns_before_reading_and_tabs_land_from_workers() {
    let dir = tempfile::tempdir().unwrap();
    let (data, a, b, typed) = quit_with_two_tabs(dir.path());
    let ctx = crate::document::tests::Context::new(
        None,
        None,
        lux_platform::dialog::SaveDecision::Discard,
    );
    let mut app = new_app();
    app.ui.session.data_dir = Some(data);
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());

    let hold = app.ui.dialogs.jobs.hold();
    restore::start(&mut app, Some(&ctx), None);
    assert_eq!(app.documents.entries.len(), 1, "restored on the UI thread");
    assert!(app.documents.path.is_none());
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 1);

    hold.release();
    crate::ui_io::finish_all(&mut app, &ctx);
    assert!(app.ui.session.restoring.is_none(), "every tab landed");
    assert_eq!(tab_paths(&app), vec![Some(b.clone()), Some(a.clone())]);
    assert_eq!(app.documents.path.as_deref(), Some(a.as_path()));
    assert_eq!(app.buffer.text(), typed);
    assert!(app.history.can_undo(), "the undo history came back");
    let line_height = app.editor_view.metrics().line_height.max(1.0);
    assert!((app.viewport.scroll_y - 5.0 * line_height).abs() < 0.5);
    assert_eq!(app.ui.split.panes.len(), 2, "the split came back");
    let b_tab = app.documents.tabs.tabs()[0].clone();
    assert!(b_tab.pinned);
}

/// Tabs are added in session order as each lands, the active tab shows as
/// soon as it lands, and the launch's empty tab goes away then.
#[test]
fn tabs_open_one_by_one_and_the_active_one_shows_when_it_lands() {
    let dir = tempfile::tempdir().unwrap();
    let (data, a, b, typed) = quit_with_two_tabs(dir.path());
    let file = lux_workspace::session_store::session_file(&data, None);
    let plan = restore::read_plan(&file).unwrap().expect("a session");
    let mut app = new_app();
    let generation = restore_apply::next_generation(&mut app);
    let (tabs, history_dir) = restore_apply::begin(&mut app, generation, plan).unwrap();
    let mut tabs = tabs.into_iter();

    let first = restore::read_tab(tabs.next().unwrap(), &history_dir).map(Box::new);
    assert_eq!(
        restore_apply::apply_tab(&mut app, generation, 0, first),
        None
    );
    // A pinned tab goes to the front of the strip, as it did when saved.
    assert_eq!(tab_paths(&app), vec![Some(b.clone()), None]);
    assert!(app.documents.path.is_none(), "b is not the active tab");

    let second = restore::read_tab(tabs.next().unwrap(), &history_dir).map(Box::new);
    assert_eq!(
        restore_apply::apply_tab(&mut app, generation, 1, second),
        Some(true)
    );
    assert_eq!(tab_paths(&app), vec![Some(b), Some(a.clone())]);
    assert_eq!(app.documents.path.as_deref(), Some(a.as_path()));
    assert_eq!(app.buffer.text(), typed);
}

/// A user who starts typing before the active tab lands keeps their tab.
#[test]
fn a_tab_the_user_typed_in_is_not_taken_away() {
    let dir = tempfile::tempdir().unwrap();
    let (data, _, _, _) = quit_with_two_tabs(dir.path());
    let file = lux_workspace::session_store::session_file(&data, None);
    let plan = restore::read_plan(&file).unwrap().unwrap();
    let mut app = new_app();
    let generation = restore_apply::next_generation(&mut app);
    let (tabs, history_dir) = restore_apply::begin(&mut app, generation, plan).unwrap();
    type_text(&mut app, "mine");
    for (index, state) in tabs.into_iter().enumerate() {
        let tab = restore::read_tab(state, &history_dir).map(Box::new);
        restore_apply::apply_tab(&mut app, generation, index, tab);
    }
    assert_eq!(app.buffer.text(), "mine");
    assert!(app.documents.path.is_none());
    assert_eq!(app.documents.entries.len(), 3, "both tabs were still added");
}

/// A newer restore (another folder opened) drops the older one's tabs.
#[test]
fn an_older_restore_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (data, _, _, _) = quit_with_two_tabs(dir.path());
    let file = lux_workspace::session_store::session_file(&data, None);
    let plan = restore::read_plan(&file).unwrap().unwrap();
    let mut app = new_app();
    let old = restore_apply::next_generation(&mut app);
    let (tabs, history_dir) = restore_apply::begin(&mut app, old, plan).unwrap();
    restore_apply::next_generation(&mut app);
    let tab = restore::read_tab(tabs.into_iter().next().unwrap(), &history_dir).map(Box::new);
    assert_eq!(restore_apply::apply_tab(&mut app, old, 0, tab), None);
    assert_eq!(app.documents.entries.len(), 1);
}

/// Quitting while tabs are still landing keeps the unlanded ones in the
/// session, with their drafts and undo histories.
#[test]
fn quitting_mid_restore_keeps_the_tabs_still_landing() {
    let dir = tempfile::tempdir().unwrap();
    let (data, a, b, typed) = quit_with_two_tabs(dir.path());
    let file = lux_workspace::session_store::session_file(&data, None);
    let plan = restore::read_plan(&file).unwrap().unwrap();
    let mut app = new_app();
    app.ui.session.data_dir = Some(data.clone());
    let generation = restore_apply::next_generation(&mut app);
    let (tabs, history_dir) = restore_apply::begin(&mut app, generation, plan).unwrap();
    let first = restore::read_tab(tabs[0].clone(), &history_dir).map(Box::new);
    restore_apply::apply_tab(&mut app, generation, 0, first);

    PlatformEventHandler::exiting(&mut app, &Ctx);
    let mut relaunch = new_app();
    relaunch.ui.session.data_dir = Some(data);
    assert!(restore_session(&mut relaunch, &file).unwrap());
    let paths: Vec<_> = tab_paths(&relaunch).into_iter().flatten().collect();
    assert!(paths.contains(&a) && paths.contains(&b), "{paths:?}");
    let a_entry = relaunch
        .documents
        .entries
        .iter()
        .find(|e| e.path.as_deref() == Some(a.as_path()))
        .unwrap();
    let text = if relaunch.documents.path.as_deref() == Some(a.as_path()) {
        relaunch.buffer.text()
    } else {
        a_entry.buffer.text()
    };
    assert_eq!(text, typed);
}
