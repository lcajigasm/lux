//! Press a sidebar row to arm a drag. Movement past a few pixels moves the
//! node onto the directory under the pointer. A press that stays put is a
//! click: expand a folder, or open a file.

use std::path::{Path, PathBuf};

use lux_platform::PlatformContext;
use lux_ui::file_tree::{NodeId, NodeKind};
use lux_ui::tabs::TabId;

use crate::app::App;
use crate::sidebar::{SIDEBAR_HEADER_HEIGHT, SIDEBAR_ROW_HEIGHT};

/// Pointer travel, in logical pixels, before a press becomes a move.
const DRAG_THRESHOLD_PX: f32 = 6.0;

pub(crate) struct TreeDrag {
    pub(crate) node: NodeId,
    origin_x: f32,
    origin_y: f32,
    pub(crate) active: bool,
    pub(crate) drop_target: Option<NodeId>,
}

pub(crate) enum Hit {
    Outside,
    Header,
    Empty,
    /// The close button on a folder header. Index 0 is the fixed header.
    Close {
        index: usize,
    },
    Row {
        id: NodeId,
        kind: NodeKind,
        root: usize,
        title: bool,
    },
}

pub(crate) fn hit(app: &mut App, mouse_x: f32, mouse_y: f32) -> Hit {
    let sidebar_w = app.sidebar_width();
    if sidebar_w <= 0.0 || mouse_x < 0.0 || mouse_x >= sidebar_w {
        return Hit::Outside;
    }
    let scale = app.scale_factor;
    let tab_bar_h = app.tab_bar_height();
    let header_h = SIDEBAR_HEADER_HEIGHT * scale;
    let row_h = SIDEBAR_ROW_HEIGHT * scale;
    let relative_y = mouse_y - tab_bar_h;
    if relative_y < 0.0 || row_h <= 0.0 {
        return Hit::Outside;
    }
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return Hit::Outside;
    };
    let close_w = 28.0 * scale;
    if relative_y < header_h {
        if mouse_x >= sidebar_w - close_w && sidebar.workspace_root.is_some() {
            return Hit::Close { index: 0 };
        }
        return Hit::Header;
    }
    let scroll_y = sidebar.scroll_y;
    let rows = super::folders::slots(sidebar);
    let content_y = relative_y - header_h + scroll_y;
    if content_y < 0.0 {
        return Hit::Header;
    }
    let Some(row) = rows.get((content_y / row_h) as usize) else {
        return Hit::Empty;
    };
    if row.title && mouse_x >= sidebar_w - close_w {
        return Hit::Close { index: row.root };
    }
    Hit::Row {
        id: row.id,
        kind: row.kind,
        root: row.root,
        title: row.title,
    }
}

pub(crate) fn arm(app: &mut App, node: NodeId, x: f32, y: f32) {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return;
    };
    sidebar.drag = Some(TreeDrag {
        node,
        origin_x: x,
        origin_y: y,
        active: false,
        drop_target: None,
    });
}

pub(crate) fn cancel(app: &mut App) {
    if let Some(sidebar) = app.ui.sidebar.as_mut() {
        sidebar.drag = None;
    }
}

/// Returns whether the drop highlight changed and the window should repaint.
pub(crate) fn update(app: &mut App, x: f32, y: f32) -> bool {
    let (origin_x, origin_y, active, node) = {
        let Some(drag) = app
            .ui
            .sidebar
            .as_ref()
            .and_then(|sidebar| sidebar.drag.as_ref())
        else {
            return false;
        };
        (drag.origin_x, drag.origin_y, drag.active, drag.node)
    };
    let scale = app.scale_factor.max(1.0);
    let threshold = DRAG_THRESHOLD_PX * scale;
    let dx = x - origin_x;
    let dy = y - origin_y;
    if !active && dx * dx + dy * dy < threshold * threshold {
        return false;
    }
    let target = drop_parent(app, x, y, node);
    let Some(drag) = app
        .ui
        .sidebar
        .as_mut()
        .and_then(|sidebar| sidebar.drag.as_mut())
    else {
        return false;
    };
    let changed = !drag.active || drag.drop_target != target;
    drag.active = true;
    drag.drop_target = target;
    if changed {
        app.scheduler.invalidate_all();
    }
    changed
}

pub(crate) fn release(app: &mut App, ctx: &dyn PlatformContext) {
    let Some(drag) = app
        .ui
        .sidebar
        .as_mut()
        .and_then(|sidebar| sidebar.drag.take())
    else {
        return;
    };
    if drag.active {
        if let Some(parent) = drag.drop_target {
            move_into(app, drag.node, parent);
        }
        app.scheduler.invalidate_all();
        return;
    }
    activate(app, ctx, drag.node);
}

