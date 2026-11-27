//! Directory reads, split from installing them into the tree.
//!
//! [`Listing::read`] and [`RootRead::read`] touch the disk and are meant
//! for a worker thread. Everything on [`FileTree`] here is memory only: a
//! caller on the UI thread asks what still needs reading
//! ([`FileTree::unread_dir`], [`FileTree::reveal_loaded`]), hands those
//! paths to a worker, and installs what comes back.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use super::{FileTree, Node, NodeId, NodeKind};

/// One directory's entries as read from disk.
#[derive(Debug, Clone, Default)]
pub struct Listing {
    entries: Vec<(PathBuf, NodeKind)>,
}

impl Listing {
    /// Reads `dir`: one `read_dir` and one `stat` per entry.
    pub fn read(dir: &Path) -> io::Result<Self> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            let kind = if path.is_dir() {
                NodeKind::Directory
            } else {
                NodeKind::File
            };
            entries.push((path, kind));
        }
        Ok(Self { entries })
    }
}

/// Everything opening a tree reads from disk: the root's kind, its
/// resolved spelling, and its first level of entries.
#[derive(Debug)]
pub struct RootRead {
    path: PathBuf,
    kind: NodeKind,
    canonical: PathBuf,
    listing: io::Result<Listing>,
}

impl RootRead {
    pub fn read(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let kind = if path.is_dir() {
            NodeKind::Directory
        } else {
            NodeKind::File
        };
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        let listing = match kind {
            NodeKind::Directory => Listing::read(&path),
            NodeKind::File => Ok(Listing::default()),
        };
        Self {
            path,
            kind,
            canonical,
            listing,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// What revealing a path needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reveal {
    /// The path's row, with every ancestor now expanded.
    Found(NodeId),
    /// The path is outside the tree, or a loaded directory has no such entry.
    Missing,
    /// These directories, outermost first, must be read and installed.
    Read(Vec<PathBuf>),
}

impl FileTree {
    /// A tree for `root` with nothing read yet. Memory only. The root is
    /// an open folder with no rows until [`FileTree::install_root`].
    pub fn unread(root: impl Into<PathBuf>) -> Self {
        let path = root.into();
        let root = NodeId(0);
        let mut tree = FileTree {
            nodes: Default::default(),
            root,
            next_id: 1,
            rows: Vec::new(),
            rows_dirty: true,
            canonical_root: path.clone(),
        };
        tree.nodes.insert(
            root,
            Node {
                id: root,
                parent: None,
                name: super::display_name(&path),
                path,
                kind: NodeKind::Directory,
                expanded: true,
                children: None,
            },
        );
        tree
    }

    /// A loaded tree from a finished [`RootRead`]. Memory only.
    pub fn from_root(read: RootRead) -> io::Result<Self> {
        let mut tree = Self::unread(read.path.clone());
        tree.install_root(read)?;
        Ok(tree)
    }

    /// Installs the root's read into a tree made by [`FileTree::unread`].
    /// Errors when the root's directory could not be read.
    pub fn install_root(&mut self, read: RootRead) -> io::Result<()> {
        let listing = read.listing?;
        self.canonical_root = read.canonical;
        let root = self.root;
        if let Some(node) = self.nodes.get_mut(&root) {
            node.kind = read.kind;
            node.expanded = read.kind == NodeKind::Directory;
        }
        if read.kind == NodeKind::Directory {
            let children = self.children_from(root, listing);
            if let Some(node) = self.nodes.get_mut(&root) {
                node.children = Some(children);
            }
        }
        self.rows_dirty = true;
        Ok(())
    }

    /// The directory at `id`, when its entries were never read.
    pub fn unread_dir(&self, id: NodeId) -> Option<PathBuf> {
        let node = self.nodes.get(&id)?;
        (node.kind == NodeKind::Directory && node.children.is_none()).then(|| node.path.clone())
    }

    /// Installs `listing` as the entries of the loaded directory at `dir`,
    /// unless they were already read. `true` when installed.
    pub fn install_listing(&mut self, dir: &Path, listing: Listing) -> bool {
        let Some(id) = self.find_id(dir) else {
            return false;
        };
        if self.unread_dir(id).is_none() {
            return false;
        }
        let children = self.children_from(id, listing);
        if let Some(node) = self.nodes.get_mut(&id) {
            node.children = Some(children);
        }
        self.rows_dirty = true;
        true
    }

    /// Expands `id` when its entries are in memory. `false` when they
    /// still need a read (see [`FileTree::unread_dir`]) or `id` is unknown.
    pub fn expand_loaded(&mut self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        if node.kind == NodeKind::Directory && node.children.is_none() {
            return false;
        }
        if node.kind == NodeKind::Directory {
            node.expanded = true;
            self.rows_dirty = true;
        }
        true
    }

    /// [`FileTree::reveal`] without the disk: expands the ancestors of
    /// `path` when they are all loaded, otherwise lists what to read.
    pub fn reveal_loaded(&mut self, path: &Path) -> Reveal {
        let relative = path
            .strip_prefix(self.root_path())
            .or_else(|_| path.strip_prefix(&self.canonical_root));
        let Ok(relative) = relative else {
            return Reveal::Missing;
        };
        let names: Vec<_> = relative
            .components()
            .filter_map(|c| match c {
                Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        let mut current = self.root;
        let mut opened = Vec::new();
        for (index, name) in names.iter().enumerate() {
            let Some(node) = self.nodes.get(&current) else {
                return Reveal::Missing;
            };
            let Some(children) = &node.children else {
                let mut dir = node.path.clone();
                let mut need = vec![dir.clone()];
                for deeper in &names[index..names.len() - 1] {
                    dir = dir.join(deeper);
                    need.push(dir.clone());
                }
                return Reveal::Read(need);
            };
            let next = children
                .iter()
                .copied()
                .find(|c| self.nodes.get(c).is_some_and(|n| &n.name == name));
            let Some(next) = next else {
                return Reveal::Missing;
            };
            opened.push(current);
            current = next;
        }
        if names.is_empty() {
            if let Some(dir) = self.unread_dir(current) {
                return Reveal::Read(vec![dir]);
            }
            opened.push(current);
        }
        for id in opened {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.expanded = true;
            }
        }
        self.rows_dirty = true;
        Reveal::Found(current)
    }

    /// Mints the nodes for `listing` under `parent`, sorted, skipping
    /// version-control directories.
    pub(super) fn children_from(&mut self, parent: NodeId, listing: Listing) -> Vec<NodeId> {
        let mut ids = Vec::new();
        for (path, kind) in listing.entries {
            let name = super::display_name(&path);
            if kind == NodeKind::Directory && super::hidden_directory(&name) {
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
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test, removed when dropped.
    struct Temp(PathBuf);

    impl Temp {
        fn new(label: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("lux-ui-listing-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn names(tree: &mut FileTree) -> Vec<String> {
        tree.rows().iter().map(|row| row.name.clone()).collect()
    }

    #[test]
    fn an_unread_tree_shows_nothing_until_its_root_is_installed() {
        let dir = Temp::new("unread");
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        let mut tree = FileTree::unread(dir.path());
        assert_eq!(names(&mut tree).len(), 1, "only the root row");
        assert!(tree.unread_dir(tree.root()).is_some());
        tree.install_root(RootRead::read(dir.path())).unwrap();
        let mut opened = FileTree::open(dir.path()).unwrap();
        assert_eq!(names(&mut tree), names(&mut opened));
    }

    #[test]
    fn reveal_loaded_asks_for_every_unread_ancestor_then_finds_the_row() {
        let dir = Temp::new("reveal");
        let deep = dir.path().join("a").join("b");
        fs::create_dir_all(&deep).unwrap();
        let file = deep.join("f.txt");
        fs::write(&file, "f").unwrap();
        let mut tree = FileTree::open(dir.path()).unwrap();
        let Reveal::Read(dirs) = tree.reveal_loaded(&file) else {
            panic!("a and b are unread");
        };
        assert_eq!(dirs, vec![dir.path().join("a"), deep.clone()]);
        for dir in dirs {
            let listing = Listing::read(&dir).unwrap();
            assert!(tree.install_listing(&dir, listing));
        }
        let Reveal::Found(id) = tree.reveal_loaded(&file) else {
            panic!("everything is loaded");
        };
        assert_eq!(tree.node_path(id), Some(file.as_path()));
        assert_eq!(
            names(&mut tree),
            vec![
                names(&mut FileTree::unread(dir.path()))[0].clone(),
                "a".into(),
                "b".into(),
                "f.txt".into()
            ]
        );
        assert_eq!(
            tree.reveal_loaded(&dir.path().join("nope.txt")),
            Reveal::Missing
        );
    }

    #[test]
    fn a_move_into_a_loaded_folder_keeps_the_row() {
        let dir = Temp::new("move");
        fs::create_dir(dir.path().join("dest")).unwrap();
        let from = dir.path().join("m.txt");
        fs::write(&from, "m").unwrap();
        let mut tree = FileTree::open(dir.path()).unwrap();
        let dest = tree.find_id(&dir.path().join("dest")).unwrap();
        let listing = Listing::read(&dir.path().join("dest")).unwrap();
        tree.install_listing(&dir.path().join("dest"), listing);
        assert!(tree.expand_loaded(dest));
        let id = tree.find_id(&from).unwrap();
        let to = super::super::disk::move_entry(&from, &dir.path().join("dest")).unwrap();
        assert_eq!(tree.note_moved(&from, &to, NodeKind::File), Some(id));
        assert_eq!(tree.node_path(id), Some(to.as_path()));
        assert_eq!(tree.parent(id), Some(dest));
    }
}
