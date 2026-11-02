//! Workspace Folder Open & File Tree Sidebar (G12).
//!
//! Owns the sidebar panel state, directory expansion, file selection,
//! and tab opening logic for project file navigation.

use std::path::{Path, PathBuf};

use lux_platform::PlatformContext;
use lux_ui::file_tree::{FileTree, NodeId, NodeKind, Row};

pub(crate) mod menu;
pub(crate) mod ops;

pub(crate) const DEFAULT_SIDEBAR_WIDTH: f32 = 220.0;
pub(crate) const SIDEBAR_ROW_HEIGHT: f32 = 22.0;
pub(crate) const SIDEBAR_HEADER_HEIGHT: f32 = 28.0;

pub(crate) struct SidebarState {
    pub(crate) tree: Option<FileTree>,
    pub(crate) workspace_root: Option<PathBuf>,
    pub(crate) selected_node: Option<NodeId>,
    pub(crate) scroll_y: f32,
    pub(crate) visible: bool,
    pub(crate) width: f32,
    pub(crate) focused: bool,
    pub(crate) context_menu: Option<menu::SidebarContextMenu>,
    pub(crate) inline_input: Option<ops::InlineInputState>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            tree: None,
            workspace_root: None,
            selected_node: None,
            scroll_y: 0.0,
            visible: true,
            width: DEFAULT_SIDEBAR_WIDTH,
            focused: false,
            context_menu: None,
            inline_input: None,
        }
    }
}

impl SidebarState {
    pub(crate) fn open_folder(path: PathBuf) -> Option<Self> {
        let tree = FileTree::open(&path).ok()?;
        Some(Self {
            tree: Some(tree),
            workspace_root: Some(path),
            selected_node: None,
            scroll_y: 0.0,
            visible: true,
            width: DEFAULT_SIDEBAR_WIDTH,
            focused: false,
            context_menu: None,
            inline_input: None,
        })
    }

    pub(crate) fn root_name(&self) -> String {
        if let Some(root) = &self.workspace_root {
            root.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| root.display().to_string())
        } else {
            "NO FOLDER".to_string()
        }
    }

    pub(crate) fn toggle_node(&mut self, id: NodeId) {
        let Some(tree) = &mut self.tree else {
            return;
        };
        if tree.node_kind(id) == Some(NodeKind::Directory) {
            if tree.is_expanded(id) == Some(true) {
                tree.collapse(id);
            } else {
                let _ = tree.expand(id);
            }
        }
        self.selected_node = Some(id);
    }

    pub(crate) fn scroll_by(&mut self, dy: f32, scale: f32, view_height: f32) {
        let Some(tree) = self.tree.as_mut() else {
            return;
        };
        let rows = listed_rows(tree).len() as f32;
        let header = SIDEBAR_HEADER_HEIGHT * scale;
        let list_view = (view_height - header).max(0.0);
        let content = rows * SIDEBAR_ROW_HEIGHT * scale;
        let max_scroll = (content - list_view).max(0.0);
        self.scroll_y = (self.scroll_y - dy).clamp(0.0, max_scroll);
    }
}

/// Rows the sidebar paints and hit-tests. An expanded root is the header,
/// not a second copy of the folder name, and depths start at the children.
pub(crate) fn listed_rows(tree: &mut FileTree) -> Vec<Row> {
    let mut rows = tree.rows().to_vec();
    let hide_root = rows
        .first()
        .is_some_and(|row| row.depth == 0 && row.kind == NodeKind::Directory && row.expanded);
    if hide_root {
        rows.remove(0);
        for row in &mut rows {
            row.depth = row.depth.saturating_sub(1);
        }
    }
    rows
}

