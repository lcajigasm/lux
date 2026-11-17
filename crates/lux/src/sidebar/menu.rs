//! Sidebar context menu items and hit-testing (T07).

use lux_ui::file_tree::NodeId;

pub(crate) const SIDEBAR_MENU_WIDTH: f32 = 190.0;
pub(crate) const SIDEBAR_MENU_ITEM_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarAction {
    NewFile,
    NewFolder,
    Rename,
    Delete,
    Duplicate,
    RevealInFinder,
    OpenInTerminal,
    CopyRelativePath,
    CopyAbsolutePath,
    /// Drop the folder from the sidebar. The directory stays on disk.
    CloseFolder,
}

impl SidebarAction {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::NewFile => "New File...",
            Self::NewFolder => "New Folder...",
            Self::Rename => "Rename... (F2)",
            Self::Delete => "Delete (Move to Trash)",
            Self::Duplicate => "Duplicate",
            Self::RevealInFinder => "Reveal in Finder",
            Self::OpenInTerminal => "Open in Terminal",
            Self::CopyRelativePath => "Copy Relative Path",
            Self::CopyAbsolutePath => "Copy Absolute Path",
            Self::CloseFolder => "Close Folder",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SidebarContextMenu {
    pub(crate) node_id: NodeId,
    #[allow(dead_code)]
    pub(crate) is_dir: bool,
    /// Which open folder this menu closes. Zero is the header folder.
    pub(crate) root_index: usize,
    pub(crate) rect: [f32; 4],
    pub(crate) items: Vec<SidebarAction>,
}

impl SidebarContextMenu {
    pub(crate) fn new(node_id: NodeId, is_dir: bool, x: f32, y: f32, scale: f32) -> Self {
        let items = if is_dir {
            vec![
                SidebarAction::NewFile,
                SidebarAction::NewFolder,
                SidebarAction::Rename,
                SidebarAction::Delete,
                SidebarAction::RevealInFinder,
                SidebarAction::OpenInTerminal,
                SidebarAction::CopyRelativePath,
                SidebarAction::CopyAbsolutePath,
            ]
        } else {
            vec![
                SidebarAction::Rename,
                SidebarAction::Delete,
                SidebarAction::Duplicate,
                SidebarAction::RevealInFinder,
                SidebarAction::CopyRelativePath,
                SidebarAction::CopyAbsolutePath,
            ]
        };

        let width = SIDEBAR_MENU_WIDTH * scale;
        let height = (items.len() as f32 * SIDEBAR_MENU_ITEM_HEIGHT + 8.0) * scale;
        Self {
            node_id,
            is_dir,
            root_index: 0,
            rect: [x, y, width, height],
            items,
        }
    }

    /// A menu whose only command removes that folder from the sidebar.
    pub(crate) fn close_folder(
        node_id: NodeId,
        root_index: usize,
        x: f32,
        y: f32,
        scale: f32,
    ) -> Self {
        let items = vec![SidebarAction::CloseFolder];
        let width = SIDEBAR_MENU_WIDTH * scale;
        let height = (items.len() as f32 * SIDEBAR_MENU_ITEM_HEIGHT + 8.0) * scale;
        Self {
            node_id,
            is_dir: true,
            root_index,
            rect: [x, y, width, height],
            items,
        }
    }

    pub(crate) fn item_at(&self, mouse_x: f32, mouse_y: f32, scale: f32) -> Option<SidebarAction> {
        let [x, y, w, h] = self.rect;
        if mouse_x < x || mouse_x > x + w || mouse_y < y || mouse_y > y + h {
            return None;
        }
        let item_h = SIDEBAR_MENU_ITEM_HEIGHT * scale;
        let offset_y = mouse_y - y - 4.0 * scale;
        if offset_y < 0.0 {
            return None;
        }
        let index = (offset_y / item_h) as usize;
        self.items.get(index).copied()
    }
}
