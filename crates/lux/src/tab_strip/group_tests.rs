//! One strip per split group (ST-SPLIT-05): where each strip is, what a
//! click in it does, its own `▼` list, dragging a tab to another group,
//! and the session keeping each group's tabs. Clicks and drags go through
//! `handle_mouse_click_on_chrome` and the move/release handlers the event
//! loop calls.

use lux_platform::MouseButton;
use lux_ui::tabs::TabId;

use super::strips::{self, Strip};
use super::tests::{Ctx, open_tabs};
use super::{RowHit, hit};
use crate::app::App;
use crate::split::LayoutPreset;

fn app(scale: f64, tabs: usize) -> (App, Vec<TabId>) {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.set_display_scale(scale);
    let s = scale as f32;
    app.viewport.width = 1000.0 * s - app.minimap_width();
    app.viewport.height = 600.0 * s;
    let ids = open_tabs(&mut app, tabs);
    (app, ids)
}

fn strip(app: &App, group: usize) -> Strip {
    strips::all(app)
        .into_iter()
        .find(|s| s.group == Some(group))
        .expect("group strip")
}

fn center_of(app: &App, strip: &Strip, index: usize) -> (f32, f32) {
    let l = strip.layout;
    let x = l.left + (index as f32 + 0.3) * l.tab_width - strips::scroll(app, strip.group);
    (x, strip.top + strip.height * 0.5)
}

fn click(app: &mut App, (x, y): (f32, f32), button: MouseButton) {
    app.mouse.position = dpi::PhysicalPosition::new(f64::from(x), f64::from(y));
    app.mouse.placed = true;
    app.handle_mouse_click_on_chrome(&Ctx::new(), button);
}

#[test]
fn each_column_has_its_own_strip_over_its_own_column() {
    for scale in [1.0, 2.0] {
        for (with_sidebar, inset) in [(false, 78.0), (true, 78.0), (false, 0.0)] {
            let (mut app, _) = app(scale, 4);
            let dir = tempfile::tempdir().unwrap();
            if with_sidebar {
                app.ui.sidebar = crate::sidebar::SidebarState::open_folder(dir.path().into());
                app.viewport.width -= app.sidebar_width();
            }
            let inset = inset * scale as f32;
            app.documents.strip.controls_inset = inset;
            app.set_layout(LayoutPreset::Columns2);
            let groups = crate::split::groups::rects(&app);
            let (left, right) = (strip(&app, 0), strip(&app, 1));
            // Only the strip at the window's leading corner steps around
            // the window controls; in fullscreen (inset 0) nothing does.
            assert_eq!(left.layout.left, app.sidebar_width().max(inset));
            assert_eq!(right.layout.left, groups[1].editor[0]);
            assert_eq!(left.top, 0.0);
            assert!(left.layout.left + left.width <= right.layout.left + 0.01);
            let end = groups[1].editor[0] + groups[1].editor[2];
            assert!((right.layout.left + right.width - end).abs() < 0.01);
            // Group 0 lists the tabs group 1 did not take.
            assert_eq!(left.layout.count + right.layout.count, 5);
            let y = left.top + 2.0;
            assert_eq!(
                hit(&app, left.layout.left + 2.0, y).map(|(s, h)| (s.map(|s| s.group), h)),
                Some((Some(Some(0)), RowHit::Tab(0, false)))
            );
            assert_eq!(
                hit(&app, right.layout.left + 2.0, y).map(|(s, h)| (s.map(|s| s.group), h)),
                Some((Some(Some(1)), RowHit::Tab(0, false)))
            );
            if inset > 0.0 && !with_sidebar {
                assert_eq!(hit(&app, inset - 1.0, y), Some((None, RowHit::Empty)));
            }
        }
    }
}

#[test]
fn a_lower_group_has_its_strip_at_its_own_top() {
    let (mut app, _) = app(2.0, 3);
    app.documents.strip.controls_inset = 156.0;
    app.set_layout(LayoutPreset::Rows2);
    let groups = crate::split::groups::rects(&app);
    let bottom = strip(&app, 1);
    assert_eq!(bottom.layout.left, 0.0, "no window controls down here");
    assert_eq!(bottom.top + bottom.height, groups[1].editor[1]);
    assert!(bottom.top > strip(&app, 0).top);
    let at = center_of(&app, &bottom, 0);
    assert_eq!(
        hit(&app, at.0, at.1).map(|(_, h)| h),
        Some(RowHit::Tab(0, false))
    );
}

