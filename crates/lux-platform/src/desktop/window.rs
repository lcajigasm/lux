use dpi::{PhysicalPosition, PhysicalSize};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

use crate::window::{PlatformWindow, SafeAreaInsets, WindowId};

pub(super) struct DesktopWindow {
    pub(super) window: winit::window::Window,
    pub(super) id: WindowId,
}

impl PlatformWindow for DesktopWindow {
    fn id(&self) -> WindowId {
        self.id
    }

    fn inner_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    fn safe_area_insets(&self) -> SafeAreaInsets {
        // Desktop windows have no notch or rounded-corner obstruction to
        // report; this is an accurate zero, not a placeholder.
        SafeAreaInsets::ZERO
    }

    fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    fn request_redraw(&self) {
        self.window.request_redraw();
    }

    fn pre_present_notify(&self) {
        self.window.pre_present_notify();
    }

    fn set_ime_allowed(&self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
    }

    fn set_ime_cursor_area(&self, position: PhysicalPosition<i32>, size: PhysicalSize<u32>) {
        self.window.set_ime_cursor_area(position, size);
    }
}

impl HasWindowHandle for DesktopWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.window.window_handle()
    }
}

impl HasDisplayHandle for DesktopWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.window.display_handle()
    }
}
