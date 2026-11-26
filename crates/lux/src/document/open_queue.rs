//! Extra paths from the command line open after the first file, each as
//! its own tab. Disk reads stay on the document worker; this only starts
//! the next one when the previous worker has finished.

use lux_platform::PlatformContext;

use crate::app::App;

pub(crate) fn note_opened(app: &mut App) {
    let Some(path) = app.documents.path.clone() else {
        return;
    };
    eprintln!("Lux: opened {}", path.display());
    crate::cli::apply_opened(app);
    crate::probe::open_phase("apply");
    crate::probe::note_open_loaded();
    crate::sidebar::reveal_open_document(app);
}

pub(crate) fn busy(app: &App) -> bool {
    app.documents.pending.is_some()
}

/// A path dropped on the window. The worker opens it; a second drop waits
/// in the queue until that worker finishes.
pub(crate) fn enqueue(app: &mut App, ctx: &dyn PlatformContext, path: std::path::PathBuf) {
    if path.as_os_str().is_empty() {
        return;
    }
    app.documents.open_queue.push(path);
    pump(app, ctx);
}

/// Starts the next waiting request once the document worker is free: a
/// save that waited for a sidebar rename first, then the next queued open.
pub(crate) fn pump(app: &mut App, ctx: &dyn PlatformContext) {
    crate::sidebar::pending::replay(app, ctx);
    if app.documents.pending.is_some() || app.documents.open_queue.is_empty() {
        return;
    }
    let path = app.documents.open_queue.remove(0);
    app.request_document(ctx, crate::document::Request::Open(Some(path)));
}

/// Opens `path` on the document worker and moves the cursor to
/// `line`:`column` when it lands. Without a platform context the path waits
/// in the queue for the next document operation.
pub(crate) fn open_at(
    app: &mut App,
    ctx: Option<&dyn PlatformContext>,
    path: std::path::PathBuf,
    line: Option<u32>,
    column: Option<u32>,
) {
    if path.as_os_str().is_empty() {
        return;
    }
    crate::cli::place_cursor_on_open(&path, line, column);
    app.documents.open_queue.push(path);
    if let Some(ctx) = ctx {
        pump(app, ctx);
    }
}

#[cfg(test)]
#[path = "jump_tests.rs"]
mod jump_tests;