#[test]
fn clicking_a_tab_in_another_group_focuses_that_group() {
    let (mut app, ids) = app(1.0, 4);
    app.set_layout(LayoutPreset::Columns2);
    assert_eq!(app.ui.split.active_pane, 0);
    let right = strip(&app, 1);
    let shown = app.ui.split.panes[1].tab_id;
    let at = center_of(&app, &right, 0);
    click(&mut app, at, MouseButton::Left);
    assert_eq!(app.ui.split.active_pane, 1);
    assert_eq!(app.documents.active_tab, Some(shown));
    // A tab of group 0 that group 0 does not show: group 0 takes focus
    // and shows it.
    let left = strip(&app, 0);
    let target = app.ui.split.panes[0].tabs[2];
    assert!(ids.contains(&target) || target != shown);
    let at = center_of(&app, &left, 2);
    click(&mut app, at, MouseButton::Left);
    assert_eq!(app.ui.split.active_pane, 0);
    assert_eq!(app.documents.active_tab, Some(target));
    assert_eq!(app.ui.split.panes[0].tab_id, target);
    assert_eq!(app.ui.split.panes[1].tab_id, shown, "group 1 kept its tab");
}

#[test]
fn a_new_tab_opens_in_the_focused_group() {
    let (mut app, _) = app(1.0, 3);
    app.set_layout(LayoutPreset::Columns2);
    app.focus_group(1);
    let new = open_tabs(&mut app, 1)[0];
    app.switch_to_tab(new);
    assert_eq!(app.ui.split.active_pane, 1);
    assert!(app.ui.split.panes[1].tabs.contains(&new));
    assert!(!app.ui.split.panes[0].tabs.contains(&new));
    // One opened without being switched to joins the focused group too.
    let quiet = open_tabs(&mut app, 1)[0];
    super::follow_active(&mut app);
    assert!(app.ui.split.panes[1].tabs.contains(&quiet));
}

#[test]
fn switching_to_a_tab_of_another_group_focuses_that_group() {
    let (mut app, _) = app(1.0, 3);
    app.set_layout(LayoutPreset::Columns2);
    let other = app.ui.split.panes[1].tab_id;
    assert!(app.switch_to_tab(other));
    assert_eq!(app.ui.split.active_pane, 1);
    // Control+Tab cycles within the focused group only.
    let before = app.ui.split.panes[0].tabs.clone();
    app.cycle_tab(true);
    assert_eq!(app.ui.split.active_pane, 1);
    assert_eq!(app.ui.split.panes[0].tabs, before);
}

#[test]
fn each_groups_overflow_list_shows_that_groups_tabs() {
    let (mut app, _) = app(1.0, 30);
    app.set_layout(LayoutPreset::Columns2);
    let left = strip(&app, 0);
    let [x, w] = left.layout.button.expect("group 0 overflows");
    click(&mut app, (x + w * 0.5, left.top + 4.0), MouseButton::Left);
    let rows = super::list::rows(&app.documents, false).expect("list open");
    let ids: Vec<TabId> = rows.iter().map(|r| r.id).collect();
    assert_eq!(ids, app.ui.split.panes[0].tabs);
    assert_eq!(app.documents.strip.list.as_ref().unwrap().group, Some(0));
    // Group 1 has one tab: no `▼`.
    assert!(strip(&app, 1).layout.button.is_none());
}

#[test]
fn dragging_a_tab_onto_another_groups_strip_moves_it_there() {
    for scale in [1.0, 2.0] {
        let (mut app, _) = app(scale, 3);
        app.set_layout(LayoutPreset::Columns2);
        let left = strip(&app, 0);
        let moved = app.ui.split.panes[0].tabs[1];
        let from = center_of(&app, &left, 1);
        click(&mut app, from, MouseButton::Left);
        assert_eq!(app.documents.strip.drag.unwrap().group, Some(0));
        let right = strip(&app, 1);
        // Before the right group's first tab.
        let to = (right.layout.left + 3.0, right.top + 4.0);
        app.mouse.position = dpi::PhysicalPosition::new(f64::from(to.0), f64::from(to.1));
        assert!(super::pointer_moved(&mut app));
        let (target, slot) = super::drop_target(&app).expect("over a strip");
        assert_eq!((target.group, slot), (Some(1), 0));
        super::release_drag(&mut app);
        assert_eq!(app.ui.split.panes[1].tabs[0], moved);
        assert!(!app.ui.split.panes[0].tabs.contains(&moved));
        assert_eq!(app.ui.split.active_pane, 1);
        assert_eq!(app.documents.active_tab, Some(moved));
    }
}

