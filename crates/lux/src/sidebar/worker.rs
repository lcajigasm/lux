//! Sidebar filesystem operations run here, never on the UI thread: create,
//! rename, duplicate, move to trash, drag-move, and the directory reads
//! that open a folder and expand or reveal a row (see `reads`). Each job runs on its own short
//! thread, sends one `Done` back, and wakes the platform loop. `poll`
//! applies the result to whichever open folder holds the path. The
//! watcher may report the same change first; applying it twice is a
//! no-op. The jobs live in the workspace session, so a result still
//! reaches the open tabs after the last folder is closed.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use lux_platform::{FileDialogs, PlatformContext};
use lux_ui::file_tree::{FileTree, Listing, NodeId, NodeKind, RootRead, disk};

use super::SidebarState;
use super::pending::{self, Renames};
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
    /// Drag-move of `from` into the directory `to_dir`.
    Move {
        from: PathBuf,
        to_dir: PathBuf,
        kind: NodeKind,
    },
    /// The first level of a folder opened in the sidebar.
    ReadRoot(PathBuf),
    /// Directory listings, outermost first, then `then`.
    List {
        dirs: Vec<PathBuf>,
        then: super::reads::After,
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
    /// The rename did not happen; `from` is still the file's path.
    RenameFailed {
        from: PathBuf,
        message: String,
    },
    Trashed {
        path: PathBuf,
    },
    /// The user declined the trash confirmation.
    Kept,
    Failed(String),
    /// `listing` is the destination folder after the move, so a folder
    /// never expanded before shows the moved entry once it opens.
    Moved {
        from: PathBuf,
        to: PathBuf,
        kind: NodeKind,
        listing: Option<Listing>,
    },
    RootRead(Box<RootRead>),
    /// Every listing read before the first failure. `then` runs only when
    /// all of them were read.
    Listed {
        listings: Vec<(PathBuf, Listing)>,
        then: Option<super::reads::After>,
    },
}

pub(crate) struct Jobs {
    tx: Sender<Done>,
    rx: Receiver<Done>,
    /// Renames in flight, and the saves waiting for them.
    pub(crate) renames: Renames,
    /// Jobs submitted whose result `drain` has not returned yet.
    in_flight: usize,
    /// Tests: the next job waits for one message here before it runs.
    #[cfg(test)]
    pub(crate) hold: Option<Receiver<()>>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            renames: Renames::default(),
            in_flight: 0,
            #[cfg(test)]
            hold: None,
        }
    }
}

