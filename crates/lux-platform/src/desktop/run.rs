use crate::{PlatformError, PlatformEventHandler};

use super::app::DesktopApp;
use super::clipboard::ArboardClipboard;

/// Runs the desktop event loop, dispatching to `handler` until it calls
/// [`crate::PlatformContext::exit`] or every window closes. Blocks the
/// calling thread for the lifetime of the application — on macOS this must
/// be the process's main thread, same restriction `winit` itself imposes.
pub fn run<H: PlatformEventHandler + 'static>(handler: H) -> Result<(), PlatformError> {
    let event_loop =
        winit::event_loop::EventLoop::new().map_err(|e| PlatformError::Window(e.to_string()))?;
    let clipboard = ArboardClipboard::new()?;
    let mut app = DesktopApp::new(handler, clipboard);
    event_loop
        .run_app(&mut app)
        .map_err(|e| PlatformError::Window(e.to_string()))
}
