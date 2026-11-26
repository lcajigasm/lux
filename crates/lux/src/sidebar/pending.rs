//! Saves that wait for a sidebar rename. The rename runs on the sidebar
//! worker and the open tabs learn the new path only when its result
//! arrives. A save of a file under a rename in flight would write the old
//! path, so it waits here instead. When the result arrives, the tabs
//! already point at the new path (or still at the old one, if the rename
//! failed), and the save runs as it was asked.

use std::path::{Path, PathBuf};

use lux_platform::PlatformContext;
use lux_ui::tabs::TabId;

use crate::app::App;
use crate::document::Request;

#[derive(Default)]
pub(crate) struct Renames {
    /// Source paths of renames that have not reported back yet.
    from: Vec<PathBuf>,
    deferred: Vec<Deferred>,
}

enum Deferred {
    /// Save or Close of the tab that was active when it was asked.
    Active {
        tab: TabId,
        request: Request,
    },
    SaveAll,
}

impl Renames {
    pub(crate) fn begin(&mut self, from: PathBuf) {
        self.from.push(from);
    }

    pub(crate) fn finish(&mut self, from: &Path) {
        if let Some(index) = self.from.iter().position(|path| path == from) {
            self.from.remove(index);
        }
    }

    /// `true` while `path`, or a folder above it, is being renamed.
    pub(crate) fn covers(&self, path: &Path) -> bool {
        self.from.iter().any(|from| path.starts_with(from))
    }

    #[cfg(test)]
    pub(crate) fn waiting(&self) -> usize {
        self.deferred.len()
    }
}

/// `true` when `request` was kept until a rename of the active file ends.
pub(crate) fn defer_request(app: &mut App, request: &Request) -> bool {
    if !matches!(request, Request::Save { save_as: false } | Request::Close) {
        return false;
    }
    let renames = &mut app.ui.session.file_ops.renames;
    let covered = app
        .documents
        .path
        .as_deref()
        .is_some_and(|path| renames.covers(path));
    let Some(tab) = app.documents.active_tab.filter(|_| covered) else {
        return false;
    };
    renames.deferred.push(Deferred::Active {
        tab,
        request: request.clone(),
    });
    true
}

/// `true` when Save All was kept because a dirty file is being renamed.
pub(crate) fn defer_save_all(app: &mut App) -> bool {
    let renames = &mut app.ui.session.file_ops.renames;
    let active = app
        .documents
        .path
        .as_deref()
        .is_some_and(|path| renames.covers(path));
    let other = app.documents.entries.iter().any(|entry| {
        entry.is_dirty()
            && entry
                .path
                .as_deref()
                .is_some_and(|path| renames.covers(path))
    });
    if !active && !other {
        return false;
    }
    if !renames
        .deferred
        .iter()
        .any(|deferred| matches!(deferred, Deferred::SaveAll))
    {
        renames.deferred.push(Deferred::SaveAll);
    }
    true
}

/// Runs the kept requests once every rename has reported back, one at a
/// time: each waits for the document worker before the next starts.
pub(crate) fn replay(app: &mut App, ctx: &dyn PlatformContext) {
    loop {
        let busy = crate::document::open_queue::busy(app);
        let renames = &mut app.ui.session.file_ops.renames;
        if busy || !renames.from.is_empty() || renames.deferred.is_empty() {
            return;
        }
        match renames.deferred.remove(0) {
            Deferred::Active { tab, request } => {
                if !app
                    .documents
                    .entries
                    .iter()
                    .any(|entry| entry.tab_id == tab)
                {
                    continue;
                }
                app.switch_to_tab(tab);
                app.request_document(ctx, request);
            }
            Deferred::SaveAll => app.save_all(ctx),
        }
    }
}
