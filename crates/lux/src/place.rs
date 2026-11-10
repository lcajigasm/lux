//! Window frame: measure it on quit, and place the next window before it
//! is shown. The decision for a missing monitor lives in `lux-workspace`
//! so it can be tested without a display.

use lux_platform::{PlatformContext, Screen};
use lux_workspace::{MonitorRect, PlacedWindow, Session, WindowFrame, restore_window};

use crate::app::App;
use crate::session::default_session_path;

pub(crate) fn remember(app: &mut App) {
    if let Some(frame) = capture(app) {
        app.ui.session.frame = Some(frame);
    }
}

pub(crate) fn restored(ctx: &dyn PlatformContext) -> Option<PlacedWindow> {
    let path = default_session_path(None);
    let session = Session::load(&path).ok()?;
    let saved = session.windows.first()?.clone();
    let monitors: Vec<MonitorRect> = ctx.monitors().into_iter().map(monitor_rect).collect();
    Some(restore_window(&saved, &monitors))
}

/// Copies the frame into the global session when this quit wrote a
/// project session instead. A later launch creates the window before it
/// knows which project it will open, so the frame has to be in the
/// global file.
pub(crate) fn mirror_global(frame: &WindowFrame, written_to: &std::path::Path) {
    let global = default_session_path(None);
    if global == written_to {
        return;
    }
    let existed = global.exists();
    let mut session = Session::load(&global).unwrap_or_default();
    let clean = existed && !session.needs_recovery();
    session.windows.clear();
    session.windows.push(frame.clone());
    if let Some(parent) = global.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if clean {
        let _ = session.end_cleanly(&global);
    } else {
        let _ = session.save(&global);
    }
}

fn capture(app: &App) -> Option<WindowFrame> {
    let window = &app.gpu_window.as_ref()?.window;
    let (x, y) = window.outer_position()?;
    let inner = window.inner_size();
    let screen = window.current_monitor();
    Some(WindowFrame {
        x,
        y,
        width: inner.width.max(1),
        height: inner.height.max(1),
        monitor_name: screen
            .as_ref()
            .map(|screen| screen.name.clone())
            .unwrap_or_default(),
        monitor_x: screen.as_ref().map(|screen| screen.x).unwrap_or(0),
        monitor_y: screen.as_ref().map(|screen| screen.y).unwrap_or(0),
        scale_millis: screen
            .as_ref()
            .map(|screen| screen.scale_millis)
            .unwrap_or(1000),
        fullscreen: window.is_fullscreen(),
        maximized: window.is_maximized() && !window.is_fullscreen(),
    })
}

fn monitor_rect(screen: Screen) -> MonitorRect {
    MonitorRect {
        name: screen.name,
        x: screen.x,
        y: screen.y,
        width: screen.width,
        height: screen.height,
    }
}
