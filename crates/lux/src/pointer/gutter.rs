//! Which editor gutter the pointer is over. Fold markers on open regions
//! show only there. The state changes on pointer events, and a redraw is
//! requested only when it flips, so nothing polls.

use super::{contains, divider, gutter_width, overlay};
use crate::app::App;

/// Top-left corner of the pane whose gutter is under the pointer, if any.
pub(super) fn hovered(app: &mut App) -> Option<[f32; 2]> {
    if !app.mouse.placed || app.ui.distraction_free.is_some() {
        return None;
    }
    let x = app.mouse.position.x as f32;
    let y = app.mouse.position.y as f32;
    if overlay(app, x, y).is_some() || divider(app, x, y).is_some() {
        return None;
    }
    let sidebar = app.sidebar_width();
    let rect = [
        sidebar,
        app.tab_bar_height(),
        app.viewport.width,
        app.viewport.height,
    ];
    let pane = if app.ui.split.is_active() {
        let geom = crate::split::compute_layout_geometry(&app.ui.split, rect, app.scale_factor);
        geom.pane_rects
            .into_iter()
            .find(|pane| contains(pane[0], pane[1], pane[2], pane[3], x, y))?
    } else if contains(rect[0], rect[1], rect[2], rect[3], x, y) {
        rect
    } else {
        return None;
    };
    (x < pane[0] + gutter_width(app)).then_some([pane[0], pane[1]])
}

/// Stores the hovered gutter and repaints only when it changed.
pub(super) fn update(app: &mut App, hover: Option<[f32; 2]>) {
    if app.mouse.gutter_hover == hover {
        return;
    }
    app.mouse.gutter_hover = hover;
    if let Some(gpu_window) = &app.gpu_window {
        gpu_window.window.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use lux_core::buffer::Buffer;

    use super::super::sync;
    use crate::app::App;

    fn app() -> App {
        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.buffer = Buffer::from_bytes(b"fn a() {\n    b();\n}\n").expect("text");
        app
    }

    fn move_to(app: &mut App, x: f32, y: f32) {
        app.mouse.placed = true;
        app.mouse.position.x = f64::from(x);
        app.mouse.position.y = f64::from(y);
        sync(app);
    }

    #[test]
    fn the_gutter_is_hovered_only_while_the_pointer_is_over_it() {
        let mut app = app();
        assert_eq!(app.mouse.gutter_hover, None);
        move_to(&mut app, 4.0, 80.0);
        let top = app.tab_bar_height();
        assert_eq!(app.mouse.gutter_hover, Some([0.0, top]), "over the digits");
        move_to(&mut app, 300.0, 80.0);
        assert_eq!(app.mouse.gutter_hover, None, "over the text");
        move_to(&mut app, 4.0, 80.0);
        super::super::leave(&mut app);
        assert_eq!(app.mouse.gutter_hover, None, "the pointer left the window");
    }

    #[test]
    fn stacked_panes_show_fold_markers_only_in_the_hovered_gutter() {
        use crate::render::text::GutterMarks;
        let mut app = app();
        app.set_layout(crate::split::LayoutPreset::Rows2);
        let rect = [
            app.sidebar_width(),
            app.tab_bar_height(),
            app.viewport.width,
            app.viewport.height,
        ];
        let geom = crate::split::compute_layout_geometry(&app.ui.split, rect, app.scale_factor);
        let [upper, lower] = [geom.pane_rects[0], geom.pane_rects[1]];
        assert_eq!(upper[0], lower[0], "the panes share the gutter's left edge");
        move_to(&mut app, upper[0] + 4.0, upper[1] + 20.0);
        let hover = app.mouse.gutter_hover;
        assert!(GutterMarks::new(None, hover, [upper[0], upper[1]]).hovered);
        assert!(!GutterMarks::new(None, hover, [lower[0], lower[1]]).hovered);
        move_to(&mut app, lower[0] + 4.0, lower[1] + 20.0);
        let hover = app.mouse.gutter_hover;
        assert!(!GutterMarks::new(None, hover, [upper[0], upper[1]]).hovered);
        assert!(GutterMarks::new(None, hover, [lower[0], lower[1]]).hovered);
    }

    #[test]
    fn distraction_free_mode_has_no_gutter_to_hover() {
        let mut app = app();
        app.toggle_distraction_free();
        move_to(&mut app, 4.0, 80.0);
        assert_eq!(app.mouse.gutter_hover, None);
    }
}
