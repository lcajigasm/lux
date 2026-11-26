//! The window trait and the small value types around it.

use dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::event::ColorScheme;

/// Identifies a window for the lifetime of the process. Backend-assigned;
/// callers should treat it as opaque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub(crate) u64);

/// Requested attributes for a new window.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowAttributes {
    pub title: String,
    /// `None` lets the platform choose a default size.
    pub inner_size: Option<LogicalSize<f64>>,
    pub min_inner_size: Option<LogicalSize<f64>>,
    /// Inner size in physical pixels. Wins over `inner_size` when both are set.
    pub physical_inner_size: Option<PhysicalSize<u32>>,
    /// Outer position in physical pixels, top-left origin.
    pub position: Option<PhysicalPosition<i32>>,
    pub maximized: bool,
    pub fullscreen: bool,
    pub visible: bool,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        WindowAttributes {
            title: "Lux".to_string(),
            inner_size: None,
            min_inner_size: None,
            physical_inner_size: None,
            position: None,
            maximized: false,
            fullscreen: false,
            visible: true,
        }
    }
}

/// The inset, in physical pixels, between a window's edge and the area safe
/// for interactive content — non-zero on devices with notches or rounded
/// corners (iPhone/iPad), always zero on desktop, which has no such
/// obstruction to report.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SafeAreaInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl SafeAreaInsets {
    pub const ZERO: SafeAreaInsets = SafeAreaInsets {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
}

/// One connected display, in physical pixels with a top-left origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// `scale_factor * 1000`.
    pub scale_millis: u32,
}

/// The pointer a window shows. The desktop backend maps each value onto
/// the system cursor. [`CursorShape::Arrow`] is the cursor outside every
/// zone and the one restored when the pointer leaves the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Arrow,
    Text,
    Hand,
    ResizeEastWest,
    ResizeNorthSouth,
}

/// A live window. Also a valid target for a `wgpu` surface via the
/// `raw-window-handle` supertraits.
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn id(&self) -> WindowId;
    fn inner_size(&self) -> PhysicalSize<u32>;
    fn scale_factor(&self) -> f64;
    /// OS appearance, when the backend knows it. `None` on platforms that
    /// do not report a scheme; the editor then keeps its last choice.
    fn color_scheme(&self) -> Option<ColorScheme> {
        None
    }
    fn safe_area_insets(&self) -> SafeAreaInsets;
    fn set_title(&self, title: &str);
    fn request_redraw(&self);
    /// Asks the OS to bring this window forward. A later `lux` command
    /// uses it after handing paths to the editor that is already running.
    fn focus(&self) {}
    /// Must be called right before presenting a frame, so the platform can
    /// synchronize compositing (matches `winit`'s `pre_present_notify`).
    fn pre_present_notify(&self);
    fn set_ime_allowed(&self, allowed: bool);
    /// Tells the platform where to anchor the IME composition window, in
    /// physical pixels relative to this window's origin.
    fn set_ime_cursor_area(&self, position: PhysicalPosition<i32>, size: PhysicalSize<u32>);
    /// Pushes an accessibility tree update to this window's real OS
    /// accessibility adapter (see
    /// `docs/adr/0013-platform-accessibility-adapter-wiring.md`). Call this
    /// in response to
    /// [`crate::PlatformEvent::AccessibilityInitialTreeRequested`] and again
    /// on every subsequent content/cursor/selection change.
    fn update_accessibility_tree(&self, update: accesskit::TreeUpdate);
    /// Sets whether the window should enter or exit borderless fullscreen mode.
    fn set_fullscreen(&self, fullscreen: bool) {
        let _ = fullscreen;
    }
    /// Returns true if the window is currently in fullscreen mode.
    fn is_fullscreen(&self) -> bool {
        false
    }
    /// Outer origin in physical pixels. `None` when the backend cannot
    /// report it (a window that has already closed, or a test double).
    fn outer_position(&self) -> Option<(i32, i32)> {
        None
    }
    fn set_outer_position(&self, x: i32, y: i32) {
        let _ = (x, y);
    }
    fn request_inner_size(&self, width: u32, height: u32) {
        let _ = (width, height);
    }
    fn set_maximized(&self, maximized: bool) {
        let _ = maximized;
    }
    fn is_maximized(&self) -> bool {
        false
    }
    fn current_monitor(&self) -> Option<Screen> {
        None
    }
    /// Content origin in physical pixels, top-left in screen space.
    fn inner_position(&self) -> Option<(i32, i32)> {
        None
    }
    /// Sets the pointer. A no-op where the backend has no pointer.
    fn set_cursor(&self, cursor: CursorShape) {
        let _ = cursor;
    }
    /// Physical pixels the window's own controls take at the leading end
    /// of the content's top row. Non-zero only where the content draws the
    /// title bar (macOS, outside fullscreen); zero where the system draws
    /// a separate title bar or none.
    fn title_bar_leading_inset(&self) -> f64 {
        0.0
    }
    /// Height, in logical points, of the content row that serves as the
    /// title bar. macOS centers the traffic lights on it.
    fn set_title_bar_height(&self, height: f64) {
        let _ = height;
    }
    /// The primary button went down on empty space of a content-drawn
    /// title bar. Behaves like the native title bar: a drag moves the
    /// window, a double click does what the user's system setting says.
    /// A no-op where the system draws the title bar.
    fn title_bar_pressed(&self) {}
    /// Color (sRGB) of the content row serving as the title bar, and
    /// whether it is dark. macOS paints the window background with it and
    /// draws the traffic lights for that appearance.
    fn set_title_bar_color(&self, rgb: [u8; 3], dark: bool) {
        let _ = (rgb, dark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_attributes_are_a_sensible_starting_window() {
        let attrs = WindowAttributes::default();
        assert_eq!(attrs.title, "Lux");
        assert!(attrs.visible);
        assert!(attrs.inner_size.is_none());
        assert!(attrs.position.is_none());
        assert!(!attrs.maximized);
        assert!(!attrs.fullscreen);
    }

    #[test]
    fn desktop_has_no_safe_area_to_avoid() {
        assert_eq!(SafeAreaInsets::ZERO, SafeAreaInsets::default());
    }
}
