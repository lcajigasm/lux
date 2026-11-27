//! Split groups own their minimap and take input where they draw: the
//! rectangles at scale 1 and 2, with and without the sidebar, and the
//! minimaps the split renderer paints from them.

use super::{GroupRects, contains, editor_area, group_at, layout, rects};
use crate::app::App;
use crate::split::{LayoutPreset, SplitLayout};

fn split(preset: LayoutPreset) -> SplitLayout {
    SplitLayout {
        preset,
        ..SplitLayout::default()
    }
}

fn right(r: [f32; 4]) -> f32 {
    r[0] + r[2]
}

#[test]
fn each_column_keeps_its_minimap_at_its_own_right_edge() {
    for scale in [1.0, 2.0] {
        let area = [220.0 * scale, 32.0 * scale, 780.0 * scale, 600.0 * scale];
        let minimap_w = 90.0 * scale;
        let groups = layout(&split(LayoutPreset::Columns2), area, scale, minimap_w);
        assert_eq!(groups.len(), 2);
        for g in &groups {
            let map = g.minimap.expect("room for a minimap");
            assert_eq!(map[2], minimap_w);
            assert_eq!(right(map), right(g.editor), "minimap ends the group");
            assert_eq!(right(g.text), map[0], "text stops at its minimap");
            assert_eq!(g.text[0], g.editor[0]);
        }
        let [first, second] = [groups[0], groups[1]];
        assert!(
            right(first.editor) <= second.editor[0],
            "groups do not overlap"
        );
        assert!(right(first.minimap.unwrap()) <= second.text[0]);
        assert!((right(second.editor) - right(area)).abs() < 0.01);
    }
}

#[test]
fn rows_each_get_a_full_height_minimap_of_their_own() {
    let area = [0.0, 32.0, 1000.0, 600.0];
    let groups = layout(&split(LayoutPreset::Rows2), area, 1.0, 90.0);
    let [top, bottom] = [groups[0], groups[1]];
    let (top_map, bottom_map) = (top.minimap.unwrap(), bottom.minimap.unwrap());
    assert_eq!(top_map[1], top.editor[1]);
    assert_eq!(top_map[3], top.editor[3]);
    assert!(top_map[1] + top_map[3] <= bottom_map[1]);
}

#[test]
fn a_narrow_group_or_a_hidden_minimap_keeps_all_its_width_for_text() {
    let groups = layout(
        &split(LayoutPreset::Columns4),
        [0.0, 0.0, 600.0, 400.0],
        1.0,
        90.0,
    );
    for g in &groups {
        assert_eq!(g.minimap, None, "150 px is under two minimaps wide");
        assert_eq!(g.text, g.editor);
    }
    let off = layout(
        &split(LayoutPreset::Columns2),
        [0.0, 0.0, 1000.0, 400.0],
        1.0,
        0.0,
    );
    assert!(off.iter().all(|g: &GroupRects| g.minimap.is_none()));
}

fn app(scale: f64, with_sidebar: bool) -> (App, Option<tempfile::TempDir>) {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.set_display_scale(scale);
    let dir = with_sidebar.then(|| tempfile::tempdir().expect("tempdir"));
    if let Some(dir) = &dir {
        app.ui.sidebar = crate::sidebar::SidebarState::open_folder(dir.path().to_path_buf());
        assert!(app.sidebar_width() > 0.0);
    }
    let s = scale as f32;
    app.viewport.width = 1000.0 * s - app.sidebar_width() - app.minimap_width();
    app.viewport.height = 600.0 * s;
    app.insert_at_cursor("fn main() {}\n");
    app.documents.tabs.open("other.rs");
    app.set_layout(LayoutPreset::Columns2);
    (app, dir)
}

