use std::cell::RefCell;
use std::rc::Rc;

use dpi::{PhysicalPosition, PhysicalSize};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

use crate::event::ColorScheme;
use crate::window::{PlatformWindow, SafeAreaInsets, WindowId};

pub(super) struct DesktopWindow {
    pub(super) window: Rc<winit::window::Window>,
    pub(super) id: WindowId,
    /// Shared with `DesktopApp`'s own window registry, which calls
    /// `process_event` on it before this window's owner ever sees a
    /// converted `PlatformEvent` — see
    /// `docs/adr/0013-platform-accessibility-adapter-wiring.md`.
    pub(super) accessibility: Rc<RefCell<accesskit_winit::Adapter>>,
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

    fn color_scheme(&self) -> Option<ColorScheme> {
        self.window.theme().map(super::convert::color_scheme)
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

    fn update_accessibility_tree(&self, update: accesskit::TreeUpdate) {
        self.accessibility.borrow_mut().update_if_active(|| update);
    }

    fn set_fullscreen(&self, fullscreen: bool) {
        if fullscreen {
            self.window
                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        } else {
            self.window.set_fullscreen(None);
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
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
