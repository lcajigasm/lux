//! The hovered tab follows the strip, not just the pointer: after a wheel
//! scroll, a close, or a reorder, the next frame names the tab that is now
//! under the unmoved pointer.

use super::{follow_active, layout, pointer_moved, scroll_tabs};
use crate::app::{App, TAB_CLOSE_BTN_WIDTH};

fn app(tabs: usize) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    for index in 0..tabs.saturating_sub(app.documents.tabs.tabs().len()) {
        app.documents.tabs.open(format!("f{index}.txt"));
    }
    app
}

/// Puts the pointer on the close mark of the tab at strip index `index`
/// and lets the pointer move set the hover.
fn point_at_close(app: &mut App, index: usize) {
    let strip = layout(app);
    let close_w = TAB_CLOSE_BTN_WIDTH * app.scale_factor;
    let x = strip.left + (index + 1) as f32 * strip.tab_width
        - close_w * 0.5
        - app.documents.tabs.scroll();
    app.mouse.position.x = f64::from(x);
    app.mouse.position.y = 4.0;
    app.mouse.placed = true;
    pointer_moved(app);
}

fn id_at(app: &App, index: usize) -> lux_ui::tabs::TabId {
    app.documents.tabs.tabs()[index].id
}

#[test]
fn a_wheel_scroll_moves_the_hover_to_the_tab_under_the_pointer() {
    let mut app = app(20);
    // Settle the reveal of the active tab, then start from the left end.
    follow_active(&mut app);
    app.documents.tabs.set_scroll(0.0);
    point_at_close(&mut app, 0);
    assert_eq!(app.documents.strip.hover, Some((id_at(&app, 0), true)));
    let width = layout(&app).tab_width;
    scroll_tabs(&mut app, -width);
    follow_active(&mut app);
    assert_eq!(app.documents.strip.hover, Some((id_at(&app, 1), true)));
}

#[test]
fn closing_the_hovered_tab_hovers_the_one_that_slides_under_the_pointer() {
    let mut app = app(4);
    point_at_close(&mut app, 1);
    let closed = id_at(&app, 1);
    assert_eq!(app.documents.strip.hover, Some((closed, true)));
    app.documents.tabs.close(closed);
    follow_active(&mut app);
    assert_eq!(app.documents.strip.hover, Some((id_at(&app, 1), true)));
}

#[test]
fn closing_the_last_tab_under_the_pointer_clears_the_hover() {
    let mut app = app(3);
    point_at_close(&mut app, 2);
    let last = id_at(&app, 2);
    app.documents.tabs.close(last);
    follow_active(&mut app);
    assert_eq!(app.documents.strip.hover, None);
}

#[test]
fn a_reorder_hovers_the_tab_now_at_the_pointer() {
    let mut app = app(3);
    point_at_close(&mut app, 0);
    let first = id_at(&app, 0);
    assert!(app.reorder_tab(first, 2));
    follow_active(&mut app);
    assert_eq!(app.documents.strip.hover, Some((id_at(&app, 0), true)));
    assert_ne!(id_at(&app, 0), first);
}

#[test]
fn a_pointer_outside_the_window_hovers_nothing() {
    let mut app = app(3);
    point_at_close(&mut app, 0);
    crate::pointer::leave(&mut app);
    follow_active(&mut app);
    assert_eq!(app.documents.strip.hover, None);
}
