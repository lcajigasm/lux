//! Created and Removed events update a loaded sidebar row, in whichever
//! open folder holds the path. Memory only: the watcher thread already
//! knows whether the path is a directory, and each tree matches the path
//! against its root without resolving anything.

use lux_ui::file_tree::{FileTree, NodeKind};
use lux_workspace::watcher::{ChangeKind, FileChange};

use crate::app::App;

pub(crate) fn apply(app: &mut App, change: &FileChange) -> bool {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return false;
    };
    let kind = if change.is_dir {
        NodeKind::Directory
    } else {
        NodeKind::File
    };
    let mut changed = false;
    let trees = sidebar
        .tree
        .iter_mut()
        .chain(sidebar.extra.iter_mut().map(|folder| &mut folder.tree));
    for tree in trees {
        changed |= apply_to(tree, change, kind);
    }
    if !changed {
        return false;
    }
    if let Some(id) = sidebar.selected_node
        && sidebar
            .tree
            .as_ref()
            .and_then(|tree| tree.node_path(id))
            .is_none()
    {
        sidebar.selected_node = None;
    }
    true
}

fn apply_to(tree: &mut FileTree, change: &FileChange, kind: NodeKind) -> bool {
    if !tree.contains_path(&change.path) {
        return false;
    }
    match change.kind {
        ChangeKind::Created => tree.note_created(&change.path, kind),
        ChangeKind::Removed => tree.note_removed(&change.path),
        _ => false,
    }
}

/// Starts the app's watcher on `root` and returns once the detached watch
/// is in place, signalled through the same waker the platform loop uses.
#[cfg(test)]
pub(crate) fn watch_and_wait(app: &mut App, root: &std::path::Path) {
    let (tx, rx) = std::sync::mpsc::channel();
    app.ui.session.init_watcher_waking(
        root,
        std::sync::Arc::new(move || {
            let _ = tx.send(());
        }),
    );
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("the watch is in place");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::poll_file_watcher;
    use crate::sidebar::SidebarState;
    use std::time::{Duration, Instant};

    pub(super) fn wait_until(app: &mut App, mut ready: impl FnMut(&SidebarState) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            poll_file_watcher(app);
            if app.ui.sidebar.as_ref().is_some_and(&mut ready) {
                return;
            }
            assert!(Instant::now() < deadline, "the sidebar did not update");
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    fn app() -> App {
        App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
    }

    #[test]
    fn a_created_file_appears_and_a_removed_file_disappears() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.txt"), "keep\n").unwrap();

        let mut app = app();
        app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
        watch_and_wait(&mut app, dir.path());
        assert!(app.ui.session.watcher.is_some());

        let arrived = dir.path().join("arrived.txt");
        std::fs::write(&arrived, "arrived\n").unwrap();
        wait_until(&mut app, |sidebar| {
            super::super::tree_label(sidebar).contains("arrived.txt")
        });

        std::fs::remove_file(&arrived).unwrap();
        wait_until(&mut app, |sidebar| {
            let label = super::super::tree_label(sidebar);
            label.contains("keep.txt") && !label.contains("arrived.txt")
        });
    }

    /// ST-SIDE-02: a folder added beside the first one is watched too, and
    /// a directory created there is listed as a folder.
    #[test]
    fn a_second_open_folder_is_watched() {
        let alpha = tempfile::tempdir().unwrap();
        let beta = tempfile::tempdir().unwrap();
        std::fs::write(beta.path().join("keep.txt"), "keep\n").unwrap();

        let mut app = app();
        app.ui.sidebar = SidebarState::open_folder(alpha.path().to_path_buf());
        watch_and_wait(&mut app, alpha.path());
        let (tx, rx) = std::sync::mpsc::channel();
        // Only the watcher wakes this channel; see `watch_and_wait`.
        app.ui
            .session
            .watcher
            .as_mut()
            .expect("a watcher")
            .set_waker(std::sync::Arc::new(move || {
                let _ = tx.send(());
            }));
        assert!(app.add_sidebar_folder(beta.path().to_path_buf()));
        rx.recv_timeout(Duration::from_secs(5))
            .expect("the second watch is in place");

        std::fs::create_dir(beta.path().join("made")).unwrap();
        std::fs::write(beta.path().join("touched.txt"), "").unwrap();
        wait_until(&mut app, |sidebar| {
            let label = super::super::tree_label(sidebar);
            label.contains("touched.txt") && label.contains("made")
        });
        let sidebar = app.ui.sidebar.as_ref().unwrap();
        let tree = &sidebar.extra[0].tree;
        let made = tree.find_id(&beta.path().join("made")).unwrap();
        assert_eq!(tree.node_kind(made), Some(NodeKind::Directory));
    }
}
