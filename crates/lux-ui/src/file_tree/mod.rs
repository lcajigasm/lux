//! Virtualized file tree. The desktop sidebar paints `rows()`, expands and
//! collapses on click, and reveals the active file. Loading stays lazy:
//! `expand` reads one directory, and nothing below that is walked until
//! the user opens it. `create`, `rename`, `move_node`, and
//! `delete_to_trash` are real disk operations. A click that expands a
//! folder reads that one directory on the caller; keep the read to the
//! directory the user opened.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Opaque handle to a tree node. Minted by the tree itself and stable for
/// the node's lifetime — same reasoning as `lux_workspace::RootId` and
/// `tabs::TabId`: removing or moving other nodes never changes what an id
/// a caller is holding refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Directory,
    File,
}

#[derive(Debug, Clone)]
struct Node {
    id: NodeId,
    parent: Option<NodeId>,
    path: PathBuf,
    name: String,
    kind: NodeKind,
    expanded: bool,
    /// `None` until this directory's entries have been read from disk at
    /// least once — the lazy-load flag that keeps opening a huge project
    /// from walking its whole tree up front. Always `None` for a file.
    children: Option<Vec<NodeId>>,
}

/// One row of the flattened, currently-visible tree (expanded subtrees
/// only) — what a virtualized list view slices by scroll position instead
/// of ever materializing the whole tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: NodeId,
    pub depth: usize,
    pub name: String,
    pub kind: NodeKind,
    pub expanded: bool,
}

pub struct FileTree {
    nodes: HashMap<NodeId, Node>,
    root: NodeId,
    next_id: u64,
    rows: Vec<Row>,
    rows_dirty: bool,
}