#[test]
fn a_drag_within_a_group_reorders_only_that_group() {
    let (mut app, _) = app(1.0, 4);
    app.set_layout(LayoutPreset::Columns2);
    let left = strip(&app, 0);
    let order = app.ui.split.panes[0].tabs.clone();
    let right_tabs = app.ui.split.panes[1].tabs.clone();
    let at = center_of(&app, &left, 0);
    click(&mut app, at, MouseButton::Left);
    let end = left.layout.left + left.layout.tab_width * 2.0 + 2.0;
    app.mouse.position.x = f64::from(end);
    super::pointer_moved(&mut app);
    super::release_drag(&mut app);
    let now = &app.ui.split.panes[0].tabs;
    assert_eq!(now[1], order[0]);
    assert_eq!(now[0], order[1]);
    assert_eq!(app.ui.split.panes[1].tabs, right_tabs);
}

#[test]
fn closing_a_groups_last_tab_closes_the_group() {
    let (mut app, _) = app(1.0, 2);
    app.set_layout(LayoutPreset::Columns2);
    let right = strip(&app, 1);
    assert_eq!(right.layout.count, 1);
    let lone = app.ui.split.panes[1].tab_id;
    let close = (
        right.layout.left + right.layout.tab_width - 6.0,
        right.top + 4.0,
    );
    click(&mut app, close, MouseButton::Left);
    assert!(!app.documents.tabs.tabs().iter().any(|t| t.id == lone));
    assert!(!app.ui.split.is_active(), "two groups became one");
}

#[test]
fn a_dirty_tab_in_another_group_closes_with_the_prompt() {
    let (mut app, _) = app(1.0, 2);
    app.set_layout(LayoutPreset::Columns2);
    let lone = app.ui.split.panes[1].tab_id;
    let entry = app
        .documents
        .entries
        .iter_mut()
        .find(|e| e.tab_id == lone)
        .unwrap();
    entry.saved_version = entry.history.state_id() + 1;
    let right = strip(&app, 1);
    let close = (
        right.layout.left + right.layout.tab_width - 6.0,
        right.top + 4.0,
    );
    click(&mut app, close, MouseButton::Left);
    // The close went through the unsaved prompt's path: the tab's group
    // took focus and the document is being closed by the worker.
    assert_eq!(app.ui.split.active_pane, 1);
    assert_eq!(app.documents.active_tab, Some(lone));
}

#[test]
fn leaving_the_split_keeps_each_groups_order() {
    let (mut app, _) = app(1.0, 4);
    app.set_layout(LayoutPreset::Columns2);
    let mut expected = app.ui.split.panes[0].tabs.clone();
    expected.extend(app.ui.split.panes[1].tabs.iter().copied());
    app.set_layout(LayoutPreset::Single);
    let order: Vec<TabId> = app.documents.tabs.tabs().iter().map(|t| t.id).collect();
    assert_eq!(order, expected);
}

#[test]
fn the_session_keeps_each_groups_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for name in ["a.rs", "b.rs", "c.rs", "d.rs"] {
        let path = dir.path().join(name);
        std::fs::write(&path, format!("// {name}\n")).unwrap();
        paths.push(path);
    }
    let (mut app, _) = app(1.0, 0);
    for path in &paths {
        let buffer = lux_core::buffer::Buffer::open(path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let id = app.documents.tabs.open(name);
        app.documents
            .entries
            .push(crate::document::DocumentEntry::new(
                id,
                Some(path.clone()),
                buffer,
            ));
        app.switch_to_tab(id);
    }
    let untitled = app.documents.tabs.tabs()[0].id;
    app.documents.entries.retain(|e| e.tab_id != untitled);
    app.documents.tabs.close(untitled);
    app.set_layout(LayoutPreset::Columns2);
    // Move a second tab into group 1 so both groups list two.
    let extra = app.ui.split.panes[0].tabs[0];
    app.move_to_group(1);
    app.focus_group(0);
    app.switch_to_tab(extra);
    app.move_to_group(1);
    let title = |app: &App, id: TabId| {
        app.documents
            .tabs
            .tabs()
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.title.clone())
            .unwrap()
    };
    let saved: Vec<Vec<String>> = app
        .ui
        .split
        .panes
        .iter()
        .map(|p| p.tabs.iter().map(|id| title(&app, *id)).collect())
        .collect();
    assert_eq!(saved.iter().map(Vec::len).sum::<usize>(), 4);
    assert!(saved.iter().all(|g| !g.is_empty()));

    app.ui.session.data_dir = Some(dir.path().join("data"));
    let file = crate::session::current_file_for_tests(&app);
    crate::session::save_session(&mut app, true).unwrap();
    let (mut fresh, _) = self::app(1.0, 0);
    fresh.ui.session.data_dir = Some(dir.path().join("data"));
    assert!(crate::session::restore_session(&mut fresh, &file).unwrap());
    let restored: Vec<Vec<String>> = fresh
        .ui
        .split
        .panes
        .iter()
        .map(|p| p.tabs.iter().map(|id| title(&fresh, *id)).collect())
        .collect();
    assert_eq!(restored, saved);
}