pub(crate) fn reveal_open_document(app: &mut crate::app::App) {
    let Some(path) = app.documents.path.clone() else {
        return;
    };
    let scale = app.scale_factor;
    let view_height = app.viewport.height;
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return;
    };
    let id = {
        let Some(tree) = sidebar.tree.as_mut() else {
            return;
        };
        match tree.reveal(&path) {
            Ok(Some(id)) => id,
            _ => return,
        }
    };
    sidebar.selected_node = Some(id);
    let rows = {
        let tree = sidebar.tree.as_mut().expect("tree still open");
        listed_rows(tree)
    };
    let Some(index) = rows.iter().position(|row| row.id == id) else {
        return;
    };
    let row_h = SIDEBAR_ROW_HEIGHT * scale;
    let header = SIDEBAR_HEADER_HEIGHT * scale;
    let y = index as f32 * row_h;
    let list_view = (view_height - header).max(row_h);
    if y < sidebar.scroll_y {
        sidebar.scroll_y = y;
    } else if y + row_h > sidebar.scroll_y + list_view {
        sidebar.scroll_y = y + row_h - list_view;
    }
}

pub(crate) fn handle_sidebar_click(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    mouse_x: f32,
    mouse_y: f32,
    button: lux_platform::MouseButton,
) -> bool {
    let sidebar_w = app.sidebar_width();
    if sidebar_w <= 0.0 || mouse_x >= sidebar_w {
        return false;
    }
    app.chrome
        .focus
        .set_focus(lux_ui::focus::FocusTarget::FileTree);
    crate::focus::sync_surface_focus(app, lux_ui::focus::FocusTarget::FileTree);

    // 1. If context menu is open, handle click on it
    if let Some(sidebar) = app.ui.sidebar.as_mut()
        && let Some(menu) = sidebar.context_menu.take()
    {
        if button == lux_platform::MouseButton::Left
            && let Some(action) = menu.item_at(mouse_x, mouse_y, app.scale_factor)
        {
            ops::handle_sidebar_action(app, ctx, action, menu.node_id);
            app.scheduler.invalidate_all();
            return true;
        }
        app.scheduler.invalidate_all();
        return true;
    }

    let tab_bar_h = app.tab_bar_height();
    let header_h = SIDEBAR_HEADER_HEIGHT * app.scale_factor;
    let row_h = SIDEBAR_ROW_HEIGHT * app.scale_factor;

    let relative_y = mouse_y - tab_bar_h;
    if relative_y < 0.0 {
        return false;
    }

    if relative_y < header_h {
        return true;
    }

    let row_idx = ((relative_y - header_h) / row_h) as usize;

    let click_action = {
        let Some(sidebar) = app.ui.sidebar.as_mut() else {
            return false;
        };
        let Some(tree) = sidebar.tree.as_mut() else {
            return false;
        };
        let rows: Vec<Row> = listed_rows(tree);
        if let Some(row) = rows.get(row_idx) {
            let id = row.id;
            let kind = row.kind;
            let path = tree.node_path(id).map(Path::to_path_buf);
            sidebar.selected_node = Some(id);
            Some((id, kind, path))
        } else {
            None
        }
    };

    let Some((id, kind, path)) = click_action else {
        return true;
    };

    match button {
        lux_platform::MouseButton::Right => {
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
                sidebar.context_menu = Some(menu::SidebarContextMenu::new(
                    id,
                    kind == NodeKind::Directory,
                    mouse_x,
                    mouse_y,
                    app.scale_factor,
                ));
            }
            app.scheduler.invalidate_all();
            true
        }
        lux_platform::MouseButton::Left => {
            match kind {
                NodeKind::Directory => {
                    if let Some(sidebar) = app.ui.sidebar.as_mut() {
                        sidebar.toggle_node(id);
                    }
                    app.scheduler.invalidate_all();
                }
                NodeKind::File => {
                    if let Some(file_path) = path {
                        app.request_document(ctx, crate::document::Request::Open(Some(file_path)));
                    }
                }
            }
            true
        }
        _ => true,
    }
}

