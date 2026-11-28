//! The async restore with split groups (ST-SPLIT-05): every landed tab
//! goes back to the group that listed it, at its saved place, whatever
//! order the reads finish in, and unsaved text comes back unsaved.

use lux_platform::PlatformEventHandler;
use lux_ui::tabs::TabId;

use super::hot_exit::{Ctx, new_app, open_file, type_text};
use super::*;
use crate::split::LayoutPreset;

/// Quits an app with `a`, `b`, `c` (unsaved edit) and `d` open, split in
/// two groups: `[a, c]` showing `c`, and `[b, d]` showing `d`, focused.
/// Returns the data directory and the four paths.
fn quit_with_two_groups(dir: &Path) -> (PathBuf, [PathBuf; 4]) {
    let paths = ["a.txt", "b.txt", "c.txt", "d.txt"].map(|name| {
        let path = dir.join(name);
        std::fs::write(&path, format!("{name}\n")).unwrap();
        path
    });
    let data = dir.join("data");
    let mut app = new_app();
    app.ui.session.data_dir = Some(data.clone());
    let untitled = app.documents.entries[0].tab_id;
    let mut ids = Vec::new();
    for path in &paths {
        open_file(&mut app, path);
        if path.ends_with("c.txt") {
            type_text(&mut app, "draft ");
        }
        ids.push(app.documents.active_tab.unwrap());
    }
    app.documents.entries.retain(|e| e.tab_id != untitled);
    app.documents.tabs.close(untitled);
    app.set_layout(LayoutPreset::Columns2);
    let lists = [[ids[0], ids[2]], [ids[1], ids[3]]];
    for (pane, list) in app.ui.split.panes.iter_mut().zip(lists) {
        pane.tabs = list.to_vec();
        pane.tab_id = list[1];
        pane.active_tab = list[1];
    }
    app.documents.active_tab = None;
    crate::split::tabs::enter_pane(&mut app, 1);
    crate::split::tabs::reconcile(&mut app);
    PlatformEventHandler::exiting(&mut app, &Ctx);
    (data, paths)
}

fn path_of(app: &App, id: TabId) -> PathBuf {
    app.documents
        .entries
        .iter()
        .find(|e| e.tab_id == id)
        .and_then(|e| e.path.clone())
        .expect("a file tab")
}

fn group(app: &App, idx: usize) -> Vec<PathBuf> {
    let pane = &app.ui.split.panes[idx];
    pane.tabs.iter().map(|id| path_of(app, *id)).collect()
}

fn strip(app: &App) -> Vec<PathBuf> {
    let tabs = app.documents.tabs.tabs();
    tabs.iter().map(|t| path_of(app, t.id)).collect()
}

/// Each group's tabs, order and shown tab, the focused group, the strip
/// order, and the unsaved tab, as the synchronous restore gives them.
fn assert_groups_restored(app: &App, [a, b, c, d]: &[PathBuf; 4]) {
    assert!(app.ui.session.restoring.is_none(), "every tab landed");
    assert_eq!(app.ui.split.panes.len(), 2, "the split came back");
    assert_eq!(group(app, 0), vec![a.clone(), c.clone()]);
    assert_eq!(group(app, 1), vec![b.clone(), d.clone()]);
    assert_eq!(path_of(app, app.ui.split.panes[0].tab_id), *c);
    assert_eq!(app.ui.split.active_pane, 1, "the focused group");
    assert_eq!(app.documents.path.as_deref(), Some(d.as_path()));
    let all = vec![a.clone(), b.clone(), c.clone(), d.clone()];
    assert_eq!(strip(app), all, "the strip keeps the session's order");
    let c_entry = app
        .documents
        .entries
        .iter()
        .find(|e| e.path.as_deref() == Some(c.as_path()))
        .unwrap();
    assert!(c_entry.is_dirty(), "the unsaved tab stays unsaved");
    assert_eq!(c_entry.buffer.text(), "draft c.txt\n");
}

