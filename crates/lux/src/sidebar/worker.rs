//! Sidebar filesystem operations run here, never on the UI thread: create,
//! rename, duplicate, and move to trash. Each job runs on its own short
//! thread, sends one `Done` back, and wakes the platform loop. `poll`
//! applies the result to whichever open folder holds the path. The
//! watcher may report the same change first; applying it twice is a
//! no-op.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use lux_platform::{FileDialogs, PlatformContext};
use lux_ui::file_tree::{FileTree, NodeId, NodeKind, disk};

use super::SidebarState;
use crate::app::App;

type Wake = Arc<dyn Fn() + Send + Sync>;

pub(crate) enum Job {
    Create {
        parent: PathBuf,
        name: String,
        kind: NodeKind,
    },
    Rename {
        from: PathBuf,
        name: String,
        kind: NodeKind,
    },
    Duplicate {
        src: PathBuf,
    },
    /// Asks through the native dialog first, on the same worker.
    Trash {
        path: PathBuf,
        dialogs: Arc<dyn FileDialogs>,
    },
}

#[derive(Debug)]
pub(crate) enum Done {
    Created {
        path: PathBuf,
        kind: NodeKind,
    },
    Renamed {
        from: PathBuf,
        to: PathBuf,
        kind: NodeKind,
    },
    Duplicated {
        path: PathBuf,
    },
    Trashed {
        path: PathBuf,
    },
    /// The user declined the trash confirmation.
    Kept,
    Failed(String),
}

pub(crate) struct Jobs {
    tx: Sender<Done>,
    rx: Receiver<Done>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }
}

impl Jobs {
    pub(crate) fn submit(&self, job: Job, wake: Option<Wake>) {
        let tx = self.tx.clone();
        let spawned = std::thread::Builder::new()
            .name("lux-sidebar-op".into())
            .spawn(move || {
                let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(job)))
                    .unwrap_or_else(|_| Done::Failed("File operation failed".into()));
                if tx.send(done).is_ok()
                    && let Some(wake) = wake
                {
                    wake();
                }
            });
        if let Err(error) = spawned {
            let _ = self.tx.send(Done::Failed(format!(
                "Cannot start file operation: {error}"
            )));
        }
    }

    fn drain(&self) -> Vec<Done> {
        self.rx.try_iter().collect()
    }
}

fn run(job: Job) -> Done {
    match job {
        Job::Create { parent, name, kind } => match disk::create_entry(&parent, &name, kind) {
            Ok(path) => Done::Created { path, kind },
            Err(error) => Done::Failed(format!("Cannot create {name}: {error}")),
        },
        Job::Rename { from, name, kind } => match disk::rename_entry(&from, &name) {
            Ok(to) => Done::Renamed { from, to, kind },
            Err(error) => Done::Failed(format!("Cannot rename to {name}: {error}")),
        },
        Job::Duplicate { src } => match disk::duplicate_file(&src) {
            Ok(path) => Done::Duplicated { path },
            Err(error) => Done::Failed(format!("Cannot duplicate {}: {error}", label(&src))),
        },
        Job::Trash { path, dialogs } => {
            if !dialogs.confirm_trash(&label(&path)) {
                return Done::Kept;
            }
            match disk::move_to_trash(&path) {
                Ok(()) => Done::Trashed { path },
                Err(error) => {
                    Done::Failed(format!("Cannot move {} to trash: {error}", label(&path)))
                }
            }
        }
    }
}

fn label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The open folder whose tree holds `path`, as `(root index, tree)`.
fn tree_for<'a>(sidebar: &'a mut SidebarState, path: &Path) -> Option<(usize, &'a mut FileTree)> {
    sidebar
        .tree
        .iter_mut()
        .chain(sidebar.extra.iter_mut().map(|folder| &mut folder.tree))
        .enumerate()
        .find(|(_, tree)| tree.contains_path(path))
}

/// Selects `id` when it is in the header folder. `selected_node` does not
/// say which folder it belongs to, so rows elsewhere are not selected.
fn select(sidebar: &mut SidebarState, root: usize, id: Option<NodeId>) {
    if root == 0 && id.is_some() {
        sidebar.selected_node = id;
    }
}

/// Applies finished jobs. Called from the platform's background event.
pub(crate) fn poll(app: &mut App, ctx: &dyn PlatformContext) {
    let Some(done) = app.ui.sidebar.as_ref().map(|sidebar| sidebar.jobs.drain()) else {
        return;
    };
    for result in done {
        apply(app, ctx, result);
        app.scheduler.invalidate_all();
    }
}

fn apply(app: &mut App, ctx: &dyn PlatformContext, done: Done) {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return;
    };
    match done {
        Done::Created { path, kind } => {
            if let Some((root, tree)) = tree_for(sidebar, &path) {
                let id = tree.note_created_id(&path, kind);
                select(sidebar, root, id);
            }
            if kind == NodeKind::File {
                app.request_document(ctx, crate::document::Request::Open(Some(path)));
            }
        }
        Done::Renamed { from, to, kind } => {
            if let Some((root, tree)) = tree_for(sidebar, &from) {
                let id = tree.note_renamed(&from, &to, kind);
                select(sidebar, root, id);
            }
            super::drag::retarget_paths(app, &from, &to);
            app.documents.message = Some(format!("Renamed to {}", label(&to)));
        }
        Done::Duplicated { path } => {
            if let Some((root, tree)) = tree_for(sidebar, &path) {
                let id = tree.note_created_id(&path, NodeKind::File);
                select(sidebar, root, id);
            }
            app.documents.message = Some(format!("Duplicated: {}", label(&path)));
        }
        Done::Trashed { path } => {
            if let Some((_, tree)) = tree_for(sidebar, &path) {
                tree.note_removed(&path);
            }
            let gone = sidebar.selected_node.is_some_and(|id| {
                sidebar
                    .tree
                    .as_ref()
                    .and_then(|tree| tree.node_path(id))
                    .is_none()
            });
            if gone {
                sidebar.selected_node = None;
            }
            app.documents.message = Some(format!("Moved to trash: {}", path.display()));
        }
        Done::Kept => {}
        Done::Failed(message) => app.documents.message = Some(message),
    }
}