pub(crate) fn handle_sidebar_key(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    event: &lux_platform::KeyEvent,
) -> bool {
    if event.state != lux_platform::ElementState::Pressed {
        return false;
    }

    // Dismiss context menu or inline input on Escape
    if event.key == lux_platform::Key::Named(lux_platform::NamedKey::Escape)
        && let Some(sidebar) = app.ui.sidebar.as_mut()
        && (sidebar.context_menu.take().is_some() || sidebar.inline_input.take().is_some())
    {
        app.scheduler.invalidate_all();
        return true;
    }

    // Handle inline input typing if active
    if let Some(sidebar) = app.ui.sidebar.as_mut()
        && sidebar.inline_input.is_some()
    {
        match &event.key {
            lux_platform::Key::Named(lux_platform::NamedKey::Enter) => {
                return ops::commit_inline_input(app, ctx);
            }
            lux_platform::Key::Named(lux_platform::NamedKey::Backspace) => {
                if let Some(inp) = sidebar.inline_input.as_mut() {
                    inp.text.pop();
                    app.scheduler.invalidate_all();
                }
                return true;
            }
            lux_platform::Key::Character(text)
                if !app.modifiers.meta && !app.modifiers.control && !app.modifiers.alt =>
            {
                if let Some(inp) = sidebar.inline_input.as_mut() {
                    inp.text.push_str(text);
                    app.scheduler.invalidate_all();
                }
                return true;
            }
            _ => {}
        }
        return false;
    }

    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return false;
    };
    let Some(tree) = sidebar.tree.as_mut() else {
        return false;
    };
    let rows = listed_rows(tree);
    if rows.is_empty() {
        return false;
    }
    let current_idx = sidebar
        .selected_node
        .and_then(|id| rows.iter().position(|r| r.id == id))
        .unwrap_or(0);

    match &event.key {
        lux_platform::Key::Named(lux_platform::NamedKey::F(2)) => {
            if let Some(node_id) = sidebar.selected_node {
                ops::handle_sidebar_action(app, ctx, menu::SidebarAction::Rename, node_id);
                app.scheduler.invalidate_all();
                return true;
            }
        }
        lux_platform::Key::Named(lux_platform::NamedKey::Delete) => {
            if let Some(node_id) = sidebar.selected_node {
                ops::delete_sidebar_node(app, node_id);
                return true;
            }
        }
        lux_platform::Key::Named(lux_platform::NamedKey::ArrowDown) => {
            let next = if sidebar.selected_node.is_none() {
                0
            } else if current_idx + 1 < rows.len() {
                current_idx + 1
            } else {
                return false;
            };
            sidebar.selected_node = Some(rows[next].id);
            app.scheduler.invalidate_all();
            return true;
        }
        lux_platform::Key::Named(lux_platform::NamedKey::ArrowUp) => {
            if sidebar.selected_node.is_none() {
                sidebar.selected_node = Some(rows[0].id);
                app.scheduler.invalidate_all();
                return true;
            }
            if current_idx > 0 {
                sidebar.selected_node = Some(rows[current_idx - 1].id);
                app.scheduler.invalidate_all();
                return true;
            }
        }
        lux_platform::Key::Named(lux_platform::NamedKey::Enter) => {
            let row = &rows[current_idx];
            match row.kind {
                NodeKind::Directory => {
                    sidebar.toggle_node(row.id);
                    app.scheduler.invalidate_all();
                }
                NodeKind::File => {
                    if let Some(file_path) = tree.node_path(row.id).map(Path::to_path_buf) {
                        app.request_document(ctx, crate::document::Request::Open(Some(file_path)));
                    }
                }
            }
            return true;
        }
        lux_platform::Key::Named(lux_platform::NamedKey::ArrowRight) => {
            let row = &rows[current_idx];
            if row.kind == NodeKind::Directory {
                if !row.expanded {
                    let _ = tree.expand(row.id);
                    app.scheduler.invalidate_all();
                }
                return true;
            }
        }
        lux_platform::Key::Named(lux_platform::NamedKey::ArrowLeft) => {
            let row = &rows[current_idx];
            if row.kind == NodeKind::Directory && row.expanded {
                tree.collapse(row.id);
                app.scheduler.invalidate_all();
                return true;
            }
        }
        _ => {}
    }
    false
}