fn drop_parent(app: &mut App, x: f32, y: f32, node: NodeId) -> Option<NodeId> {
    let candidate = match hit(app, x, y) {
        Hit::Header | Hit::Empty => app.ui.sidebar.as_ref()?.tree.as_ref()?.root(),
        Hit::Row { id, kind, root, .. } => {
            if root != 0 {
                return None;
            }
            match kind {
                NodeKind::Directory => id,
                NodeKind::File => app.ui.sidebar.as_ref()?.tree.as_ref()?.parent(id)?,
            }
        }
        Hit::Close { .. } | Hit::Outside => return None,
    };
    let tree = app.ui.sidebar.as_ref()?.tree.as_ref()?;
    if candidate == node || tree.node_kind(candidate) != Some(NodeKind::Directory) {
        return None;
    }
    let node_path = tree.node_path(node)?;
    let parent_path = tree.node_path(candidate)?;
    if parent_path.starts_with(node_path) {
        return None;
    }
    Some(candidate)
}

fn activate(app: &mut App, ctx: &dyn PlatformContext, id: NodeId) {
    let kind = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.tree.as_ref())
        .and_then(|tree| tree.node_kind(id));
    let path = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.tree.as_ref())
        .and_then(|tree| tree.node_path(id).map(Path::to_path_buf));
    match kind {
        Some(NodeKind::Directory) => {
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
                sidebar.toggle_node(id);
            }
            app.scheduler.invalidate_all();
        }
        Some(NodeKind::File) => {
            if let Some(path) = path {
                app.request_document(ctx, crate::document::Request::Open(Some(path)));
            }
        }
        None => {}
    }
}

fn move_into(app: &mut App, id: NodeId, new_parent: NodeId) {
    let (old_path, current_parent, name) = {
        let Some(tree) = app
            .ui
            .sidebar
            .as_ref()
            .and_then(|sidebar| sidebar.tree.as_ref())
        else {
            return;
        };
        let old_path = tree.node_path(id).map(Path::to_path_buf);
        let name = old_path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "item".to_string());
        (old_path, tree.parent(id), name)
    };
    if current_parent == Some(new_parent) {
        return;
    }
    let moved = {
        let Some(tree) = app
            .ui
            .sidebar
            .as_mut()
            .and_then(|sidebar| sidebar.tree.as_mut())
        else {
            return;
        };
        tree.move_node(id, new_parent)
    };
    match moved {
        Ok(()) => {
            let new_path = app
                .ui
                .sidebar
                .as_ref()
                .and_then(|sidebar| sidebar.tree.as_ref())
                .and_then(|tree| tree.node_path(id).map(Path::to_path_buf));
            if let (Some(old_path), Some(new_path)) = (old_path, new_path) {
                retarget_paths(app, &old_path, &new_path);
            }
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
                sidebar.selected_node = Some(id);
            }
            app.documents.message = Some(format!("Moved {name}"));
        }
        Err(err) => {
            app.documents.message = Some(format!("Move error: {err}"));
        }
    }
    app.scheduler.invalidate_all();
}

fn retarget_paths(app: &mut App, old: &Path, new: &Path) {
    let mut titles = Vec::new();
    rewrite_entries(&mut app.documents.entries, old, new, &mut titles);
    for entry in &mut app.documents.closed_entries {
        if let Some(path) = entry.path.clone()
            && let Some(updated) = rewrite_path(&path, old, new)
        {
            entry.path = Some(updated);
        }
    }
    for (id, title) in titles {
        app.documents.tabs.set_title(id, title);
    }
    if let Some(path) = app.documents.path.clone()
        && let Some(updated) = rewrite_path(&path, old, new)
    {
        app.documents.path = Some(updated);
    }
}

fn rewrite_entries(
    entries: &mut [crate::document::DocumentEntry],
    old: &Path,
    new: &Path,
    titles: &mut Vec<(TabId, String)>,
) {
    for entry in entries {
        let Some(path) = entry.path.clone() else {
            continue;
        };
        let Some(updated) = rewrite_path(&path, old, new) else {
            continue;
        };
        let title = updated
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| updated.display().to_string());
        titles.push((entry.tab_id, title));
        entry.path = Some(updated);
    }
}

fn rewrite_path(path: &Path, old: &Path, new: &Path) -> Option<PathBuf> {
    if path == old {
        return Some(new.to_path_buf());
    }
    path.strip_prefix(old).ok().map(|suffix| new.join(suffix))
}
