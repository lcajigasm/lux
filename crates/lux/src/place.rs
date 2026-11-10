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

/// Records the inner size from a `Resized` event. This does not ask the
/// window for its zoom state or its screen: those AppKit calls from inside
/// `windowDidResize` rewrite the style mask and drop the next wheel event.
pub(crate) fn note_resized(app: &mut App, width: u32, height: u32) {
    let frame = frame_mut(app);
    frame.width = width.max(1);
    frame.height = height.max(1);
}

/// Records the outer origin from a `WindowMoved` event.
pub(crate) fn note_moved(app: &mut App, x: i32, y: i32) {
    let frame = frame_mut(app);
    frame.x = x;
    frame.y = y;
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

fn frame_mut(app: &mut App) -> &mut WindowFrame {
    app.ui.session.frame.get_or_insert_with(|| WindowFrame {
        x: 0,
        y: 0,
        width: 1000,
        height: 700,
        monitor_name: String::new(),
        monitor_x: 0,
        monitor_y: 0,
        scale_millis: 1000,
        fullscreen: false,
        maximized: false,
    })
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

#[cfg(test)]
mod tests {
    use super::{note_moved, note_resized};

    #[test]
    fn resize_and_move_are_recorded_without_a_window() {
        let mut app = crate::app::App::new(
            include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
        );
        note_resized(&mut app, 640, 480);
        note_moved(&mut app, 12, 34);
        let frame = app.ui.session.frame.expect("frame");
        assert_eq!(
            (frame.x, frame.y, frame.width, frame.height),
            (12, 34, 640, 480)
        );
        assert!(!frame.maximized);
        assert!(!frame.fullscreen);
    }
}
