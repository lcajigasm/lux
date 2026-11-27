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
    /// Adds `path` under the open folder, unread. The directory stays where
    /// it is. Callers go through `reads::add_root`, which reads it.
    pub(crate) fn add_folder(&mut self, path: PathBuf) -> bool {
        if self.tree.is_none() || self.owns(&path) {
            return false;
        }
        // Read on the file-ops worker; see `reads::add_root`.
        let tree = FileTree::unread(&path);
        self.extra.push(ExtraFolder { path, tree });
        true
    }

    /// Drops the folder at `index` from the sidebar. Index 0 is the header
    /// folder. The returned path is not removed from disk.
    pub(crate) fn close_folder(&mut self, index: usize) -> Option<PathBuf> {
        let open = 1 + self.extra.len();
        if index < open && self.workspace_root.is_some() {
            self.shift_inline_input(index);
        }
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

    /// The inline field follows its folder when an earlier one closes and
    /// is cancelled when its own folder closes, so Enter never resolves
    /// its node against another folder's tree.
    fn shift_inline_input(&mut self, closing: usize) {
        let Some(input) = self.inline_input.as_mut() else {
            return;
        };
        if input.root == closing {
            self.inline_input = None;
        } else if input.root > closing {
            input.root -= 1;
        }
    }

    /// The tree of open folder `root`: 0 is the header, then `extra`.
    pub(crate) fn tree_at(&self, root: usize) -> Option<&FileTree> {
        match root {
            0 => self.tree.as_ref(),
            n => self.extra.get(n - 1).map(|folder| &folder.tree),
        }
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
            let mut unread = None;
            if tree.is_expanded(id) == Some(true) {
                tree.collapse(id);
            } else if !tree.expand_loaded(id) {
                unread = tree.unread_dir(id);
            }
            sidebar.selected_node = Some(id);
            if let Some(dir) = unread {
                super::reads::expand_later(app, dir);
            }
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
    if index == 0 && app.ui.sidebar.is_some() {
        // The header folder keys the session; its writes go elsewhere next.
        crate::session::leave_root(app);
    }
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
        app.ui.session.watcher = None;
        app.ui.session.symbols = None;
        if app.chrome.focus.current() == lux_ui::focus::FocusTarget::FileTree {
            app.chrome
                .focus
                .set_focus(lux_ui::focus::FocusTarget::Editor);
        }
    } else {
        // The other folders stay watched, and so do unsaved-file marks.
        app.ui.session.watch_folder(&path, false);
        if index == 0
            && let Some(root) = app
                .ui
                .sidebar
                .as_ref()
                .and_then(|sidebar| sidebar.workspace_root.clone())
        {
            // The project symbol index follows the new first folder.
            app.ui.session.symbols = Some(lux_workspace::SymbolIndexer::start(
                Some(root),
                app.ui.session.waker(),
            ));
        }
    }
    detach_closed_repo(app, &path);
    // A clean tab whose file lives in the closed folder goes with it.
    // A tab with unsaved edits stays open. There is no dialog: the
    // announcement says so, and the file is still on disk.
    let ids: Vec<_> = app
        .documents
        .entries
        .iter()
        .filter_map(|entry| {
            let tab_path = entry.path.as_ref()?;
            if tab_path.starts_with(&path) && !entry.is_dirty() {
                Some(entry.tab_id)
            } else {
                None
            }
        })
        .collect();
    let dirty = app.documents.entries.iter().any(|entry| {
        entry
            .path
            .as_ref()
            .is_some_and(|tab_path| tab_path.starts_with(&path) && entry.is_dirty())
    });
    for id in ids {
        app.close_clean_tab_now(id);
    }
    app.ui.announcement = Some(if dirty {
        format!(
            "Closed {}. Unsaved files from that folder stayed open.",
            folder_name(&path)
        )
    } else {
        format!("Closed {}", folder_name(&path))
    });
    app.update_viewport_dimensions();
    app.scheduler.invalidate_all();
}

