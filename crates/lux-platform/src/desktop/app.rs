use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};

use crate::event::PlatformEvent;
use crate::{PlatformEventHandler, WindowId};

use super::clipboard::ArboardClipboard;
use super::context::DesktopContext;
use super::convert;
use super::dialog::RfdFileDialogs;

/// The event loop's user-event payload: just a newtype around
/// `accesskit_winit::Event` so `DesktopApp` can implement
/// `ApplicationHandler<AccessibilityEvent>` — see
/// `docs/adr/0013-platform-accessibility-adapter-wiring.md`.
pub(super) struct AccessibilityEvent(pub(super) accesskit_winit::Event);

impl From<accesskit_winit::Event> for AccessibilityEvent {
    fn from(event: accesskit_winit::Event) -> Self {
        AccessibilityEvent(event)
    }
}

/// Per-window state `DesktopApp` itself must retain: the real `winit::window::Window`
/// (never handed out directly — callers get a `DesktopWindow` holding the
/// same `Rc`) and its accessibility adapter, which needs the raw `winit`
/// event and window on every `window_event`, before conversion to
/// `PlatformEvent`.
pub(super) struct WindowRecord {
    pub(super) id: WindowId,
    pub(super) window: Rc<winit::window::Window>,
    pub(super) accessibility: Rc<RefCell<accesskit_winit::Adapter>>,
}

pub(super) struct DesktopApp<H: PlatformEventHandler> {
    handler: H,
    clipboard: ArboardClipboard,
    dialogs: RfdFileDialogs,
    windows: RefCell<HashMap<winit::window::WindowId, WindowRecord>>,
    next_id: Cell<u64>,
    accessibility_proxy: EventLoopProxy<AccessibilityEvent>,
}

impl<H: PlatformEventHandler> DesktopApp<H> {
    pub(super) fn new(
        handler: H,
        clipboard: ArboardClipboard,
        accessibility_proxy: EventLoopProxy<AccessibilityEvent>,
    ) -> Self {
        DesktopApp {
            handler,
            clipboard,
            dialogs: RfdFileDialogs,
            windows: RefCell::new(HashMap::new()),
            next_id: Cell::new(1),
            accessibility_proxy,
        }
    }
}

// Each callback below builds its `DesktopContext` from individually
// borrowed fields (`&self.clipboard`, not `&self`) so the compiler can see
// that borrow as disjoint from the `&mut self.handler` call that follows —
// a helper method taking `&self` would borrow the whole struct and this
// wouldn't compile.
impl<H: PlatformEventHandler> ApplicationHandler<AccessibilityEvent> for DesktopApp<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.windows,
            &self.next_id,
            &self.accessibility_proxy,
        );
        self.handler.resumed(&ctx);
        self.handler.surfaces_available(&ctx);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.windows,
            &self.next_id,
            &self.accessibility_proxy,
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
        // Must run before our own handling, same requirement
        // `accesskit_winit::Adapter::process_event`'s own doc comment states.
        if let Some(record) = self.windows.borrow().get(&window_id) {
            record
                .accessibility
                .borrow_mut()
                .process_event(&record.window, &event);
        }

        let Some(id) = self.windows.borrow().get(&window_id).map(|r| r.id) else {
            return;
        };
        let Some(event) = convert::window_event(event) else {
            return;
        };
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.windows,
            &self.next_id,
            &self.accessibility_proxy,
        );
        self.handler.window_event(&ctx, id, event);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AccessibilityEvent) {
        let accesskit_winit::Event {
            window_id,
            window_event,
        } = event.0;
        let Some(id) = self.windows.borrow().get(&window_id).map(|r| r.id) else {
            return;
        };
        let event = match window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                PlatformEvent::AccessibilityInitialTreeRequested
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                PlatformEvent::AccessibilityActionRequested(request)
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                PlatformEvent::AccessibilityDeactivated
            }
        };
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.windows,
            &self.next_id,
            &self.accessibility_proxy,
        );
        self.handler.window_event(&ctx, id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.windows,
            &self.next_id,
            &self.accessibility_proxy,
        );
        self.handler.about_to_wait(&ctx);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        let ctx = DesktopContext::new(
            event_loop,
            &self.clipboard,
            &self.dialogs,
            &self.windows,
            &self.next_id,
            &self.accessibility_proxy,
        );
        self.handler.exiting(&ctx);
    }
}
