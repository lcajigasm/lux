use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use winit::event_loop::ActiveEventLoop;

use crate::{
    Clipboard, FileDialogs, PlatformContext, PlatformError, PlatformWindow, WindowAttributes,
    WindowId,
};

use super::clipboard::ArboardClipboard;
use super::dialog::RfdFileDialogs;
use super::window::DesktopWindow;

pub(super) struct DesktopContext<'a> {
    event_loop: &'a ActiveEventLoop,
    clipboard: &'a ArboardClipboard,
    dialogs: &'a RfdFileDialogs,
    window_ids: &'a RefCell<HashMap<winit::window::WindowId, WindowId>>,
    next_id: &'a Cell<u64>,
}

impl<'a> DesktopContext<'a> {
    pub(super) fn new(
        event_loop: &'a ActiveEventLoop,
        clipboard: &'a ArboardClipboard,
        dialogs: &'a RfdFileDialogs,
        window_ids: &'a RefCell<HashMap<winit::window::WindowId, WindowId>>,
        next_id: &'a Cell<u64>,
    ) -> Self {
        DesktopContext {
            event_loop,
            clipboard,
            dialogs,
            window_ids,
            next_id,
        }
    }
}

impl PlatformContext for DesktopContext<'_> {
    fn create_window(
        &self,
        attrs: WindowAttributes,
    ) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        let mut winit_attrs = winit::window::WindowAttributes::default()
            .with_title(attrs.title)
            .with_visible(attrs.visible);
        if let Some(size) = attrs.inner_size {
            winit_attrs = winit_attrs.with_inner_size(size);
        }
        if let Some(min_size) = attrs.min_inner_size {
            winit_attrs = winit_attrs.with_min_inner_size(min_size);
        }

        let window = self
            .event_loop
            .create_window(winit_attrs)
            .map_err(|e| PlatformError::Window(e.to_string()))?;

        let id = WindowId(self.next_id.get());
        self.next_id.set(self.next_id.get() + 1);
        self.window_ids.borrow_mut().insert(window.id(), id);

        Ok(Box::new(DesktopWindow { window, id }))
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
}
