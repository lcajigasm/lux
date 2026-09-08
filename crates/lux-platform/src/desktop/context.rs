use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use winit::event_loop::{ActiveEventLoop, EventLoopProxy};

use crate::{
    Clipboard, FileDialogs, PlatformContext, PlatformError, PlatformWindow, WindowAttributes,
    WindowId,
};

use super::app::{AccessibilityEvent, WindowRecord};
use super::clipboard::ArboardClipboard;
use super::dialog::RfdFileDialogs;
use super::window::DesktopWindow;

pub(super) struct DesktopContext<'a> {
    event_loop: &'a ActiveEventLoop,
    clipboard: &'a ArboardClipboard,
    dialogs: &'a RfdFileDialogs,
    windows: &'a RefCell<HashMap<winit::window::WindowId, WindowRecord>>,
    next_id: &'a Cell<u64>,
    accessibility_proxy: &'a EventLoopProxy<AccessibilityEvent>,
}

impl<'a> DesktopContext<'a> {
    pub(super) fn new(
        event_loop: &'a ActiveEventLoop,
        clipboard: &'a ArboardClipboard,
        dialogs: &'a RfdFileDialogs,
        windows: &'a RefCell<HashMap<winit::window::WindowId, WindowRecord>>,
        next_id: &'a Cell<u64>,
        accessibility_proxy: &'a EventLoopProxy<AccessibilityEvent>,
    ) -> Self {
        DesktopContext {
            event_loop,
            clipboard,
            dialogs,
            windows,
            next_id,
            accessibility_proxy,
        }
    }
}

impl PlatformContext for DesktopContext<'_> {
    fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        let proxy = self.accessibility_proxy.clone();
        std::sync::Arc::new(move || {
            let _ = proxy.send_event(AccessibilityEvent::Background);
        })
    }

    fn background_dialogs(&self) -> std::sync::Arc<dyn FileDialogs> {
        let windows = self.windows.borrow();
        let dialogs = windows
            .values()
            .next()
            .map(|record| RfdFileDialogs::with_parent(&record.window))
            .unwrap_or_default();
        std::sync::Arc::new(dialogs)
    }
    fn create_window(
        &self,
        attrs: WindowAttributes,
    ) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        let requested_visible = attrs.visible;
        let mut winit_attrs = winit::window::WindowAttributes::default()
            .with_title(attrs.title)
            // `accesskit_winit::Adapter` must be built before the window is
            // ever shown (see `docs/adr/0013-platform-accessibility-adapter-wiring.md`),
            // so every window is created invisible and made visible right
            // after the adapter attaches, regardless of what was requested.
            .with_visible(false);
        if let Some(size) = attrs.inner_size {
            winit_attrs = winit_attrs.with_inner_size(size);
        }
        if let Some(min_size) = attrs.min_inner_size {
            winit_attrs = winit_attrs.with_min_inner_size(min_size);
        }

        let window = Rc::new(
            self.event_loop
                .create_window(winit_attrs)
                .map_err(|e| PlatformError::Window(e.to_string()))?,
        );

        let id = WindowId(self.next_id.get());
        self.next_id.set(self.next_id.get() + 1);

        let adapter = accesskit_winit::Adapter::with_event_loop_proxy(
            self.event_loop,
            &window,
            self.accessibility_proxy.clone(),
        );
        let accessibility = Rc::new(RefCell::new(adapter));

        self.windows.borrow_mut().insert(
            window.id(),
            WindowRecord {
                id,
                window: Rc::clone(&window),
                accessibility: Rc::clone(&accessibility),
            },
        );

        if requested_visible {
            window.set_visible(true);
        }

        Ok(Box::new(DesktopWindow {
            window,
            id,
            accessibility,
        }))
    }

    fn clipboard(&self) -> &dyn Clipboard {
        self.clipboard
    }

    fn file_dialogs(&self) -> &dyn FileDialogs {
        self.dialogs
    }

    fn exit(&self) {
        self.event_loop.exit();
    }

    fn request_wakeup(&self, at: std::time::Instant) {
        self.event_loop
            .set_control_flow(winit::event_loop::ControlFlow::WaitUntil(at));
    }
}
