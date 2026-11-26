//! Pressing and dragging in the minimap. A press on the viewport box grabs
//! it where it was pressed; a press elsewhere centers the editor on that
//! line first, then grabs the box there. While the button is held, every
//! pointer move (the same `MouseMoved` path the sidebar sash uses) moves
//! the box's top edge with the pointer and scrolls the editor to match:
//! one minimap row is `lines_per_row` document lines.

use crate::app::App;
use crate::minimap::MINIMAP_ROW_HEIGHT;

/// Where the minimap is on screen, in physical pixels — the same rectangle
/// `render::minimap` paints.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Geometry {
    pub(crate) left: f32,
    pub(crate) right: f32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    pub(crate) row_h: f32,
    pub(crate) rows: usize,
}

fn window_width(app: &App) -> f32 {
    if let Some(gpu) = &app.gpu_window {
        return gpu.window.inner_size().width as f32;
    }
    app.sidebar_width() + app.viewport.width + app.minimap_width()
}

pub(crate) fn geometry(app: &App) -> Option<Geometry> {
    let width = app.minimap_width();
    if width <= 0.0 {
        return None;
    }
    let window_w = window_width(app);
    let top = app.tab_bar_height();
    // Painted the same way: the minimap ends where the editor does, above
    // the find bar and the dock.
    let bottom = top + app.viewport.height;
    let row_h = MINIMAP_ROW_HEIGHT * app.scale_factor;
    let rows = ((bottom - top) / row_h).floor().max(0.0) as usize;
    (rows > 0).then_some(Geometry {
        left: window_w - width,
        right: window_w,
        top,
        bottom,
        row_h,
        rows,
    })
}

fn total_visual(app: &App) -> usize {
    let total = app.buffer.len_lines();
    app.fold_state.total_visual_lines(total).max(1)
}

fn line_height(app: &App) -> f32 {
    app.editor_view.metrics().line_height.max(1.0)
}

/// Screen pixels the viewport box moves per editor pixel scrolled.
fn box_scale(app: &App, geo: &Geometry) -> f32 {
    let per_row = lux_ui::minimap::lines_per_row(total_visual(app), geo.rows) as f32;
    geo.row_h / (per_row * line_height(app))
}

/// The viewport box's top edge and height on screen.
pub(crate) fn box_span(app: &App, geo: &Geometry) -> (f32, f32) {
    let (offset, height) = box_extent(
        app.viewport.scroll_y,
        app.viewport.height,
        box_scale(app, geo),
        geo.row_h,
        geo.bottom - geo.top,
    );
    (geo.top + offset, height)
}

/// The viewport box below the minimap's top edge, as (offset, height):
/// the editor's scroll offset and height times `scale` (minimap pixels per
/// editor pixel), never shorter than two rows so it stays grabbable, and
/// kept inside a strip `strip_h` tall (its top at least one row above the
/// bottom). The paint and the hit test both use this, so the box is where
/// it's drawn.
pub(crate) fn box_extent(
    scroll_y: f32,
    viewport_h: f32,
    scale: f32,
    row_h: f32,
    strip_h: f32,
) -> (f32, f32) {
    let offset = (scroll_y * scale).min(strip_h - row_h).max(0.0);
    let height = (viewport_h * scale).max(row_h * 2.0).min(strip_h - offset);
    (offset, height)
}

fn max_scroll(app: &App) -> f32 {
    (total_visual(app) - 1) as f32 * line_height(app)
}

fn scroll_to(app: &mut App, scroll_y: f32) -> bool {
    let scroll_y = scroll_y.clamp(0.0, max_scroll(app).max(0.0));
    if (scroll_y - app.viewport.scroll_y).abs() < 0.01 {
        return false;
    }
    app.viewport.scroll_y = scroll_y;
    app.scheduler.invalidate_all();
    true
}

/// A left press at `(x, y)`. Returns whether the minimap took it.
pub(crate) fn press(app: &mut App, x: f32, y: f32) -> bool {
    let Some(geo) = geometry(app) else {
        return false;
    };
    if x < geo.left || x > geo.right || y < geo.top || y >= geo.bottom {
        return false;
    }
    let (box_top, box_h) = box_span(app, &geo);
    if y < box_top || y >= box_top + box_h {
        center_on_row(app, &geo, ((y - geo.top) / geo.row_h).floor() as usize);
    }
    let (box_top, _) = box_span(app, &geo);
    app.ui.minimap.drag = Some(y - box_top);
    true
}

fn center_on_row(app: &mut App, geo: &Geometry, row: usize) {
    let total_lines = app.buffer.len_lines();
    let target_line =
        lux_ui::minimap::line_for_row_with_folds(total_lines, geo.rows, row, Some(&app.fold_state));
    let visual = app
        .fold_state
        .buffer_to_visual_line(target_line)
        .unwrap_or(0);
    let target = visual as f32 * line_height(app) - app.viewport.height * 0.5;
    scroll_to(app, target.max(0.0));
}

/// A pointer move. While the box is held, scrolls the editor so the box's
/// top stays where the pointer holds it. Returns whether it scrolled.
pub(crate) fn update(app: &mut App) -> bool {
    let Some(grab) = app.ui.minimap.drag else {
        return false;
    };
    let Some(geo) = geometry(app) else {
        app.ui.minimap.drag = None;
        return false;
    };
    let box_top = app.mouse.position.y as f32 - grab;
    let scroll = (box_top - geo.top) / box_scale(app, &geo);
    let moved = scroll_to(app, scroll);
    if moved && let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
    moved
}

pub(crate) fn release(app: &mut App) {
    app.ui.minimap.drag = None;
}

#[cfg(test)]
#[path = "drag_tests.rs"]
mod tests;
