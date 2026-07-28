use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use winit::application::ApplicationHandler;
use winit::event_loop::ActiveEventLoop;

use crate::{PlatformEventHandler, WindowId};

use super::clipboard::ArboardClipboard;
use super::context::DesktopContext;
use super::convert;
use super::dialog::RfdFileDialogs;

pub(super) struct DesktopApp<H: PlatformEventHandler> {
    handler: H,
    clipboard: ArboardClipboard,
    dialogs: RfdFileDialogs,
    window_ids: RefCell<HashMap<winit::window::WindowId, WindowId>>,
    next_id: Cell<u64>,
}

impl<H: PlatformEventHandler> DesktopApp<H> {
    pub(super) fn new(handler: H, clipboard: ArboardClipboard) -> Self {
        DesktopApp {
            handler,
            clipboard,
            dialogs: RfdFileDialogs,
            window_ids: RefCell::new(HashMap::new()),
            next_id: Cell::new(1),
        }
    }
}

// Each callback below builds its `DesktopContext` from individually
// borrowed fields (`&self.clipboard`, not `&self`) so the compiler can see
// that borrow as disjoint from the `&mut self.handler` call that follows —
// a helper method taking `&self` would borrow the whole struct and this
// wouldn't compile.
impl<H: PlatformEventHandler> ApplicationHandler for DesktopApp<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.window_ids,
            &self.next_id,
        );
        self.handler.resumed(&ctx);
        self.handler.surfaces_available(&ctx);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.window_ids,
            &self.next_id,
        );
        self.handler.surfaces_lost(&ctx);
        self.handler.suspended(&ctx);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some(id) = self.window_ids.borrow().get(&window_id).copied() else {
            return;
        };
        let Some(event) = convert::window_event(event) else {
            return;
        };
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.window_ids,
            &self.next_id,
        );
        self.handler.window_event(&ctx, id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.window_ids,
            &self.next_id,
        );
        self.handler.about_to_wait(&ctx);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.window_ids,
            &self.next_id,
        );
        self.handler.exiting(&ctx);
    }
}
