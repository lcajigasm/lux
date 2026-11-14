//! Created and Removed events update a loaded sidebar row.

use std::path::Path;

use lux_workspace::watcher::ChangeKind;

use crate::app::App;

pub(crate) fn apply(app: &mut App, path: &Path, kind: ChangeKind) -> bool {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return false;
    };
    let changed = {
        let Some(tree) = sidebar.tree.as_mut() else {
            return false;
        };
        match kind {
            ChangeKind::Created => tree.note_created(path),
            ChangeKind::Removed => tree.note_removed(path),
            _ => false,
        }
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::poll_file_watcher;
    use crate::sidebar::SidebarState;
    use std::time::{Duration, Instant};

    fn wait_until(app: &mut App, mut ready: impl FnMut(&SidebarState) -> bool) {
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

    #[test]
    fn a_created_file_appears_and_a_removed_file_disappears() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.txt"), "keep\n").unwrap();

        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.ui.sidebar = SidebarState::open_folder(dir.path().to_path_buf());
        app.ui.session.init_watcher(dir.path());
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
}
