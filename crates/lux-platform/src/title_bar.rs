//! Geometry and settings of a title bar the app draws itself.
//!
//! On macOS the window's content extends under a transparent title bar,
//! so the editor's tab row is the title bar. The traffic lights stay
//! where AppKit draws them, vertically centered on that row, and the tabs
//! start to the right of them. These functions hold the arithmetic so it
//! is tested on every platform; `desktop` feeds them real AppKit frames.

/// Leading space the traffic lights take, in points, when AppKit does not
/// report the zoom button's frame. The width of the three standard
/// buttons plus their margins.
pub const FALLBACK_LEADING_INSET_POINTS: f64 = 78.0;

/// Space between the zoom (green) button's right edge and the first tab.
pub const BUTTON_TRAILING_GAP_POINTS: f64 = 12.0;

/// Leading inset of the tab row, in points.
///
/// `zoom_button_max_x` is the right edge of the zoom button in window
/// coordinates, when known. Fullscreen has no traffic lights, so no inset.
pub fn leading_inset_points(zoom_button_max_x: Option<f64>, fullscreen: bool) -> f64 {
    if fullscreen {
        return 0.0;
    }
    match zoom_button_max_x {
        Some(max_x) if max_x.is_finite() && max_x > 0.0 => max_x + BUTTON_TRAILING_GAP_POINTS,
        _ => FALLBACK_LEADING_INSET_POINTS,
    }
}

/// [`leading_inset_points`] in physical pixels at `scale`.
pub fn leading_inset_physical(points: f64, scale: f64) -> f64 {
    (points * scale).round()
}

/// Offset from the top (and, being centered, from the bottom) of a row of
/// `row_height` points at which a button `button_height` points tall sits
/// centered. Never negative.
pub fn centered_offset(row_height: f64, button_height: f64) -> f64 {
    ((row_height - button_height) / 2.0).max(0.0)
}

/// What a double click on the title bar does, per the user's macOS setting
/// (System Settings > Desktop & Dock > "Double-click a window's title bar to").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DoubleClickAction {
    /// Zoom (`Maximize`, and Sequoia's `Fill`). The system default.
    #[default]
    Zoom,
    Minimize,
    None,
}

impl DoubleClickAction {
    /// From the global defaults `AppleActionOnDoubleClick` and, when that is
    /// absent, the older `AppleMiniaturizeOnDoubleClick` boolean.
    pub fn from_defaults(action: Option<&str>, legacy_minimize: Option<bool>) -> Self {
        match action.map(str::trim) {
            Some("Minimize") => DoubleClickAction::Minimize,
            Some("None") => DoubleClickAction::None,
            Some("Maximize") | Some("Fill") => DoubleClickAction::Zoom,
            _ if legacy_minimize == Some(true) => DoubleClickAction::Minimize,
            _ => DoubleClickAction::Zoom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inset_clears_the_zoom_button() {
        assert_eq!(leading_inset_points(Some(66.0), false), 78.0);
        assert_eq!(leading_inset_points(Some(70.0), false), 82.0);
    }

    #[test]
    fn without_a_button_frame_the_inset_is_the_standard_78_points() {
        assert_eq!(leading_inset_points(None, false), 78.0);
        assert_eq!(leading_inset_points(Some(f64::NAN), false), 78.0);
        assert_eq!(leading_inset_points(Some(0.0), false), 78.0);
    }

    #[test]
    fn fullscreen_has_no_inset() {
        assert_eq!(leading_inset_points(Some(66.0), true), 0.0);
        assert_eq!(leading_inset_points(None, true), 0.0);
    }

    #[test]
    fn the_inset_scales_to_physical_pixels() {
        assert_eq!(leading_inset_physical(78.0, 1.0), 78.0);
        assert_eq!(leading_inset_physical(78.0, 2.0), 156.0);
        assert_eq!(leading_inset_physical(0.0, 2.0), 0.0);
    }

    #[test]
    fn buttons_center_on_the_tab_row() {
        assert_eq!(centered_offset(32.0, 16.0), 8.0);
        assert_eq!(centered_offset(32.0, 14.0), 9.0);
        assert_eq!(centered_offset(10.0, 16.0), 0.0);
    }

    #[test]
    fn double_click_follows_the_user_setting() {
        type A = DoubleClickAction;
        assert_eq!(A::from_defaults(Some("Maximize"), None), A::Zoom);
        assert_eq!(A::from_defaults(Some("Fill"), None), A::Zoom);
        assert_eq!(A::from_defaults(Some("Minimize"), None), A::Minimize);
        assert_eq!(A::from_defaults(Some("None"), Some(true)), A::None);
        assert_eq!(A::from_defaults(None, Some(true)), A::Minimize);
        assert_eq!(A::from_defaults(None, Some(false)), A::Zoom);
        assert_eq!(A::from_defaults(None, None), A::Zoom);
    }
}
