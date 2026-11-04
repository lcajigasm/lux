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
    crate::sidebar::reveal_open_document(app);
}

pub(crate) fn busy(app: &App) -> bool {
    app.documents.pending.is_some()
}

pub(crate) fn pump(app: &mut App, ctx: &dyn PlatformContext) {
    if app.documents.pending.is_some() || app.documents.open_queue.is_empty() {
        return;
    }
    let path = app.documents.open_queue.remove(0);
    app.request_document(ctx, crate::document::Request::Open(Some(path)));
}

/// `--columns 2` lays the files out once every queued open has finished.
pub(crate) fn apply_launch_columns(app: &mut App) {
    if app.ui.split.columns_after_open != 2 {
        return;
    }
    if app.documents.pending.is_some() || !app.documents.open_queue.is_empty() {
        return;
    }
    if app.documents.active_tab.is_none() {
        return;
    }
    app.ui.split.columns_after_open = 0;
    app.set_layout(crate::split::LayoutPreset::Columns2);
}
