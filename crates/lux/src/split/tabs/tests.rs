//! Review fixes for per-group tabs (ST-SPLIT-05): closing a view versus a
//! document, moving a tab to its own group, reconcile only when something
//! changed, the session note on view changes, older sessions, and Goto
//! Anything's preview keeping focus in its group.

use std::sync::Arc;

use lux_ui::tabs::TabId;

use crate::app::App;
use crate::document::Request;
use crate::split::LayoutPreset;
use crate::tab_strip::tests::{Ctx, open_tabs};

fn app(tabs: usize) -> (App, Vec<TabId>) {
    let mut app =
        App::new(include_bytes!("../../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.viewport.width = 1200.0;
    app.viewport.height = 600.0;
    let untitled = app.documents.tabs.tabs()[0].id;
    let ids = open_tabs(&mut app, tabs);
    if tabs > 0 {
        // Only the tabs a test names, so `reconcile` adds none.
        app.documents.entries.retain(|e| e.tab_id != untitled);
        app.documents.tabs.close(untitled);
        app.switch_to_tab(ids[0]);
    }
    (app, ids)
}

/// Gives each group exactly `lists`, showing each list's first tab, and
/// focuses group `focus` on its first tab.
fn arrange(app: &mut App, lists: &[&[TabId]], focus: usize) {
    for (pane, list) in app.ui.split.panes.iter_mut().zip(lists) {
        pane.tabs = list.to_vec();
        pane.tab_id = list[0];
        pane.active_tab = list[0];
    }
    app.ui.split.active_pane = focus;
    app.documents.active_tab = None;
    crate::split::tabs::enter_pane(app, focus);
    crate::split::tabs::reconcile(app);
}

fn listed(app: &App, group: usize) -> Vec<TabId> {
    app.ui.split.panes[group].tabs.clone()
}

fn is_open(app: &App, tab: TabId) -> bool {
    app.documents.tabs.position(tab).is_some()
}

#[test]
fn moving_a_tab_to_its_own_group_changes_nothing() {
    let (mut app, ids) = app(3);
    app.set_layout(LayoutPreset::Columns2);
    arrange(&mut app, &[&[ids[0], ids[1], ids[2]], &[ids[0]]], 0);
    let before = listed(&app, 0);
    assert!(!app.move_to_group(0));
    assert_eq!(listed(&app, 0), before);
    assert_eq!(app.documents.active_tab, Some(ids[0]));
}

#[test]
fn command_w_on_a_tab_another_group_lists_closes_only_this_view() {
    let (mut app, ids) = app(3);
    app.set_layout(LayoutPreset::Columns2);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    arrange(&mut app, &[&[a, b], &[c, a]], 0);
    app.request_document(&Ctx::new(), Request::Close);
    assert!(is_open(&app, a), "the document stays open");
    assert!(app.documents.entries.iter().any(|e| e.tab_id == a));
    assert_eq!(listed(&app, 0), vec![b]);
    assert_eq!(listed(&app, 1), vec![c, a]);
    assert_eq!(app.ui.split.active_pane, 0);
    assert_eq!(app.documents.active_tab, Some(b));
}

#[test]
fn command_w_on_a_groups_only_view_closes_that_group() {
    let (mut app, ids) = app(3);
    app.set_layout(LayoutPreset::Columns3);
    let (a, b, other) = (ids[0], ids[1], ids[2]);
    arrange(&mut app, &[&[other], &[a], &[b, a]], 1);
    app.request_document(&Ctx::new(), Request::Close);
    assert!(is_open(&app, a));
    assert_eq!(app.ui.split.panes.len(), 2, "the emptied group closed");
    assert!(app.ui.split.panes.iter().any(|p| p.tabs.contains(&a)));
    assert!(app.documents.active_tab.is_some());
}

#[test]
fn a_command_w_with_no_other_view_still_closes_the_document() {
    let (mut app, ids) = app(3);
    app.set_layout(LayoutPreset::Columns2);
    arrange(&mut app, &[&[ids[0], ids[1]], &[ids[2]]], 0);
    // Not routed to a view: the close goes to the worker as before, which
    // asks about unsaved changes and then closes the document.
    assert!(!crate::split::tabs::close_active_view(&mut app));
    assert_eq!(listed(&app, 0), vec![ids[0], ids[1]]);
}

/// The document of the focused group closed while another group showed
/// it, and the focused group, now empty, hands its place to that group.
#[test]
fn closing_a_document_another_group_shows_leaves_a_document_active() {
    let (mut app, ids) = app(3);
    app.set_layout(LayoutPreset::Columns3);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    arrange(&mut app, &[&[a], &[a, b], &[c]], 0);
    // What the worker's close does for the editor's document.
    app.documents.tabs.close(a);
    app.documents.entries.retain(|e| e.tab_id != a);
    assert!(crate::split::tabs::after_active_closed(&mut app, a));
    let active = app.documents.active_tab.expect("a document is active");
    assert!(app.documents.entries.iter().any(|e| e.tab_id == active));
    assert_eq!(active, b);
    assert_eq!(app.buffer.text(), "1");
    for pane in &app.ui.split.panes {
        assert_ne!(pane.tab_id, a);
        assert!(!pane.tabs.contains(&a));
    }
}

#[test]
fn entering_a_group_that_names_a_closed_document_shows_an_open_one() {
    let (mut app, ids) = app(3);
    app.set_layout(LayoutPreset::Columns2);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    arrange(&mut app, &[&[a], &[b, c]], 0);
    app.documents.entries.retain(|e| e.tab_id != b);
    app.documents.tabs.close(b);
    crate::split::tabs::enter_pane(&mut app, 1);
    assert_eq!(app.documents.active_tab, Some(c));
    assert_eq!(app.ui.split.panes[1].tab_id, c);
}

#[test]
fn reconcile_runs_only_after_a_change() {
    let (mut app, ids) = app(4);
    app.set_layout(LayoutPreset::Columns2);
    crate::tab_strip::follow_active(&mut app);
    // A steady frame: nothing to reconcile, so nothing is allocated.
    assert!(!crate::split::tabs::reconcile(&mut app));
    assert!(!crate::split::tabs::reconcile(&mut app));
    // A tab closes: the next frame reconciles once.
    let gone = ids[3];
    app.documents.entries.retain(|e| e.tab_id != gone);
    app.documents.tabs.close(gone);
    assert!(crate::split::tabs::reconcile(&mut app));
    assert!(app.ui.split.panes.iter().all(|p| !p.tabs.contains(&gone)));
    assert!(!crate::split::tabs::reconcile(&mut app));
    // A tab opens: it joins the focused group.
    let new = open_tabs(&mut app, 1)[0];
    assert!(crate::split::tabs::reconcile(&mut app));
    assert!(listed(&app, app.ui.split.active_pane).contains(&new));
}

#[test]
fn view_changes_are_noted_for_the_session() {
    let (mut app, ids) = app(3);
    app.ui.session.register_waker(Arc::new(|| {}));
    app.set_layout(LayoutPreset::Columns2);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    arrange(&mut app, &[&[a, b], &[c, a]], 0);

    app.ui.session.take_unsaved();
    crate::split::tabs::close_in_group(&mut app, &Ctx::new(), Some(1), a);
    assert_eq!(listed(&app, 1), vec![c]);
    assert!(app.ui.session.take_unsaved(), "removing a view is noted");

    crate::split::tabs::select(&mut app, Some(0), b);
    assert!(app.ui.session.take_unsaved(), "selecting a tab is noted");
}

#[test]
fn an_older_session_without_group_tabs_restores_every_tab_to_the_first_group() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app(0);
    for name in ["a.rs", "b.rs", "c.rs", "d.rs"] {
        let path = dir.path().join(name);
        std::fs::write(&path, format!("// {name}\n")).unwrap();
        let buffer = lux_core::buffer::Buffer::open(&path).unwrap();
        let id = app.documents.tabs.open(name);
        app.documents
            .entries
            .push(crate::document::DocumentEntry::new(id, Some(path), buffer));
        app.switch_to_tab(id);
    }
    let untitled = app.documents.tabs.tabs()[0].id;
    app.documents.entries.retain(|e| e.tab_id != untitled);
    app.documents.tabs.close(untitled);
    app.set_layout(LayoutPreset::Columns2);
    app.focus_group(1);
    let shown_right = app
        .documents
        .tabs
        .get(app.ui.split.panes[1].tab_id)
        .unwrap()
        .title
        .clone();

    app.ui.session.data_dir = Some(dir.path().join("data"));
    let file = crate::session::current_file_for_tests(&app);
    crate::session::save_session(&mut app, true).unwrap();
    // Rewrite it as a session from before groups had their own tabs.
    let mut json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let panes = json["layout"]["panes"].as_array_mut().expect("panes");
    assert!(panes.iter().any(|p| p.get("tabs").is_some()));
    for pane in panes {
        pane.as_object_mut().unwrap().remove("tabs");
    }
    std::fs::write(&file, serde_json::to_string(&json).unwrap()).unwrap();

    let (mut fresh, _) = self::app(0);
    fresh.ui.session.data_dir = Some(dir.path().join("data"));
    assert!(crate::session::restore_session(&mut fresh, &file).unwrap());
    let titles = |app: &App, group: usize| -> Vec<String> {
        app.ui.split.panes[group]
            .tabs
            .iter()
            .map(|id| app.documents.tabs.get(*id).unwrap().title.clone())
            .collect()
    };
    assert_eq!(fresh.ui.split.panes.len(), 2);
    assert_eq!(titles(&fresh, 1), vec![shown_right.clone()]);
    let mut first = titles(&fresh, 0);
    first.sort();
    let mut expected: Vec<String> = ["a.rs", "b.rs", "c.rs", "d.rs"]
        .iter()
        .map(|s| s.to_string())
        .filter(|s| *s != shown_right)
        .collect();
    expected.sort();
    assert_eq!(first, expected, "every other tab is in the first group");
}

#[test]
fn dismissing_an_active_preview_keeps_focus_in_its_group() {
    let (mut app, ids) = app(4);
    app.set_layout(LayoutPreset::Columns2);
    let (first, a, b, c) = (ids[0], ids[1], ids[2], ids[3]);
    arrange(&mut app, &[&[first, a], &[b, c]], 0);
    // A preview opens in the focused group, last in the global order, so
    // its global neighbor (`c`) lives in the other group.
    let preview = open_tabs(&mut app, 1)[0];
    app.switch_to_tab(preview);
    crate::goto::preview::mark_transient(&mut app, preview);
    crate::split::tabs::reconcile(&mut app);
    assert_eq!(listed(&app, 0), vec![first, a, preview]);

    crate::goto::preview::discard(&mut app);
    assert!(!is_open(&app, preview));
    assert_eq!(app.ui.split.active_pane, 0, "focus stays in the group");
    assert_eq!(app.documents.active_tab, Some(a));
}

#[test]
fn dismissing_goto_anything_returns_to_the_group_it_was_opened_from() {
    let (mut app, ids) = app(4);
    app.set_layout(LayoutPreset::Columns2);
    let (first, a, b, c) = (ids[0], ids[1], ids[2], ids[3]);
    arrange(&mut app, &[&[first, a], &[b, c]], 0);
    let mut goto = crate::goto::GotoState::new(&app);
    // Goto Anything activated a tab of the other group, which took focus.
    app.switch_to_tab(c);
    assert_eq!(app.ui.split.active_pane, 1);
    goto.snapshot.as_mut().unwrap().touched.clear();
    crate::goto::preview::dismiss(&mut app, goto);
    assert_eq!(app.ui.split.active_pane, 0);
    assert_eq!(app.documents.active_tab, Some(first));
}
