//! Apply a change that already happened on disk: a watcher event, or the
//! result of a worker that created, renamed, or trashed an entry.
//!
//! None of these touch the filesystem. A path is matched by stripping
//! the root (as given, or as resolved once in `open`) and walking the
//! remaining components through children already in memory. Only a
//! directory whose children have been read is updated, so an event for a
//! file inside a folder never expanded does not walk that folder.

use std::path::{Component, Path, PathBuf};

use super::{FileTree, Node, NodeId, NodeKind};

impl FileTree {
    /// The path the tree was opened with.
    pub fn root_path(&self) -> &Path {
        &self.nodes[&self.root].path
    }

    /// Adds `path` under its already-loaded parent. `false` when the row
    /// is already present, the parent has not been read, or the name is a
    /// hidden version-control directory.
    pub fn note_created(&mut self, path: &Path, kind: NodeKind) -> bool {
        self.insert_known(path, kind).is_some()
    }

    /// Like `note_created`, but returns the row's id, including when the
    /// row was already present (a watcher event can arrive first).
    pub fn note_created_id(&mut self, path: &Path, kind: NodeKind) -> Option<NodeId> {
        if let Some(id) = self.find_id(path) {
            return Some(id);
        }
        self.insert_known(path, kind)
    }

    fn insert_known(&mut self, path: &Path, kind: NodeKind) -> Option<NodeId> {
        if self.find_id(path).is_some() {
            return None;
        }
        let parent = self.find_id(path.parent()?)?;
        let node = self.nodes.get(&parent)?;
        if node.kind != NodeKind::Directory || node.children.is_none() {
            return None;
        }
        let name = super::display_name(path);
        if kind == NodeKind::Directory && super::hidden_directory(&name) {
            return None;
        }
        // The tree's own spelling of the parent, so every path in the tree
        // shares the root as the caller gave it.
        let path = node.path.join(&name);
        let id = self.mint_id();
        self.nodes.insert(
            id,
            Node {
                id,
                parent: Some(parent),
                path,
                name,
                kind,
                expanded: false,
                children: None,
            },
        );
        self.nodes
            .get_mut(&parent)?
            .children
            .get_or_insert_with(Vec::new)
            .push(id);
        self.resort_children(parent);
        self.rows_dirty = true;
        Some(id)
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

    /// `from` was renamed to `to` in the same directory. The node keeps its
    /// id, its expansion, and its loaded children. If `from` is already
    /// gone (the watcher was faster), `to` is inserted as a new row.
    pub fn note_renamed(&mut self, from: &Path, to: &Path, kind: NodeKind) -> Option<NodeId> {
        let Some(id) = self.find_id(from) else {
            return self.note_created_id(to, kind);
        };
        if id == self.root {
            return None;
        }
        if let Some(existing) = self.find_id(to)
            && existing != id
        {
            // The watcher already inserted `to`. Keep that row.
            self.note_removed(from);
            return Some(existing);
        }
        let old_path = self.nodes.get(&id)?.path.clone();
        let name = super::display_name(to);
        let new_path = old_path.with_file_name(&name);
        self.reparent_path(id, &old_path, &new_path);
        let parent = {
            let node = self.nodes.get_mut(&id)?;
            node.name = name;
            node.parent
        };
        if let Some(parent) = parent {
            self.resort_children(parent);
        }
        self.rows_dirty = true;
        Some(id)
    }

    /// The loaded node at `path`, given either the tree's spelling of the
    /// root or the resolved one. Memory only.
    pub fn find_id(&self, path: &Path) -> Option<NodeId> {
        let relative = path
            .strip_prefix(self.root_path())
            .or_else(|_| path.strip_prefix(&self.canonical_root))
            .ok()?;
        let mut current = self.root;
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return None;
            };
            let name = name.to_string_lossy();
            let children = self.nodes.get(&current)?.children.as_ref()?;
            current = children
                .iter()
                .copied()
                .find(|child| self.nodes.get(child).is_some_and(|n| n.name == name))?;
        }
        Some(current)
    }

    /// Whether `path` is under this tree's root, in either spelling.
    pub fn contains_path(&self, path: &Path) -> bool {
        path.starts_with(self.root_path()) || path.starts_with(&self.canonical_root)
    }

    /// `path` respelled under the root as the caller gave it, so a
    /// watcher path compares equal to tab paths. `None` outside the tree.
    pub fn local_path(&self, path: &Path) -> Option<PathBuf> {
        if let Ok(rest) = path.strip_prefix(self.root_path()) {
            return Some(self.root_path().join(rest));
        }
        let rest = path.strip_prefix(&self.canonical_root).ok()?;
        Some(self.root_path().join(rest))
    }
}
