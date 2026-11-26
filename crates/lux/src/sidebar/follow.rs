//! Keeps the sidebar selection on the active tab's file, the way the tab
//! strip keeps the active tab in view.
//!
//! Only rows the tree has already listed are considered: expanding a
//! collapsed folder would read the disk, and this runs on the UI thread.
//! A file under a collapsed folder clears the selection instead of
//! leaving it on some other file. Keyboard focus is never moved.

use super::{SIDEBAR_HEADER_HEIGHT, SIDEBAR_ROW_HEIGHT, SidebarState, listed_rows};
use crate::app::App;

/// Before a frame: the tab strip scrolls to the active tab and the
/// sidebar selects its file.
pub(crate) fn follow_tabs(app: &mut App) {
    crate::tab_strip::follow_active(app);
    follow_active_tab(app);
}

/// Moves the selection when the active tab changed since the last call.
pub(crate) fn follow_active_tab(app: &mut App) {
    let active = app.documents.active_tab;
    let path = app.documents.path.clone();
    let scale = app.scale_factor;
    let view_height = app.viewport.height;
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return;
    };
    if sidebar.followed_tab == active {
        return;
    }
    sidebar.followed_tab = active;
    select_listed(sidebar, path.as_deref(), scale, view_height);
}

/// Selects `path`'s row when it is listed and scrolls it into view.
pub(crate) fn select_listed(
    sidebar: &mut SidebarState,
    path: Option<&std::path::Path>,
    scale: f32,
    view_height: f32,
) {
    let Some(tree) = sidebar.tree.as_mut() else {
        return;
    };
    let rows = listed_rows(tree);
    let index = path.and_then(|path| {
        rows.iter()
            .position(|row| tree.node_path(row.id) == Some(path))
    });
    let Some(index) = index else {
        sidebar.selected_node = None;
        return;
    };
    sidebar.selected_node = Some(rows[index].id);
    let row_h = SIDEBAR_ROW_HEIGHT * scale;
    let list_view = (view_height - SIDEBAR_HEADER_HEIGHT * scale).max(row_h);
    let y = index as f32 * row_h;
    if y < sidebar.scroll_y {
        sidebar.scroll_y = y;
    } else if y + row_h > sidebar.scroll_y + list_view {
        sidebar.scroll_y = y + row_h - list_view;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, SidebarState) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
        std::fs::write(dir.path().join("README.md"), "").unwrap();
        let mut sidebar = SidebarState::open_folder(dir.path().to_path_buf()).unwrap();
        let tree = sidebar.tree.as_mut().unwrap();
        let root = tree.root();
        tree.expand(root).unwrap();
        (dir, sidebar)
    }

    fn selected_path(sidebar: &SidebarState) -> Option<std::path::PathBuf> {
        let id = sidebar.selected_node?;
        sidebar
            .tree
            .as_ref()?
            .node_path(id)
            .map(|p| p.to_path_buf())
    }

    #[test]
    fn the_selection_moves_to_the_active_file_when_its_row_is_listed() {
        let (dir, mut sidebar) = fixture();
        let readme = dir.path().join("README.md");
        select_listed(&mut sidebar, Some(&readme), 2.0, 600.0);
        assert_eq!(selected_path(&sidebar), Some(readme));
        assert!(!sidebar.focused, "following never takes focus");
    }

    #[test]
    fn a_file_under_a_collapsed_folder_clears_the_selection_without_reading_it() {
        let (dir, mut sidebar) = fixture();
        let readme = dir.path().join("README.md");
        select_listed(&mut sidebar, Some(&readme), 2.0, 600.0);
        let main = dir.path().join("src/main.rs");
        select_listed(&mut sidebar, Some(&main), 2.0, 600.0);
        assert_eq!(sidebar.selected_node, None);
        // `src` stays collapsed: nothing was expanded to find the file.
        let rows = listed_rows(sidebar.tree.as_mut().unwrap());
        assert!(rows.iter().all(|row| row.name != "main.rs"));
    }

    #[test]
    fn a_row_below_the_fold_is_scrolled_into_view() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..60 {
            std::fs::write(dir.path().join(format!("f{i:02}.txt")), "").unwrap();
        }
        let mut sidebar = SidebarState::open_folder(dir.path().to_path_buf()).unwrap();
        let tree = sidebar.tree.as_mut().unwrap();
        let root = tree.root();
        tree.expand(root).unwrap();
        let last = dir.path().join("f59.txt");
        select_listed(&mut sidebar, Some(&last), 1.0, 300.0);
        assert!(sidebar.scroll_y > 0.0);
        let row_bottom = 60.0 * SIDEBAR_ROW_HEIGHT;
        assert!(row_bottom <= sidebar.scroll_y + 300.0 - SIDEBAR_HEADER_HEIGHT + 0.5);
    }
}