impl Jobs {
    pub(crate) fn submit(&mut self, job: Job, wake: Option<Wake>) {
        let renaming = match &job {
            Job::Rename { from, .. } | Job::Move { from, .. } => Some(from.clone()),
            _ => None,
        };
        if let Some(from) = &renaming {
            self.renames.begin(from.clone());
        }
        #[cfg(test)]
        let hold = self.hold.take();
        let tx = self.tx.clone();
        let failed = |message: String| match renaming.clone() {
            Some(from) => Done::RenameFailed { from, message },
            None => Done::Failed(message),
        };
        let on_panic = failed("File operation failed".into());
        // A failed spawn still sends one result below.
        self.in_flight += 1;
        let spawned = std::thread::Builder::new()
            .name("lux-sidebar-op".into())
            .spawn(move || {
                #[cfg(test)]
                if let Some(hold) = hold {
                    let _ = hold.recv();
                }
                let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(job)))
                    .unwrap_or(on_panic);
                if tx.send(done).is_ok()
                    && let Some(wake) = wake
                {
                    wake();
                }
            });
        if let Err(error) = spawned {
            let _ = self
                .tx
                .send(failed(format!("Cannot start file operation: {error}")));
        }
    }

    pub(crate) fn drain(&mut self) -> Vec<Done> {
        let done: Vec<Done> = self.rx.try_iter().collect();
        self.in_flight = self.in_flight.saturating_sub(done.len());
        done
    }

    /// Jobs whose result has not been applied yet.
    #[cfg(test)]
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight
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
            Err(error) => Done::RenameFailed {
                from,
                message: format!("Cannot rename to {name}: {error}"),
            },
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
        Job::Move { from, to_dir, kind } => match disk::move_entry(&from, &to_dir) {
            Ok(to) => Done::Moved {
                listing: Listing::read(&to_dir).ok(),
                from,
                to,
                kind,
            },
            Err(error) => Done::RenameFailed {
                from,
                message: format!("Move error: {error}"),
            },
        },
        Job::ReadRoot(path) => Done::RootRead(Box::new(RootRead::read(path))),
        Job::List { dirs, then } => {
            let mut listings = Vec::with_capacity(dirs.len());
            for dir in dirs {
                match Listing::read(&dir) {
                    Ok(listing) => listings.push((dir, listing)),
                    Err(_) => {
                        return Done::Listed {
                            listings,
                            then: None,
                        };
                    }
                }
            }
            Done::Listed {
                listings,
                then: Some(then),
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
pub(super) fn tree_for<'a>(
    sidebar: &'a mut SidebarState,
    path: &Path,
) -> Option<(usize, &'a mut FileTree)> {
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

/// Applies finished jobs, then runs any save that waited for a rename.
/// Called from the platform's background event.
pub(crate) fn poll(app: &mut App, ctx: &dyn PlatformContext) {
    let done = app.ui.session.file_ops.drain();
    if done.is_empty() {
        return;
    }
    for result in done {
        apply(app, ctx, result);
        app.scheduler.invalidate_all();
    }
    pending::replay(app, ctx);
}

/// Updates the folder that holds `path`, if it is still open.
fn note(app: &mut App, path: &Path, update: impl FnOnce(&mut FileTree) -> Option<NodeId>) {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return;
    };
    if let Some((root, tree)) = tree_for(sidebar, path) {
        let id = update(tree);
        select(sidebar, root, id);
    }
}

fn apply(app: &mut App, ctx: &dyn PlatformContext, done: Done) {
    match done {
        Done::Created { path, kind } => {
            note(app, &path, |tree| tree.note_created_id(&path, kind));
            if kind == NodeKind::File {
                app.request_document(ctx, crate::document::Request::Open(Some(path)));
            }
        }
        Done::Renamed { from, to, kind } => {
            note(app, &from, |tree| tree.note_renamed(&from, &to, kind));
            super::drag::retarget_paths(app, &from, &to);
            app.ui.session.file_ops.renames.finish(&from);
            app.documents.message = Some(format!("Renamed to {}", label(&to)));
        }
        Done::RenameFailed { from, message } => {
            app.ui.session.file_ops.renames.finish(&from);
            app.documents.message = Some(message);
        }
        Done::Duplicated { path } => {
            note(app, &path, |tree| {
                tree.note_created_id(&path, NodeKind::File)
            });
            app.documents.message = Some(format!("Duplicated: {}", label(&path)));
        }
        Done::Trashed { path } => {
            note(app, &path, |tree| {
                tree.note_removed(&path);
                None
            });
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
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
            }
            crate::session::mark_removed(app, &path);
            app.documents.message = Some(format!("Moved to trash: {}", path.display()));
        }
        Done::Kept => {}
        Done::Failed(message) => app.documents.message = Some(message),
        Done::Moved {
            from,
            to,
            kind,
            listing,
        } => {
            note(app, &from, |tree| {
                super::reads::note_moved(tree, &from, &to, kind, listing)
            });
            super::drag::retarget_paths(app, &from, &to);
            app.ui.session.file_ops.renames.finish(&from);
            app.documents.message = Some(format!("Moved {}", label(&to)));
        }
        Done::RootRead(read) => super::reads::apply_root(app, *read),
        Done::Listed { listings, then } => super::reads::apply_listed(app, listings, then),
    }
}
