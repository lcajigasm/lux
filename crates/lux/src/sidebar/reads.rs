//! Directory reads for the sidebar, off the UI thread.
//!
//! Opening a folder shows its header at once with an unread tree; the
//! first level lands from the file-ops worker. Expanding a folder whose
//! entries were never read, and revealing a file below unread folders,
//! queue those reads the same way and finish when they land. Nothing here
//! touches the disk on the caller's thread.

use std::path::{Path, PathBuf};

use lux_ui::file_tree::{FileTree, Listing, Reveal, RootRead};

use super::SidebarState;
use super::worker::{self, Job};
use crate::app::App;

/// What to do once a set of listings is installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum After {
    /// Expand and select the row at this path.
    Expand(PathBuf),
    /// Reveal the open document again.
    Reveal,
}

fn submit(app: &mut App, job: Job) {
    let waker = app.ui.session.waker();
    app.ui.session.file_ops.submit(job, waker);
}

impl SidebarState {
    /// A sidebar on `path` with nothing read yet. Memory only; use
    /// [`open_root`] to read it.
    pub(crate) fn unread(path: PathBuf) -> Self {
        Self {
            tree: Some(FileTree::unread(&path)),
            workspace_root: Some(path),
            ..Self::default()
        }
    }
}

/// Opens `path` as the sidebar's folder. The header shows at once; the
/// first level of entries is read on the worker.
pub(crate) fn open_root(app: &mut App, path: PathBuf) {
    app.ui.sidebar = Some(SidebarState::unread(path.clone()));
    submit(app, Job::ReadRoot(path));
}

/// Adds `path` beside the open folder, read on the worker. `false` when
/// there is no open folder or it is already shown.
pub(crate) fn add_root(app: &mut App, path: PathBuf) -> bool {
    let added = app
        .ui
        .sidebar
        .as_mut()
        .is_some_and(|sidebar| sidebar.add_folder(path.clone()));
    if added {
        submit(app, Job::ReadRoot(path));
    }
    added
}

/// Installs a folder's first level into the open folder it belongs to. A
/// folder that could not be read is dropped, as a failed open always was.
pub(crate) fn apply_root(app: &mut App, read: RootRead) {
    let path = read.path().to_path_buf();
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return;
    };
    let unread =
        |tree: &FileTree| tree.root_path() == path && tree.unread_dir(tree.root()).is_some();
    if sidebar.tree.as_ref().is_some_and(unread) {
        let installed = sidebar.tree.as_mut().map(|tree| tree.install_root(read));
        if matches!(installed, Some(Err(_))) && sidebar.extra.is_empty() {
            app.ui.sidebar = None;
            app.update_viewport_dimensions();
        }
    } else if let Some(index) = sidebar.extra.iter().position(|f| unread(&f.tree))
        && sidebar.extra[index].tree.install_root(read).is_err()
    {
        sidebar.extra.remove(index);
    }
    app.scheduler.invalidate_all();
}

/// Reads `dir` on the worker, then expands and selects its row.
pub(crate) fn expand_later(app: &mut App, dir: PathBuf) {
    submit(
        app,
        Job::List {
            dirs: vec![dir.clone()],
            then: After::Expand(dir),
        },
    );
}

/// Reads `dirs` on the worker, then reveals the open document again.
pub(crate) fn reveal_later(app: &mut App, dirs: Vec<PathBuf>) {
    submit(
        app,
        Job::List {
            dirs,
            then: After::Reveal,
        },
    );
}

/// Installs listings into whichever open folder holds each directory,
/// then finishes the expand or reveal that asked for them.
pub(crate) fn apply_listed(app: &mut App, listings: Vec<(PathBuf, Listing)>, then: Option<After>) {
    if let Some(sidebar) = app.ui.sidebar.as_mut() {
        for (dir, listing) in listings {
            if let Some((_, tree)) = worker::tree_for(sidebar, &dir) {
                tree.install_listing(&dir, listing);
            }
        }
    }
    match then {
        Some(After::Expand(dir)) => expand_at(app, &dir),
        Some(After::Reveal) => super::reveal_open_document(app),
        None => {}
    }
    app.scheduler.invalidate_all();
}

fn expand_at(app: &mut App, dir: &Path) {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return;
    };
    let Some((root, tree)) = worker::tree_for(sidebar, dir) else {
        return;
    };
    let Some(id) = tree.find_id(dir) else {
        return;
    };
    tree.expand_loaded(id);
    if root == 0 {
        sidebar.selected_node = Some(id);
    }
}

/// Applies a finished drag-move. Like the synchronous move it replaces,
/// the destination folder ends up loaded and expanded, with the moved row
/// in it; the row keeps its id when the destination was already loaded.
pub(crate) fn note_moved(
    tree: &mut FileTree,
    from: &Path,
    to: &Path,
    kind: lux_ui::file_tree::NodeKind,
    listing: Option<Listing>,
) -> Option<lux_ui::file_tree::NodeId> {
    let to_dir = to.parent()?;
    let unread = tree
        .find_id(to_dir)
        .is_some_and(|dir| tree.unread_dir(dir).is_some());
    if unread && let Some(listing) = listing {
        tree.note_removed(from);
        tree.install_listing(to_dir, listing);
    }
    let id = tree.note_moved(from, to, kind);
    if let Some(dir) = tree.find_id(to_dir) {
        tree.expand_loaded(dir);
    }
    id
}

/// [`FileTree::reveal_loaded`], queueing the reads it asks for.
pub(crate) fn reveal(app: &mut App, tree_reveal: Reveal) -> Option<lux_ui::file_tree::NodeId> {
    match tree_reveal {
        Reveal::Found(id) => Some(id),
        Reveal::Missing => None,
        Reveal::Read(dirs) => {
            reveal_later(app, dirs);
            None
        }
    }
}