#[cfg(test)]
mod behavior_tests;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_opens_existing_folder() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let sidebar = SidebarState::open_folder(manifest_dir).expect("valid folder opens");
        assert!(sidebar.tree.is_some());
        assert!(sidebar.visible);
        assert_eq!(sidebar.root_name(), "lux");
        assert!(sidebar.selected_node.is_none());
    }

    #[test]
    fn an_expanded_root_is_the_header_not_a_row() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("notes")).unwrap();
        std::fs::write(dir.path().join("notes").join("readme.txt"), "hi").unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        let mut sidebar = SidebarState::open_folder(dir.path().to_path_buf()).unwrap();
        let tree = sidebar.tree.as_mut().unwrap();
        let names: Vec<_> = listed_rows(tree)
            .iter()
            .map(|row| row.name.clone())
            .collect();
        assert_eq!(names, vec!["notes".to_string(), "a.txt".to_string()]);
        let notes = listed_rows(sidebar.tree.as_mut().unwrap())
            .iter()
            .find(|row| row.name == "notes")
            .unwrap()
            .id;
        sidebar.toggle_node(notes);
        let names: Vec<_> = listed_rows(sidebar.tree.as_mut().unwrap())
            .iter()
            .map(|row| row.name.clone())
            .collect();
        assert_eq!(
            names,
            vec![
                "notes".to_string(),
                "readme.txt".to_string(),
                "a.txt".to_string()
            ]
        );
    }

    #[test]
    fn sidebar_defaults_to_visible_and_standard_width() {
        let sidebar = SidebarState::default();
        assert_eq!(sidebar.width, DEFAULT_SIDEBAR_WIDTH);
        assert!(sidebar.visible);
    }

    #[test]
    fn sidebar_context_menu_items_for_dir_and_file() {
        use crate::sidebar::menu::{SidebarAction, SidebarContextMenu};

        let temp_dir = tempfile::tempdir().unwrap();
        let sidebar = SidebarState::open_folder(temp_dir.path().to_path_buf()).unwrap();
        let root = sidebar.tree.as_ref().unwrap().root();

        let dir_menu = SidebarContextMenu::new(root, true, 10.0, 20.0, 1.0);
        assert!(dir_menu.items.contains(&SidebarAction::NewFile));
        assert!(dir_menu.items.contains(&SidebarAction::NewFolder));

        let file_menu = SidebarContextMenu::new(root, false, 10.0, 20.0, 1.0);
        assert!(!file_menu.items.contains(&SidebarAction::NewFile));
        assert!(file_menu.items.contains(&SidebarAction::Rename));
        assert!(file_menu.items.contains(&SidebarAction::Delete));
        assert!(file_menu.items.contains(&SidebarAction::Duplicate));
    }

    #[test]
    fn sidebar_duplicate_and_delete_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("source.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        let mut app = crate::app::App::new(
            include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
        );
        app.ui.sidebar = SidebarState::open_folder(temp_dir.path().to_path_buf());
        let tree = app.ui.sidebar.as_mut().unwrap().tree.as_mut().unwrap();
        let rows = tree.rows().to_vec();
        let file_row = rows.iter().find(|r| r.name == "source.txt").unwrap();
        let file_id = file_row.id;

        assert!(ops::duplicate_sidebar_node(&mut app, file_id));
        let copy_path = temp_dir.path().join("source copy.txt");
        assert!(copy_path.exists());
        assert_eq!(std::fs::read_to_string(&copy_path).unwrap(), "hello world");

        assert!(ops::delete_sidebar_node(&mut app, file_id));
        assert!(!file_path.exists());
    }
}
