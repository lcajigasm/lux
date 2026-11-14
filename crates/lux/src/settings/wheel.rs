//! Wheel input for the settings page. An open menu scrolls its own list.

use lux_platform::ScrollDelta;

use super::layout::layout_settings;
use super::panel::bounds_for;
use crate::app::App;

pub(crate) fn scroll_settings(app: &mut App, delta: ScrollDelta) -> bool {
    if app.ui.settings.panel.is_none() {
        return false;
    }
    if app
        .ui
        .settings
        .panel
        .as_ref()
        .is_some_and(|panel| panel.menu.is_some())
    {
        let steps = match delta {
            ScrollDelta::Lines { y, .. } => (y < 0.0) as isize - (y > 0.0) as isize,
            ScrollDelta::Pixels(position) => {
                let y = position.y as f32;
                (y < 0.0) as isize - (y > 0.0) as isize
            }
        };
        if steps != 0 {
            let bounds = bounds_for(app);
            let mut settings = std::mem::take(&mut app.ui.settings);
            settings.move_selection(bounds, steps);
            app.scheduler.invalidate_all();
            app.ui.settings = settings;
        }
        return true;
    }
    let scale = app.scale_factor;
    let mut settings = std::mem::take(&mut app.ui.settings);
    let scrolled = settings.scroll_by(delta, scale);
    if scrolled {
        let bounds = bounds_for(app);
        if let Some(panel) = settings.panel.as_mut() {
            let layout = layout_settings(panel, bounds);
            panel.scroll = layout.scroll;
        }
        app.scheduler.invalidate_all();
    }
    app.ui.settings = settings;
    scrolled
}