/// Drops git state when the closed folder is that repository, and always
/// when the last folder is gone. A status shot already in flight carries
/// the old `folder_epoch` and is ignored.
fn detach_closed_repo(app: &mut crate::app::App, closed: &std::path::Path) {
    let repo = app.ui.git.repo_root.clone();
    let drop_repo = match &repo {
        Some(repo) => repo == closed || repo.starts_with(closed),
        None => app.ui.sidebar.is_none(),
    };
    if !drop_repo {
        return;
    }
    app.ui.git.repo_root = None;
    app.ui.git.branch_name = None;
    app.ui.git.hunks.clear();
    app.ui.git.markers.clear();
    app.ui.git.changed_files.clear();
    app.ui.git.untracked_dirs.clear();
    app.ui.git.ignored.clear();
    app.ui.git.selected_file_index = 0;
    app.ui.git.active_popup = None;
    app.ui.git.conflict_blocks.clear();
    app.ui.git.branch_picker = None;
    app.ui.git.commit_message.clear();
    crate::git::new_gutter_context(app);
    app.ui.git.folder_epoch = app.ui.git.folder_epoch.wrapping_add(1);
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
        assert!(sidebar.add_folder_now(beta.path().to_path_buf()));
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
        assert!(sidebar.add_folder_now(beta.path().to_path_buf()));
        let removed = sidebar.close_folder(0).unwrap();
        assert_eq!(removed, alpha.path());
        assert!(alpha.path().join("keep.txt").is_file());
        assert_eq!(sidebar.root_names(), vec![folder_name(beta.path())]);
        let label = super::super::tree_label(&sidebar);
        assert!(label.contains("stay.txt"), "{label}");
        assert!(!label.contains("keep.txt"), "{label}");
    }

    #[test]
    fn closing_a_folder_drops_its_clean_tab_and_keeps_a_dirty_one() {
        let alpha = tempfile::tempdir().unwrap();
        let beta = tempfile::tempdir().unwrap();
        std::fs::write(alpha.path().join("keep.txt"), "keep").unwrap();
        std::fs::write(beta.path().join("gone.txt"), "gone").unwrap();
        std::fs::write(beta.path().join("dirty.txt"), "dirty").unwrap();
        let mut app = crate::app::App::new(
            include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
        );
        app.ui.sidebar = Some(SidebarState::open_folder(alpha.path().to_path_buf()).unwrap());
        assert!(
            app.ui
                .sidebar
                .as_mut()
                .unwrap()
                .add_folder_now(beta.path().to_path_buf())
        );
        let gone_id = app.documents.tabs.open("gone.txt");
        app.documents
            .entries
            .push(crate::document::DocumentEntry::new(
                gone_id,
                Some(beta.path().join("gone.txt")),
                lux_core::buffer::Buffer::from_bytes(b"gone").unwrap(),
            ));
        let dirty_id = app.documents.tabs.open("dirty.txt");
        let mut dirty = crate::document::DocumentEntry::new(
            dirty_id,
            Some(beta.path().join("dirty.txt")),
            lux_core::buffer::Buffer::from_bytes(b"dirty").unwrap(),
        );
        dirty.saved_version = 1;
        app.documents.entries.push(dirty);
        app.switch_to_tab(gone_id);

        close(&mut app, 1);

        let names: Vec<String> = app
            .documents
            .entries
            .iter()
            .filter_map(|entry| {
                entry
                    .path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect();
        assert!(!names.iter().any(|name| name == "gone.txt"), "{names:?}");
        assert!(names.iter().any(|name| name == "dirty.txt"), "{names:?}");
        assert!(beta.path().join("gone.txt").is_file());
        assert!(beta.path().join("dirty.txt").is_file());
        let announcement = app.ui.announcement.unwrap_or_default();
        assert!(announcement.contains("Unsaved"), "{announcement}");
    }

    /// Luis: the only project header is closed while a nested file
    /// (`workflows/ci.yml`) and an Untitled tab are open, on branch main.
    /// The click focuses the file tree, then the tree goes away. AccessKit
    /// panics if that focus id is no longer a node.
    #[test]
    fn closing_the_only_project_drops_its_tree_without_a_dangling_focus() {
        let root = tempfile::tempdir().unwrap();
        let workflows = root.path().join(".github/workflows");
        std::fs::create_dir_all(&workflows).unwrap();
        std::fs::create_dir_all(root.path().join(".claude")).unwrap();
        std::fs::create_dir_all(root.path().join("scripts")).unwrap();
        std::fs::create_dir_all(root.path().join("tests")).unwrap();
        let ci = workflows.join("ci.yml");
        std::fs::write(&ci, "name: ci\n").unwrap();
        std::fs::write(root.path().join(".claude/notes.md"), "notes\n").unwrap();
        std::fs::write(root.path().join("scripts/run.sh"), "echo ok\n").unwrap();
        std::fs::write(root.path().join("tests/app.rs"), "fn main() {}\n").unwrap();

        let mut app = crate::app::App::new(
            include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
        );
        app.ui.sidebar = Some(SidebarState::open_folder(root.path().to_path_buf()).unwrap());
        app.ui.session.init_watcher(root.path());
        app.ui.git.repo_root = Some(root.path().to_path_buf());
        app.ui.git.branch_name = Some("main".into());
        {
            let sidebar = app.ui.sidebar.as_mut().unwrap();
            let id = sidebar.tree.as_mut().unwrap().reveal(&ci).unwrap().unwrap();
            sidebar.selected_node = Some(id);
        }
        let untitled = app.documents.tabs.open("Untitled");
        app.documents
            .entries
            .push(crate::document::DocumentEntry::new(
                untitled,
                None,
                lux_core::buffer::Buffer::from_bytes(b"").unwrap(),
            ));
        let ci_id = app.documents.tabs.open("ci.yml");
        app.documents
            .entries
            .push(crate::document::DocumentEntry::new(
                ci_id,
                Some(ci.clone()),
                lux_core::buffer::Buffer::from_bytes(b"name: ci\n").unwrap(),
            ));
        app.switch_to_tab(ci_id);
        app.chrome
            .focus
            .set_focus(lux_ui::focus::FocusTarget::FileTree);

        close(&mut app, 0);

        let update = app.build_accessibility_tree();
        let _tree = accesskit_consumer::Tree::new(update, true);
        assert!(app.ui.sidebar.is_none());
        assert!(app.ui.session.watcher.is_none());
        assert!(app.ui.git.repo_root.is_none());
        assert!(app.ui.git.branch_name.is_none());
        assert_eq!(app.ui.git.folder_epoch, 1);
        assert_eq!(
            app.chrome.focus.current(),
            lux_ui::focus::FocusTarget::Editor
        );
        assert!(ci.is_file());
        assert!(app.documents.path.is_none());
        let titles: Vec<_> = app
            .documents
            .tabs
            .tabs()
            .iter()
            .map(|tab| tab.title.as_str())
            .collect();
        assert!(titles.contains(&"Untitled"), "{titles:?}");
        assert!(!titles.contains(&"ci.yml"), "{titles:?}");
    }

    /// A field open in the third folder moves to index 1 when the header
    /// folder closes, and is cancelled when its own folder closes.
    #[test]
    fn the_inline_field_follows_its_folder_and_closes_with_it() {
        use crate::sidebar::inline::{self, InlineInputMode};
        let alpha = tempfile::tempdir().unwrap();
        let beta = tempfile::tempdir().unwrap();
        let gamma = tempfile::tempdir().unwrap();
        std::fs::write(gamma.path().join("g.txt"), "g").unwrap();
        let mut sidebar = SidebarState::open_folder(alpha.path().to_path_buf()).unwrap();
        assert!(sidebar.add_folder_now(beta.path().to_path_buf()));
        assert!(sidebar.add_folder_now(gamma.path().to_path_buf()));
        let root = sidebar.tree_at(2).unwrap().root();
        inline::begin(
            &mut sidebar,
            2,
            InlineInputMode::CreateFile { parent_id: root },
        );

        sidebar.close_folder(0);
        let input = sidebar.inline_input.as_ref().expect("field still open");
        assert_eq!(input.root, 1);
        let tree = sidebar.tree_at(input.root).unwrap();
        let anchor = tree.node_path(input.anchor()).unwrap();
        assert!(anchor.ends_with(gamma.path().file_name().unwrap()));

        sidebar.close_folder(0);
        assert_eq!(sidebar.inline_input.as_ref().unwrap().root, 0);
        sidebar.close_folder(0);
        assert!(sidebar.inline_input.is_none());
    }
}
