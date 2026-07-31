use winit::event_loop::EventLoop;

use crate::{PlatformError, PlatformEventHandler};

use super::app::{AccessibilityEvent, DesktopApp};
use super::clipboard::ArboardClipboard;

/// Runs the desktop event loop, dispatching to `handler` until it calls
/// [`crate::PlatformContext::exit`] or every window closes. Blocks the
/// calling thread for the lifetime of the application — on macOS this must
/// be the process's main thread, same restriction `winit` itself imposes.
///
/// The event loop's user-event channel carries `accesskit_winit`'s own
/// events (see `docs/adr/0013-platform-accessibility-adapter-wiring.md`) —
/// an internal detail of this backend, never exposed to `handler`, which
/// only ever sees the resulting `PlatformEvent::Accessibility*` variants.
pub fn run<H: PlatformEventHandler + 'static>(handler: H) -> Result<(), PlatformError> {
    let event_loop = EventLoop::<AccessibilityEvent>::with_user_event()
        .build()
        .map_err(|e| PlatformError::Window(e.to_string()))?;
    let accessibility_proxy = event_loop.create_proxy();
    let clipboard = ArboardClipboard::new()?;
    let mut app = DesktopApp::new(handler, clipboard, accessibility_proxy);
    event_loop
        .run_app(&mut app)
        .map_err(|e| PlatformError::Window(e.to_string()))
}
