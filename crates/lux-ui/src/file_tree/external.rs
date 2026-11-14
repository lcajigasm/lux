//! Insert or drop a tree row when the watcher reports a path.
//!
//! Only a directory whose children have already been read is updated.
//! A folder that has never been expanded still reads disk on the next
//! expand, so an event for a file inside it does not walk that folder.

use std::fs;
use std::path::{Path, PathBuf};

use super::{FileTree, Node, NodeId, NodeKind};

impl FileTree {
    /// Adds `path` under its already-loaded parent. `false` when the row
    /// is already present, the parent has not been read, or the name is a
    /// hidden version-control directory.
    pub fn note_created(&mut self, path: &Path) -> bool {
        if self.find_id(path).is_some() {
            return false;
        }
        let Some(parent_path) = path.parent() else {
            return false;
        };
        let Some(parent) = self.find_id(parent_path) else {
            return false;
        };
        let Some(node) = self.nodes.get(&parent) else {
            return false;
        };
        if node.kind != NodeKind::Directory || node.children.is_none() {
            return false;
        }
        let name = super::display_name(path);
        let kind = if path.is_dir() {
            NodeKind::Directory
        } else {
            NodeKind::File
        };
        if kind == NodeKind::Directory && super::hidden_directory(&name) {
            return false;
        }
        let id = self.mint_id();
        self.nodes.insert(
            id,
            Node {
                id,
                parent: Some(parent),
                path: path.to_path_buf(),
                name,
                kind,
                expanded: false,
                children: None,
            },
        );
        self.nodes
            .get_mut(&parent)
            .unwrap()
            .children
            .get_or_insert_with(Vec::new)
            .push(id);
        self.resort_children(parent);
        self.rows_dirty = true;
        true
    }

    /// Drops `path` and any loaded descendants. The root stays. `false`
    /// when the path is not a loaded row.
    pub fn note_removed(&mut self, path: &Path) -> bool {
        let Some(id) = self.find_id(path) else {
            return false;
        };
        if id == self.root {
            return false;
        }
        let parent = self.nodes.get(&id).and_then(|node| node.parent);
        self.remove_subtree(id);
        if let Some(parent) = parent {
            self.remove_child(parent, id);
        }
        self.rows_dirty = true;
        true
    }

    fn find_id(&self, path: &Path) -> Option<NodeId> {
        let key = canonical_key(path);
        self.nodes.iter().find_map(|(id, node)| {
            (node.path == path || canonical_key(&node.path) == key).then_some(*id)
        })
    }
}

/// Compares a tree path with a watcher path. A removed file no longer
/// canonicalizes, so the parent directory is resolved and the file name
/// is joined back on.
fn canonical_key(path: &Path) -> PathBuf {
    if let Ok(canon) = fs::canonicalize(path) {
        return canon;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => fs::canonicalize(parent)
            .map(|parent| parent.join(name))
            .unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}
