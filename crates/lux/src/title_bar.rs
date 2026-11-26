//! The tab row as the window's title bar (ST-MAC-02).
//!
//! On macOS the window content runs under a transparent title bar, so the
//! tab row sits at the top of the window with the traffic lights at its
//! leading end. The platform reports how much room the lights take
//! (`PlatformWindow::title_bar_leading_inset`, zero in fullscreen and on
//! other platforms); the tab strip starts after it. Empty space in the row
//! behaves like a native title bar: drag to move, double-click for the
//! user's system action.

use crate::app::{App, TAB_BAR_HEIGHT};

/// Where tabs may go in the tab row, `(left, width)` in physical pixels.
/// The strip starts after the sidebar or after the window controls,
/// whichever reaches further, and ends where the editor ends.
pub(crate) fn strip_span(
    sidebar_width: f32,
    viewport_width: f32,
    controls_inset: f32,
) -> (f32, f32) {
    let left = sidebar_width.max(controls_inset);
    let width = (sidebar_width + viewport_width - left).max(0.0);
    (left, width)
}

/// Run once the window exists: tell the platform how tall the row is,
/// then take its inset.
pub(crate) fn attach(app: &mut App) {
    if let Some(gpu) = &app.gpu_window {
        gpu.window.set_title_bar_height(f64::from(TAB_BAR_HEIGHT));
    }
    sync_color(app);
    refresh(app);
}

/// Whether text and controls drawn on `color` should be light: true
/// when white contrasts with it more than black does.
pub(crate) fn is_dark(color: lux_ui::theme::Color) -> bool {
    use lux_ui::theme::{Color, contrast_ratio};
    contrast_ratio(color, Color::rgb(0xFF, 0xFF, 0xFF)) > contrast_ratio(color, Color::rgb(0, 0, 0))
}

/// The window behind the tab row follows the theme's tab-bar color.
pub(crate) fn sync_color(app: &App) {
    let color = app.chrome.theme.chrome.tab_bar;
    if let Some(gpu) = &app.gpu_window {
        gpu.window
            .set_title_bar_color([color.r, color.g, color.b], is_dark(color));
    }
}

/// Reads the controls' inset again. Run on resize, scale change, and
/// fullscreen transitions, which are when it changes.
pub(crate) fn refresh(app: &mut App) {
    let inset = app
        .gpu_window
        .as_ref()
        .map_or(0.0, |gpu| gpu.window.title_bar_leading_inset() as f32);
    if inset != app.documents.strip.controls_inset {
        app.documents.strip.controls_inset = inset;
        app.scheduler.invalidate_all();
    }
}

/// A native fullscreen transition finished, whoever started it.
/// Distraction-free mode lives in fullscreen, as in Sublime Text: leaving
/// fullscreen by the green button or Control-Command-F leaves it too.
pub(crate) fn fullscreen_changed(app: &mut App, fullscreen: bool) {
    if !fullscreen && app.is_distraction_free() {
        app.exit_distraction_free();
    }
    refresh(app);
    #[cfg(target_os = "macos")]
    crate::menu::macos::update_view_checkmarks(app);
    app.update_viewport_dimensions();
    app.scheduler.invalidate_all();
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
}

/// Primary press on empty tab-row space.
pub(crate) fn press_empty(app: &App) {
    if let Some(gpu) = &app.gpu_window {
        gpu.window.title_bar_pressed();
    }
}

/// Probe fields, in points: where the tab row sits in the window frame
/// (content top minus window top, zero under a unified title bar), the
/// controls' inset, and where the first tab starts.
pub(crate) fn probe_lines(app: &App) -> String {
    let scale = app.scale_factor.max(0.01);
    let (row_y, fullscreen) = app.gpu_window.as_ref().map_or((-1.0, false), |gpu| {
        let window = gpu.window;
        let row_y = match (window.inner_position(), window.outer_position()) {
            (Some((_, inner)), Some((_, outer))) => (inner - outer) as f32 / scale,
            _ => -1.0,
        };
        (row_y, window.is_fullscreen())
    });
    let (strip_left, _) = crate::tab_strip::span(app);
    format!(
        "tab_row_window_y_pt={row_y}\ntab_row_height_pt={}\ntitle_bar_inset_pt={}\ntab_strip_left_pt={}\nfullscreen={}\n",
        app.tab_bar_height() / scale,
        app.documents.strip.controls_inset / scale,
        strip_left / scale,
        u8::from(fullscreen),
    )
}

impl App {
    /// View > Enter Full Screen: native fullscreen in its own Space on
    /// macOS, borderless fullscreen elsewhere. Chrome stays as it is.
    pub(crate) fn toggle_full_screen(&mut self) {
        if let Some(gpu) = &self.gpu_window {
            let next = !gpu.window.is_fullscreen();
            gpu.window.set_fullscreen(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::strip_span;

    #[test]
    fn traffic_lights_take_the_appearance_of_every_bundled_tab_bar() {
        use lux_ui::theme::{Color, Theme};
        assert!(super::is_dark(Theme::MONOKAI.chrome.tab_bar));
        assert!(super::is_dark(Color::rgb(0x1E, 0x1F, 0x1C)));
        assert!(!super::is_dark(Color::rgb(0xF3, 0xF3, 0xF3)));
        assert!(!super::is_dark(Color::rgb(0xDD, 0xDD, 0xDD)));
    }

    #[test]
    fn without_controls_the_strip_starts_at_the_sidebar() {
        assert_eq!(strip_span(0.0, 1000.0, 0.0), (0.0, 1000.0));
        assert_eq!(strip_span(200.0, 800.0, 0.0), (200.0, 800.0));
    }

    #[test]
    fn the_strip_clears_the_traffic_lights_at_scale_1_and_2() {
        // 78 pt at scale 1, 156 px at scale 2; no sidebar.
        assert_eq!(strip_span(0.0, 1000.0, 78.0), (78.0, 922.0));
        assert_eq!(strip_span(0.0, 2000.0, 156.0), (156.0, 1844.0));
    }

    #[test]
    fn a_sidebar_wider_than_the_controls_keeps_its_edge() {
        // The lights sit over the sidebar's part of the row.
        assert_eq!(strip_span(250.0, 750.0, 78.0), (250.0, 750.0));
        assert_eq!(strip_span(100.0, 900.0, 156.0), (156.0, 844.0));
    }

    #[test]
    fn a_window_narrower_than_the_inset_has_no_room_for_tabs() {
        assert_eq!(strip_span(0.0, 50.0, 78.0), (78.0, 0.0));
    }
}
