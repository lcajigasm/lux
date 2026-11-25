//! Trackpad pinch-to-zoom. Drives the same editor font size as
//! Command+= / Command+- / Command+0 (`crate::scale`).
//!
//! macOS reports a pinch as a stream of small magnification deltas. They
//! are multiplied into one factor for the whole gesture, and the font moves
//! in whole-point steps from the size the gesture started at, the same step
//! as Command+=. Layout is rebuilt only when that step changes, not on every
//! event, so a slow pinch does not reshape the buffer at the event rate.

use lux_platform::TouchPhase;

use crate::app::App;
use crate::scale::{MAX_FONT, MIN_FONT};

/// One pinch gesture, from `Started` to `Ended` or `Cancelled`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PinchZoom {
    /// Logical font size when the gesture began.
    base: f32,
    /// Product of `1 + delta` over the gesture, clamped so that the size it
    /// implies stays inside the zoom range. Pinching past a limit therefore
    /// does not have to be undone before the reverse pinch takes effect.
    magnification: f64,
}

impl PinchZoom {
    fn new(base: f32) -> Self {
        Self {
            base,
            magnification: 1.0,
        }
    }

    fn step(&mut self, delta: f64) {
        let base = f64::from(self.base.max(f32::EPSILON));
        let low = f64::from(MIN_FONT) / base;
        let high = f64::from(MAX_FONT) / base;
        self.magnification = (self.magnification * (1.0 + delta)).clamp(low, high);
    }

    /// The font size this gesture asks for, in whole points from `base`.
    fn target(&self) -> f32 {
        let scaled = f64::from(self.base) * self.magnification;
        self.base + (scaled - f64::from(self.base)).round() as f32
    }
}

/// Applies one pinch event and requests a redraw if the font changed.
pub(crate) fn on_pinch(app: &mut App, delta: f64, phase: TouchPhase) {
    if apply(app, delta, phase)
        && let Some(gpu_window) = &app.gpu_window
    {
        gpu_window.window.request_redraw();
    }
}

/// Returns whether the editor font size changed.
fn apply(app: &mut App, delta: f64, phase: TouchPhase) -> bool {
    if phase == TouchPhase::Started || app.mouse.pinch.is_none() {
        // A `Moved` without `Started` (the window gained focus mid-gesture)
        // starts from the current size.
        app.mouse.pinch = Some(PinchZoom::new(app.logical_font_size()));
    }
    let Some(mut pinch) = app.mouse.pinch else {
        return false;
    };
    pinch.step(delta);
    app.mouse.pinch = match phase {
        TouchPhase::Ended | TouchPhase::Cancelled => None,
        TouchPhase::Started | TouchPhase::Moved => Some(pinch),
    };
    app.set_logical_font_size(pinch.target())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::FONT_SIZE;
    use lux_core::buffer::Buffer;
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.buffer = Buffer::from_bytes(b"abcd\n").unwrap();
        app
    }

    fn line_width(app: &mut App) -> f32 {
        app.editor_view
            .build_frame(
                &app.buffer,
                &app.cursors,
                &app.viewport,
                Duration::ZERO,
                Some(&app.fold_state),
            )
            .lines[0]
            .layout
            .width()
    }

    fn pinch(app: &mut App, deltas: &[f64]) -> Vec<bool> {
        let mut changed = vec![apply(app, 0.0, TouchPhase::Started)];
        for &delta in deltas {
            changed.push(apply(app, delta, TouchPhase::Moved));
        }
        changed.push(apply(app, 0.0, TouchPhase::Ended));
        changed
    }

    #[test]
    fn pinching_out_grows_the_text_in_whole_points_and_in_reverses_it() {
        let mut app = app();
        app.set_display_scale(2.0);
        let before = line_width(&mut app);
        // 1.05^4 ≈ 1.2155 → 16 × 1.2155 ≈ 19.4 → 19pt.
        pinch(&mut app, &[0.05, 0.05, 0.05, 0.05]);
        assert_eq!(app.logical_font_size(), FONT_SIZE + 3.0);
        assert_eq!(app.editor_view.metrics().font_size, (FONT_SIZE + 3.0) * 2.0);
        assert!(line_width(&mut app) > before + 1.0);
        assert_eq!(app.mouse.pinch, None);

        pinch(&mut app, &[-0.1, -0.1]);
        // 19 × 0.81 = 15.39 → 19 - 3.61 → 15pt.
        assert_eq!(app.logical_font_size(), FONT_SIZE - 1.0);
    }

    #[test]
    fn small_deltas_do_not_relayout_until_a_whole_point_is_reached() {
        let mut app = app();
        // 16 × 1.01^3 ≈ 16.48: still 16pt, so nothing changes.
        let changed = pinch(&mut app, &[0.01, 0.01, 0.01]);
        assert!(changed.iter().all(|c| !c));
        assert_eq!(app.logical_font_size(), FONT_SIZE);
        // The fourth 1% step crosses 16.5 and changes the font once.
        apply(&mut app, 0.0, TouchPhase::Started);
        let steps: Vec<bool> = [0.01, 0.01, 0.01, 0.01]
            .iter()
            .map(|&d| apply(&mut app, d, TouchPhase::Moved))
            .collect();
        assert_eq!(steps, [false, false, false, true]);
        assert_eq!(app.logical_font_size(), FONT_SIZE + 1.0);
    }

    #[test]
    fn a_pinch_clamps_to_the_zoom_range_and_reverses_at_once() {
        let mut app = app();
        apply(&mut app, 0.0, TouchPhase::Started);
        for _ in 0..40 {
            apply(&mut app, 0.5, TouchPhase::Moved);
        }
        assert_eq!(app.logical_font_size(), MAX_FONT);
        // One reverse step from the clamp already shrinks the text.
        assert!(apply(&mut app, -0.1, TouchPhase::Moved));
        assert!(app.logical_font_size() < MAX_FONT);
        apply(&mut app, 0.0, TouchPhase::Ended);

        pinch(&mut app, &[-0.99, -0.99]);
        assert_eq!(app.logical_font_size(), MIN_FONT);
    }

    #[test]
    fn command_zero_resets_what_a_pinch_zoomed() {
        let mut app = app();
        pinch(&mut app, &[0.5]);
        assert_eq!(app.logical_font_size(), FONT_SIZE + 8.0);
        app.reset_font_zoom();
        assert_eq!(app.logical_font_size(), FONT_SIZE);
    }

    #[test]
    fn a_moved_without_started_begins_from_the_current_size() {
        let mut app = app();
        app.zoom_font(2.0);
        assert!(apply(&mut app, 0.1, TouchPhase::Moved));
        // 18 × 1.1 = 19.8 → 20pt.
        assert_eq!(app.logical_font_size(), FONT_SIZE + 4.0);
    }
}
