//! The window trait and the small value types around it.

use dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

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
    pub visible: bool,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        WindowAttributes {
            title: "Lux".to_string(),
            inner_size: None,
            min_inner_size: None,
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

/// A live window. Also a valid target for a `wgpu` surface via the
/// `raw-window-handle` supertraits.
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn id(&self) -> WindowId;
    fn inner_size(&self) -> PhysicalSize<u32>;
    fn scale_factor(&self) -> f64;
    fn safe_area_insets(&self) -> SafeAreaInsets;
    fn set_title(&self, title: &str);
    fn request_redraw(&self);
    /// Must be called right before presenting a frame, so the platform can
    /// synchronize compositing (matches `winit`'s `pre_present_notify`).
    fn pre_present_notify(&self);
    fn set_ime_allowed(&self, allowed: bool);
    /// Tells the platform where to anchor the IME composition window, in
    /// physical pixels relative to this window's origin.
    fn set_ime_cursor_area(&self, position: PhysicalPosition<i32>, size: PhysicalSize<u32>);
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
    }

    #[test]
    fn desktop_has_no_safe_area_to_avoid() {
        assert_eq!(SafeAreaInsets::ZERO, SafeAreaInsets::default());
    }
}