impl FileTree {
    /// Opens `root_path` as the tree's root. If it's a directory, its
    /// immediate children are loaded and it starts expanded (so something
    /// is visible immediately, like any real explorer) — nothing below
    /// that first level is read until the caller expands further.
    pub fn open(root_path: impl Into<PathBuf>) -> io::Result<Self> {
        let root_path = root_path.into();
        let kind = if root_path.is_dir() {
            NodeKind::Directory
        } else {
            NodeKind::File
        };
        let name = display_name(&root_path);
        let root = NodeId(0);
        let mut tree = FileTree {
            nodes: HashMap::new(),
            root,
            next_id: 1,
            rows: Vec::new(),
            rows_dirty: true,
        };
        tree.nodes.insert(
            root,
            Node {
                id: root,
                parent: None,
                path: root_path,
                name,
                kind,
                expanded: false,
                children: None,
            },
        );
        tree.expand(root)?;
        Ok(tree)
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn node_path(&self, id: NodeId) -> Option<&Path> {
        self.nodes.get(&id).map(|n| n.path.as_path())
    }

    pub fn node_kind(&self, id: NodeId) -> Option<NodeKind> {
        self.nodes.get(&id).map(|n| n.kind)
    }

    pub fn is_expanded(&self, id: NodeId) -> Option<bool> {
        self.nodes.get(&id).map(|n| n.expanded)
    }

    /// Lazily reads `id`'s directory entries from disk — only on the first
    /// call; a later call just flips `expanded` back on — and marks it
    /// expanded. A no-op on a file (nothing to expand). Leaves the node
    /// collapsed, not partially expanded, if the read fails.
    pub fn expand(&mut self, id: NodeId) -> io::Result<()> {
        let node = self.nodes.get(&id).ok_or_else(not_found)?;
        if node.kind != NodeKind::Directory {
            return Ok(());
        }
        if node.children.is_none() {
            let path = node.path.clone();
            let children = self.load_children(id, &path)?;
            self.nodes.get_mut(&id).unwrap().children = Some(children);
        }
        self.nodes.get_mut(&id).unwrap().expanded = true;
        self.rows_dirty = true;
        Ok(())
    }

    /// Collapses `id`. Already-loaded children stay cached (re-expanding
    /// doesn't re-hit disk); only their visibility in `rows()` changes.
    /// `false` if `id` is unknown.
    pub fn collapse(&mut self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        node.expanded = false;
        self.rows_dirty = true;
        true
    }

    fn load_children(&mut self, parent: NodeId, dir: &Path) -> io::Result<Vec<NodeId>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            let kind = if path.is_dir() {
                NodeKind::Directory
            } else {
                NodeKind::File
            };
            let name = display_name(&path);
            if kind == NodeKind::Directory && hidden_directory(&name) {
                continue;
            }
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
            ids.push(id);
        }
        ids.sort_by_key(|&id| self.node_sort_key(id));
        Ok(ids)
    }

    fn mint_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Directories before files, then case-insensitive name — the standard
    /// explorer ordering, and what a fresh directory read already produces
    /// via `load_children`; `create`/`rename`/`move_node` re-derive it
    /// instead of forcing a re-read from disk.
    fn node_sort_key(&self, id: NodeId) -> (u8, String) {
        match self.nodes.get(&id) {
            Some(n) => (
                u8::from(matches!(n.kind, NodeKind::File)),
                n.name.to_lowercase(),
            ),
            None => (2, String::new()),
        }
    }

    fn resort_children(&mut self, parent: NodeId) {
        let Some(mut children) = self.nodes.get_mut(&parent).and_then(|n| n.children.take()) else {
            return;
        };
        children.sort_by_key(|&id| self.node_sort_key(id));
        if let Some(n) = self.nodes.get_mut(&parent) {
            n.children = Some(children);
        }
    }

    /// Drops `child` from `parent`'s already-loaded children list, if both
    /// exist — the shared cleanup step `move_node` and `delete_to_trash`
    /// each need after removing a node from its old location.
    fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        if let Some(children) = self
            .nodes
            .get_mut(&parent)
            .and_then(|n| n.children.as_mut())
        {
            children.retain(|c| *c != child);
        }
    }

    /// The flattened list of currently-visible rows, rebuilt lazily on
    /// structural change.
    pub fn rows(&mut self) -> &[Row] {
        if self.rows_dirty {
            let mut rows = Vec::new();
            self.push_rows(self.root, 0, &mut rows);
            self.rows = rows;
            self.rows_dirty = false;
        }
        &self.rows
    }

    fn push_rows(&self, id: NodeId, depth: usize, rows: &mut Vec<Row>) {
        let Some(node) = self.nodes.get(&id) else {
            return;
        };
        rows.push(Row {
            id: node.id,
            depth,
            name: node.name.clone(),
            kind: node.kind,
            expanded: node.expanded,
        });
        if node.expanded
            && let Some(children) = &node.children
        {
            for &child in children {
                self.push_rows(child, depth + 1, rows);
            }
        }
    }

    /// The subrange of `rows()` a viewport starting at `first_row` and
    /// showing `visible_count` rows would draw — the actual
    /// virtualization: a caller never needs the full row list, just this
    /// slice.
    pub fn visible_rows(&mut self, first_row: usize, visible_count: usize) -> &[Row] {
        let rows = self.rows();
        let start = first_row.min(rows.len());
        let end = (start + visible_count).min(rows.len());
        &rows[start..end]
    }

    /// Creates a new file or directory named `name` under `parent` — a
    /// real filesystem write, not a tree-only insert. `parent`'s children
    /// are loaded first (if not already) so the new entry lands in the
    /// right sorted position without a full re-read.
    pub fn create(&mut self, parent: NodeId, name: &str, kind: NodeKind) -> io::Result<NodeId> {
        self.expand(parent)?;
        let parent_node = self.nodes.get(&parent).ok_or_else(not_found)?;
        if parent_node.kind != NodeKind::Directory {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot create a child under a file",
            ));
        }
        let path = parent_node.path.join(name);
        match kind {
            NodeKind::Directory => fs::create_dir(&path)?,
            NodeKind::File => {
                // `create_new`, not `File::create`: the latter truncates a
                // pre-existing file silently, which in a file explorer
                // means data loss. This fails with `AlreadyExists` instead.
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)?;
            }
        }

        let id = self.mint_id();
        self.nodes.insert(
            id,
            Node {
                id,
                parent: Some(parent),
                path,
                name: name.to_owned(),
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
        Ok(id)
    }

    /// Renames `id` to `new_name`: a real `fs::rename`, with this node's
    /// (and, for a directory, every already-loaded descendant's) path
    /// rewritten to match so the tree doesn't go stale relative to disk.
    /// Errors on the root — its path was given by the caller when opening
    /// the tree, not this method's to change.
    pub fn rename(&mut self, id: NodeId, new_name: &str) -> io::Result<()> {
        if id == self.root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot rename the tree root",
            ));
        }
        let node = self.nodes.get(&id).ok_or_else(not_found)?;
        let old_path = node.path.clone();
        let parent = node.parent;
        let new_path = old_path.with_file_name(new_name);

        reject_if_exists(&new_path)?;
        fs::rename(&old_path, &new_path)?;

        self.reparent_path(id, &old_path, &new_path);
        self.nodes.get_mut(&id).unwrap().name = new_name.to_owned();
        if let Some(parent) = parent {
            self.resort_children(parent);
        }
        self.rows_dirty = true;
        Ok(())
    }

    /// Moves `id` to become a child of `new_parent` on disk (`fs::rename`
    /// across directories) — the operation a drag & drop gesture reports
    /// its drop target through. Unlike `tabs::TabBar::move_tab`, this
    /// can't be an in-memory-only reorder: the files are real.
    pub fn move_node(&mut self, id: NodeId, new_parent: NodeId) -> io::Result<()> {
        if id == self.root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot move the tree root",
            ));
        }
        let node = self.nodes.get(&id).ok_or_else(not_found)?;
        let old_path = node.path.clone();
        let old_parent = node.parent;
        let name = node.name.clone();

        let new_parent_node = self.nodes.get(&new_parent).ok_or_else(not_found)?;
        if new_parent_node.kind != NodeKind::Directory {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "drop target must be a directory",
            ));
        }
        if is_ancestor_or_self(&old_path, &new_parent_node.path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot move a directory into itself or a descendant",
            ));
        }

        // Load the destination's children *before* the real move, so the
        // just-moved entry isn't double-scanned when we insert it below.
        self.expand(new_parent)?;
        let new_path = self.nodes.get(&new_parent).unwrap().path.join(&name);
        reject_if_exists(&new_path)?;
        fs::rename(&old_path, &new_path)?;

        if let Some(old_parent) = old_parent {
            self.remove_child(old_parent, id);
        }
        self.reparent_path(id, &old_path, &new_path);
        self.nodes.get_mut(&id).unwrap().parent = Some(new_parent);
        self.nodes
            .get_mut(&new_parent)
            .unwrap()
            .children
            .get_or_insert_with(Vec::new)
            .push(id);
        self.resort_children(new_parent);
        self.rows_dirty = true;
        Ok(())
    }

    /// Deletes `id` by moving it to the system trash — never a permanent
    /// delete — and drops it from the in-memory tree. Errors on the root.
    pub fn delete_to_trash(&mut self, id: NodeId) -> io::Result<()> {
        if id == self.root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot delete the tree root",
            ));
        }
        let node = self.nodes.get(&id).ok_or_else(not_found)?;
        let path = node.path.clone();
        let parent = node.parent;

        trash::delete(&path).map_err(io::Error::other)?;

        self.remove_subtree(id);
        if let Some(parent) = parent {
            self.remove_child(parent, id);
        }
        self.rows_dirty = true;
        Ok(())
    }

    fn remove_subtree(&mut self, id: NodeId) {
        let children = self
            .nodes
            .get(&id)
            .and_then(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        self.nodes.remove(&id);
    }

    /// Rewrites `id`'s path (and every already-loaded descendant's) by
    /// replacing the `old_path` prefix with `new_path` — cheaper than
    /// re-reading the subtree from disk, and correct because a
    /// rename/move only changes the moved prefix, never anything below.
    fn reparent_path(&mut self, id: NodeId, old_path: &Path, new_path: &Path) {
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let Some(node) = self.nodes.get_mut(&current) else {
                continue;
            };
            if let Ok(suffix) = node.path.strip_prefix(old_path) {
                node.path = new_path.join(suffix);
            }
            if let Some(children) = &node.children {
                stack.extend(children.iter().copied());
            }
        }
    }

    /// Expands every ancestor directory of `path` (loading them from disk
    /// as needed, but not `path`'s own children) so its node becomes part
    /// of `rows()` — the "revealed" state a real explorer scrolls to.
    /// `None` if `path` isn't under this tree's root or doesn't exist.
    pub fn reveal(&mut self, path: impl AsRef<Path>) -> io::Result<Option<NodeId>> {
        let path = path.as_ref();
        let root_path = self.nodes.get(&self.root).unwrap().path.clone();
        let Ok(relative) = path.strip_prefix(&root_path) else {
            return Ok(None);
        };

        let mut current = self.root;
        self.expand(current)?;
        let components: Vec<_> = relative.components().collect();
        for (i, component) in components.iter().enumerate() {
            let name = component.as_os_str().to_string_lossy();
            let Some(children) = self.nodes.get(&current).and_then(|n| n.children.clone()) else {
                return Ok(None);
            };
            let next = children
                .iter()
                .copied()
                .find(|c| self.nodes.get(c).map(|n| n.name.as_str()) == Some(name.as_ref()));
            let Some(next) = next else {
                return Ok(None);
            };
            current = next;
            if i + 1 < components.len() {
                self.expand(current)?;
            }
        }
        Ok(Some(current))
    }
}

fn is_ancestor_or_self(ancestor: &Path, candidate: &Path) -> bool {
    candidate == ancestor || candidate.starts_with(ancestor)
}

fn not_found() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "unknown tree node")
}

/// Guards `rename`/`move_node` against `fs::rename`'s own POSIX semantics:
/// unlike `File::create`'s `create_new` option, `fs::rename` silently
/// replaces an existing destination rather than erroring — real data loss
/// for a rename/move landing on top of something already there. Checked
/// explicitly rather than relying on `fs::rename` itself, which has no
/// "fail if destination exists" mode. `try_exists` (not `exists`) so a
/// stat error (e.g. permission denied) surfaces as a real `io::Error`
/// instead of silently reading as "doesn't exist".
fn reject_if_exists(path: &Path) -> io::Result<()> {
    if path.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        ));
    }
    Ok(())
}

/// Version-control metadata directories. Sublime's default
/// `folder_exclude_patterns` hides the same set.
fn hidden_directory(name: &str) -> bool {
    matches!(name, ".git" | ".svn" | ".hg" | "CVS" | ".bzr")
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
