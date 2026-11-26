//! Pressing and dragging the minimap's viewport box through the app's own
//! pointer path (`pointer::sync` on every move, `pointer::release`).

use super::*;
use lux_core::buffer::Buffer;

fn app_with_lines(lines: usize) -> App {
    let font = include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec();
    let mut app = App::new(font);
    let text: String = (0..lines).map(|i| format!("line {i}\n")).collect();
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app.viewport.width = 800.0;
    app.viewport.height = 400.0;
    app
}

fn move_to(app: &mut App, x: f32, y: f32) {
    app.mouse.position = dpi::PhysicalPosition::new(x as f64, y as f64);
    app.mouse.placed = true;
    crate::pointer::sync(app);
}

#[test]
fn dragging_the_box_scrolls_in_proportion_to_the_pointer() {
    let mut app = app_with_lines(5_000);
    let geo = geometry(&app).expect("minimap on screen");
    let (box_top, _) = box_span(&app, &geo);
    let x = geo.left + 10.0;
    let y = box_top + 3.0;
    assert!(press(&mut app, x, y));
    assert_eq!(
        app.viewport.scroll_y, 0.0,
        "a press on the box must not jump"
    );

    move_to(&mut app, x, y + 40.0);
    let per_row = lux_ui::minimap::lines_per_row(app.buffer.len_lines(), geo.rows) as f32;
    let line_height = app.editor_view.metrics().line_height;
    let expected = 40.0 / geo.row_h * per_row * line_height;
    assert!(
        (app.viewport.scroll_y - expected).abs() < 0.5,
        "scrolled {} expected {expected}",
        app.viewport.scroll_y
    );
    let (moved_top, _) = box_span(&app, &geo);
    assert!((moved_top - (box_top + 40.0)).abs() < 0.01);

    move_to(&mut app, x, y + 20.0);
    assert!((app.viewport.scroll_y - expected / 2.0).abs() < 0.5);

    crate::pointer::release(&mut app);
    let settled = app.viewport.scroll_y;
    move_to(&mut app, x, y + 200.0);
    assert_eq!(
        app.viewport.scroll_y, settled,
        "released box must not follow"
    );
}

#[test]
fn a_drag_stops_at_the_ends_of_the_document() {
    let mut app = app_with_lines(5_000);
    let geo = geometry(&app).unwrap();
    let (box_top, _) = box_span(&app, &geo);
    let x = geo.left + 10.0;
    assert!(press(&mut app, x, box_top + 1.0));
    move_to(&mut app, x, -500.0);
    assert_eq!(app.viewport.scroll_y, 0.0);
    move_to(&mut app, x, geo.bottom + 10_000.0);
    let last = (app.buffer.len_lines() - 1) as f32 * app.editor_view.metrics().line_height;
    assert!((app.viewport.scroll_y - last).abs() < 0.5);
}

#[test]
fn a_press_below_the_box_centers_there_then_drags() {
    let mut app = app_with_lines(5_000);
    let geo = geometry(&app).unwrap();
    let x = geo.left + 10.0;
    let y = geo.top + geo.row_h * 100.5;
    assert!(press(&mut app, x, y));
    let centered = app.viewport.scroll_y;
    assert!(centered > 0.0);
    let (box_top, box_h) = box_span(&app, &geo);
    assert!(
        y >= box_top && y < box_top + box_h,
        "the box is under the pointer"
    );
    move_to(&mut app, x, y + 10.0);
    assert!(app.viewport.scroll_y > centered);
    crate::pointer::release(&mut app);
}

#[test]
fn a_press_outside_the_minimap_is_not_taken() {
    let mut app = app_with_lines(100);
    let geo = geometry(&app).unwrap();
    assert!(!press(&mut app, geo.left - 5.0, geo.top + 10.0));
    assert!(app.ui.minimap.drag.is_none());
}
