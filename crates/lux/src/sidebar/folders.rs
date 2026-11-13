//! Extra workspace folders in the sidebar, and closing one without
//! deleting it from disk.

use std::path::{Path, PathBuf};

use lux_ui::file_tree::{FileTree, NodeId, NodeKind};

use super::{SidebarState, listed_rows};

pub(crate) struct ExtraFolder {
    pub(crate) path: PathBuf,
    pub(crate) tree: FileTree,
}

/// One painted sidebar row. `root` 0 is the folder in the fixed header.
/// A `title` row names an extra folder and carries the close button.
pub(crate) struct Slot {
    pub(crate) root: usize,
    pub(crate) id: NodeId,
    pub(crate) depth: usize,
    pub(crate) name: String,
    pub(crate) kind: NodeKind,
    pub(crate) expanded: bool,
    pub(crate) title: bool,
}

pub(crate) fn slots(sidebar: &mut SidebarState) -> Vec<Slot> {
    let mut out = Vec::new();
    if let Some(tree) = sidebar.tree.as_mut() {
        for row in listed_rows(tree) {
            out.push(Slot {
                root: 0,
                id: row.id,
                depth: row.depth,
                name: row.name,
                kind: row.kind,
                expanded: row.expanded,
                title: false,
            });
        }
    }
    for (index, folder) in sidebar.extra.iter_mut().enumerate() {
        let id = folder.tree.root();
        out.push(Slot {
            root: index + 1,
            id,
            depth: 0,
            name: folder_name(&folder.path),
            kind: NodeKind::Directory,
            expanded: true,
            title: true,
        });
        for row in listed_rows(&mut folder.tree) {
            out.push(Slot {
                root: index + 1,
                id: row.id,
                depth: row.depth + 1,
                name: row.name,
                kind: row.kind,
                expanded: row.expanded,
                title: false,
            });
        }
    }
    out
}

pub(crate) fn row_count(sidebar: &mut SidebarState) -> usize {
    slots(sidebar).len()
}

impl SidebarState {
    /// Adds `path` under the open folder. The directory stays where it is.
    pub(crate) fn add_folder(&mut self, path: PathBuf) -> bool {
        if self.tree.is_none() || self.owns(&path) {
            return false;
        }
        let Some(tree) = FileTree::open(&path).ok() else {
            return false;
        };
        self.extra.push(ExtraFolder { path, tree });
        true
    }

    /// Drops the folder at `index` from the sidebar. Index 0 is the header
    /// folder. The returned path is not removed from disk.
    pub(crate) fn close_folder(&mut self, index: usize) -> Option<PathBuf> {
        if index == 0 {
            let removed = self.workspace_root.take()?;
            if self.extra.is_empty() {
                self.tree = None;
                self.selected_node = None;
                return Some(removed);
            }
            let next = self.extra.remove(0);
            self.tree = Some(next.tree);
            self.workspace_root = Some(next.path);
            self.selected_node = None;
            self.scroll_y = 0.0;
            return Some(removed);
        }
        let extra_index = index - 1;
        if extra_index >= self.extra.len() {
            return None;
        }
        let removed = self.extra.remove(extra_index);
        if self.selected_node.is_some() {
            self.selected_node = None;
        }
        Some(removed.path)
    }

    pub(crate) fn root_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(path) = &self.workspace_root {
            names.push(folder_name(path));
        }
        for folder in &self.extra {
            names.push(folder_name(&folder.path));
        }
        names
    }

    fn owns(&self, path: &Path) -> bool {
        self.workspace_root.as_deref() == Some(path)
            || self.extra.iter().any(|folder| folder.path == path)
    }
}

pub(crate) fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn activate(
    app: &mut crate::app::App,
    ctx: &dyn lux_platform::PlatformContext,
    root: usize,
    id: NodeId,
    kind: NodeKind,
) {
    let path = {
        let Some(sidebar) = app.ui.sidebar.as_mut() else {
            return;
        };
        let tree = if root == 0 {
            sidebar.tree.as_mut()
        } else {
            sidebar
                .extra
                .get_mut(root - 1)
                .map(|folder| &mut folder.tree)
        };
        let Some(tree) = tree else {
            return;
        };
        if kind == NodeKind::Directory {
            if tree.is_expanded(id) == Some(true) {
                tree.collapse(id);
            } else {
                let _ = tree.expand(id);
            }
            sidebar.selected_node = Some(id);
            None
        } else {
            tree.node_path(id).map(|path| path.to_path_buf())
        }
    };
    if let Some(path) = path {
        app.request_document(ctx, crate::document::Request::Open(Some(path)));
    }
    app.scheduler.invalidate_all();
}

pub(crate) fn close(app: &mut crate::app::App, index: usize) {
    let removed = {
        let Some(sidebar) = app.ui.sidebar.as_mut() else {
            return;
        };
        sidebar.context_menu = None;
        sidebar.drag = None;
        sidebar.close_folder(index)
    };
    let Some(path) = removed else {
        return;
    };
    if let Some(workspace) = app.ui.project.workspace.as_mut()
        && let Some(root) = workspace
            .roots()
            .iter()
            .find(|root| root.path == path)
            .map(|root| root.id)
    {
        workspace.remove_root(root);
    }
    let empty = app
        .ui
        .sidebar
        .as_ref()
        .is_some_and(|sidebar| sidebar.tree.is_none());
    if empty {
        app.ui.sidebar = None;
    }
    app.ui.announcement = Some(format!("Closed {}", folder_name(&path)));
    app.update_viewport_dimensions();
    app.scheduler.invalidate_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_one_folder_leaves_the_directory_on_disk() {
        let alpha = tempfile::tempdir().unwrap();
        let beta = tempfile::tempdir().unwrap();
        std::fs::write(alpha.path().join("keep.txt"), "keep").unwrap();
        std::fs::write(beta.path().join("gone.txt"), "gone").unwrap();
        let mut sidebar = SidebarState::open_folder(alpha.path().to_path_buf()).unwrap();
        assert!(sidebar.add_folder(beta.path().to_path_buf()));
        assert_eq!(
            sidebar.root_names(),
            vec![folder_name(alpha.path()), folder_name(beta.path()),]
        );
        let removed = sidebar.close_folder(1).unwrap();
        assert_eq!(removed, beta.path());
        assert!(beta.path().join("gone.txt").is_file());
        assert_eq!(sidebar.root_names(), vec![folder_name(alpha.path())]);
        let label = super::super::tree_label(&sidebar);
        assert!(label.contains("keep.txt"), "{label}");
        assert!(!label.contains("gone.txt"), "{label}");
    }

    #[test]
    fn closing_the_header_folder_promotes_the_next_one() {
        let alpha = tempfile::tempdir().unwrap();
        let beta = tempfile::tempdir().unwrap();
        std::fs::write(alpha.path().join("keep.txt"), "keep").unwrap();
        std::fs::write(beta.path().join("stay.txt"), "stay").unwrap();
        let mut sidebar = SidebarState::open_folder(alpha.path().to_path_buf()).unwrap();
        assert!(sidebar.add_folder(beta.path().to_path_buf()));
        let removed = sidebar.close_folder(0).unwrap();
        assert_eq!(removed, alpha.path());
        assert!(alpha.path().join("keep.txt").is_file());
        assert_eq!(sidebar.root_names(), vec![folder_name(beta.path())]);
        let label = super::super::tree_label(&sidebar);
        assert!(label.contains("stay.txt"), "{label}");
        assert!(!label.contains("keep.txt"), "{label}");
    }
}