#[test]
fn a_split_session_restores_each_groups_tabs_through_the_workers() {
    let dir = tempfile::tempdir().unwrap();
    let (data, paths) = quit_with_two_groups(dir.path());
    let mut app = new_app();
    app.ui.session.data_dir = Some(data);

    let hold = app.ui.dialogs.jobs.hold();
    restore::start(&mut app, Some(&Ctx), None);
    assert_eq!(app.documents.entries.len(), 1, "restored on the UI thread");
    hold.release();
    crate::ui_io::finish_all(&mut app, &Ctx);

    assert_groups_restored(&app, &paths);
}

/// The reads finish out of session order, with `c`, the first group's
/// shown and unsaved tab, last: every tab still ends at its saved place
/// in its saved group.
#[test]
fn a_tab_that_lands_last_keeps_its_place_and_group() {
    let dir = tempfile::tempdir().unwrap();
    let (data, paths) = quit_with_two_groups(dir.path());
    let file = lux_workspace::session_store::session_file(&data, None);
    let plan = restore::read_plan(&file).unwrap().expect("a session");
    let mut app = new_app();
    app.ui.session.data_dir = Some(data);
    let generation = restore_apply::next_generation(&mut app);
    let (generation, tabs, history_dir) = restore_apply::begin(&mut app, generation, plan).unwrap();
    let index_of = |path: &PathBuf| {
        tabs.iter()
            .position(|t| t.path.as_ref() == Some(path))
            .unwrap()
    };
    let [a, b, c, d] = &paths;
    let order = [index_of(d), index_of(b), index_of(a), index_of(c)];
    for (step, index) in order.into_iter().enumerate() {
        let tab = restore::read_tab(tabs[index].clone(), &history_dir).map(Box::new);
        let done = restore_apply::apply_tab(&mut app, generation, index, tab);
        assert_eq!(done, (step == 3).then_some(true), "step {step}");
    }

    assert_groups_restored(&app, &paths);
}

/// A save while tabs are still landing keeps the session's split when one
/// group's shown tab failed to read: that group shows another of its
/// tabs, and the other group keeps its own.
#[test]
fn a_save_mid_restore_keeps_the_split_when_a_shown_tab_failed() {
    let dir = tempfile::tempdir().unwrap();
    let (data, paths) = quit_with_two_groups(dir.path());
    let file = lux_workspace::session_store::session_file(&data, None);
    let plan = restore::read_plan(&file).unwrap().expect("a session");
    let mut app = new_app();
    app.ui.session.data_dir = Some(data);
    let generation = restore_apply::next_generation(&mut app);
    let (generation, tabs, history_dir) = restore_apply::begin(&mut app, generation, plan).unwrap();
    let index_of = |path: &PathBuf| {
        tabs.iter()
            .position(|t| t.path.as_ref() == Some(path))
            .unwrap()
    };
    let [a, b, c, d] = &paths;
    // `c`, the first group's shown tab, fails to read; `a` is still landing.
    restore_apply::apply_tab(&mut app, generation, index_of(c), None);
    for path in [b, d] {
        let index = index_of(path);
        let tab = restore::read_tab(tabs[index].clone(), &history_dir).map(Box::new);
        restore_apply::apply_tab(&mut app, generation, index, tab);
    }
    assert!(app.ui.session.restoring.is_some(), "a has not landed");

    save_session(&mut app, false).unwrap();
    let written = Session::load(crate::session::snapshot::current_file(&app)).unwrap();
    let layout = written.layout.expect("the split was kept");
    let path_at = |index: usize| written.tabs[index].path.clone().unwrap();
    assert_eq!(layout.panes.len(), 2);
    assert_eq!(path_at(layout.panes[0].tab), *a);
    assert_eq!(path_at(layout.panes[1].tab), *d);
    let second: Vec<_> = layout.panes[1].tabs.iter().map(|i| path_at(*i)).collect();
    assert_eq!(second, vec![b.clone(), d.clone()]);
    assert_eq!(layout.active_pane, 1);
}