#[test]
fn pointer_hits_the_group_it_is_drawn_in() {
    for scale in [1.0, 2.0] {
        for with_sidebar in [false, true] {
            let (app, _dir) = app(scale, with_sidebar);
            let area = editor_area(&app);
            assert_eq!(area[0], app.sidebar_width());
            let groups = rects(&app);
            let y = area[1] + 40.0;
            if with_sidebar {
                assert_eq!(group_at(&app, area[0] - 1.0, y), None, "the sidebar");
            }
            assert_eq!(group_at(&app, groups[0].text[0] + 1.0, y), Some(0));
            let map0 = groups[0].minimap.expect("left minimap");
            assert_eq!(group_at(&app, map0[0] + 1.0, y), Some(0));
            assert_eq!(group_at(&app, groups[1].text[0] + 1.0, y), Some(1));
            let map1 = groups[1].minimap.expect("right minimap");
            assert!(contains(map1, right(map1) - 1.0, y));
            assert_eq!(group_at(&app, right(map1) - 1.0, y), Some(1));
            assert_eq!(group_at(&app, groups[0].text[0] + 1.0, area[1] - 1.0), None);
        }
    }
}

#[test]
fn the_focused_groups_text_rect_follows_focus() {
    let (mut app, _dir) = app(2.0, true);
    let groups = rects(&app);
    assert_eq!(app.active_group_text_rect(), groups[0].text);
    app.focus_group(1);
    assert_eq!(app.active_group_text_rect(), groups[1].text);
}

#[test]
fn a_press_on_the_other_groups_minimap_focuses_that_group() {
    let (mut app, _dir) = app(1.0, false);
    let map1 = rects(&app)[1].minimap.unwrap();
    assert_eq!(app.ui.split.active_pane, 0);
    assert!(crate::minimap::drag::press(
        &mut app,
        map1[0] + 4.0,
        map1[1] + 4.0
    ));
    assert_eq!(app.ui.split.active_pane, 1);
    let geo = crate::minimap::drag::geometry(&app).unwrap();
    assert_eq!((geo.left, geo.right), (map1[0], right(map1)));
}

#[test]
fn each_group_paints_its_minimap_inside_its_own_rect() {
    use crate::render::minimap::Marks;
    use crate::render::split::{SplitMinimap, SplitRenderParams, render_split_layout};

    let (mut app, _dir) = app(1.0, false);
    let area = editor_area(&app);
    let minimap_w = app.minimap_width();
    let groups = rects(&app);
    let mut quads = Vec::new();
    let mut glyphs = Vec::new();
    let (mut gutter_quads, mut gutter_glyphs) = (Vec::new(), Vec::new());
    let params = SplitRenderParams {
        split: &app.ui.split,
        documents: &app.documents,
        area,
        minimap_w,
        scale_factor: 1.0,
        stroke: 1.0,
        font_size: 14.0,
        line_height: 20.0,
        font: app.font,
        active_syntax: app.syntax.as_ref(),
        active_buffer: &app.buffer,
        active_cursors: &app.cursors,
        active_fold_state: &app.fold_state,
        active_bookmarks: &app.bookmarks,
        elapsed: std::time::Duration::ZERO,
        active_scroll: (0.0, 0.0),
        theme: &app.chrome.theme,
        syntax_theme: &lux_syntax::theme::LUX_DARK,
    };
    let minimap = SplitMinimap {
        state: &mut app.ui.minimap,
        marks: Marks {
            git: &[],
            search: None,
        },
    };
    render_split_layout(
        &params,
        Some(minimap),
        &mut app.editor_view,
        &mut app.atlas_state,
        &mut quads,
        &mut glyphs,
        &mut gutter_quads,
        &mut gutter_glyphs,
    );
    for g in &groups {
        let map = g.minimap.unwrap();
        assert!(
            quads.iter().any(|q| q.rect == map),
            "a minimap well at {map:?}"
        );
    }
    // Nothing the left group paints reaches into the right group's text.
    let right_text = groups[1].text;
    let left_map = groups[0].minimap.unwrap();
    for q in quads
        .iter()
        .filter(|q| q.rect[0] >= left_map[0] && q.rect[0] < right(left_map))
    {
        assert!(right(q.rect) <= right_text[0] + 0.01, "{:?}", q.rect);
    }
}
