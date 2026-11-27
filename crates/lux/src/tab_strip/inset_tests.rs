//! Hit-testing with the macOS traffic-light inset (ST-MAC-02): tabs, close
//! marks, the `▼` button, and drops all start after the window controls,
//! at scale 1 and 2, and the inset is gone in fullscreen.

use lux_platform::title_bar::{leading_inset_physical, leading_inset_points};

use super::{RowHit, drop_slot, layout, press_tab, row_hit, tab_at};
use crate::app::{App, TAB_CLOSE_BTN_WIDTH};

fn app(scale: f64, tabs: usize) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.set_display_scale(scale);
    app.viewport.width = (1000.0 * scale) as f32;
    for index in 0..tabs.saturating_sub(app.documents.tabs.tabs().len()) {
        app.documents.tabs.open(format!("f{index}.txt"));
    }
    app
}

/// The inset the platform reports for a window with the standard buttons
/// (zoom button ending at 66 pt) or in fullscreen.
fn controls(app: &mut App, scale: f64, fullscreen: bool) -> f32 {
    let inset = leading_inset_physical(leading_inset_points(Some(66.0), fullscreen), scale) as f32;
    app.documents.strip.controls_inset = inset;
    inset
}

fn check_windowed(scale: f64) {
    let mut app = app(scale, 3);
    let inset = controls(&mut app, scale, false);
    assert_eq!(inset, (78.0 * scale) as f32);
    let strip = layout(&app);
    assert_eq!(strip.left, inset);
    // Every press left of the inset is title-bar space, not a tab.
    for x in [0.0, inset * 0.5, inset - 1.0] {
        assert_eq!(tab_at(&app, x), None, "x={x}");
        assert_eq!(row_hit(&app, x), RowHit::Empty, "x={x}");
    }
    assert_eq!(row_hit(&app, inset + 1.0), RowHit::Tab(0, false));
    let width = strip.tab_width;
    let close_w = TAB_CLOSE_BTN_WIDTH * app.scale_factor;
    assert_eq!(
        row_hit(&app, inset + width - close_w * 0.5),
        RowHit::Tab(0, true)
    );
    assert_eq!(row_hit(&app, inset + width + 1.0), RowHit::Tab(1, false));
    // Right of the last tab is title-bar space too.
    let end = inset + 3.0 * width;
    if end + 1.0 < strip.right() {
        assert_eq!(row_hit(&app, end + 1.0), RowHit::Empty);
    }
    // Tabs share the width that is left after the inset.
    assert!(strip.left + strip.visible_width <= app.viewport.width + 0.5);
}

#[test]
fn tabs_start_after_the_traffic_lights_at_scale_1() {
    check_windowed(1.0);
}

#[test]
fn tabs_start_after_the_traffic_lights_at_scale_2() {
    check_windowed(2.0);
}

#[test]
fn fullscreen_puts_the_first_tab_at_the_window_edge() {
    for scale in [1.0, 2.0] {
        let mut app = app(scale, 3);
        assert_eq!(controls(&mut app, scale, true), 0.0);
        assert_eq!(layout(&app).left, 0.0);
        assert_eq!(row_hit(&app, 1.0), RowHit::Tab(0, false));
    }
}

#[test]
fn the_overflow_button_stays_at_the_right_end_with_the_inset() {
    for scale in [1.0, 2.0] {
        let mut app = app(scale, 30);
        let inset = controls(&mut app, scale, false);
        let strip = layout(&app);
        let [x, w] = strip.button.expect("30 tabs overflow");
        assert!((x + w - app.viewport.width).abs() < 0.5);
        assert_eq!(row_hit(&app, x + w * 0.5), RowHit::Overflow);
        assert_eq!(row_hit(&app, inset - 1.0), RowHit::Empty);
        assert_eq!(row_hit(&app, inset + 1.0), RowHit::Tab(0, false));
    }
}

#[test]
fn a_drop_before_the_first_tab_lands_after_the_inset() {
    let mut app = app(2.0, 3);
    let inset = controls(&mut app, 2.0, false);
    let first = app.documents.tabs.tabs()[0].id;
    let width = layout(&app).tab_width;
    press_tab(&mut app, first, 0, inset + width * 0.5);
    let drag = app.documents.strip.drag.as_mut().unwrap();
    drag.moving = true;
    drag.pointer_x = inset + 2.0;
    assert_eq!(drop_slot(&app), Some(0));
    let drag = app.documents.strip.drag.as_mut().unwrap();
    drag.pointer_x = inset + width * 1.1;
    assert_eq!(drop_slot(&app), Some(1));
}

#[test]
fn activation_reveals_the_tab_between_the_sidebar_and_the_button() {
    for scale in [1.0, 2.0] {
        for with_sidebar in [false, true] {
            let mut app = app(scale, 30);
            let dir = tempfile::tempdir().expect("tempdir");
            if with_sidebar {
                app.ui.sidebar =
                    crate::sidebar::SidebarState::open_folder(dir.path().to_path_buf());
                app.viewport.width -= app.sidebar_width();
            }
            controls(&mut app, scale, false);
            for index in [29, 0, 17] {
                let id = app.documents.tabs.tabs()[index].id;
                app.documents.active_tab = Some(id);
                super::follow_active(&mut app);
                let strip = layout(&app);
                assert_eq!(
                    strip.left,
                    app.sidebar_width().max(app.documents.strip.controls_inset)
                );
                let x = strip.left + index as f32 * strip.tab_width - app.documents.tabs.scroll();
                assert!(x >= strip.left - 0.01, "{index}: tab starts at {x}");
                assert!(
                    x + strip.tab_width <= strip.right() + 0.01,
                    "{index}: tab ends past the strip"
                );
                assert_eq!(row_hit(&app, x + 2.0), RowHit::Tab(index, false));
            }
        }
    }
}
