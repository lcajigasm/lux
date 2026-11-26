//! Sidebar context-menu actions (T07). Create, rename, duplicate, and
//! trash hand their disk work to `worker`; the inline name field lives in
//! `inline`.

use lux_platform::PlatformContext;
use lux_ui::file_tree::{NodeId, NodeKind};
use std::path::{Path, PathBuf};

use super::inline::{self, InlineInputMode};
use super::worker;
use crate::app::App;
use crate::sidebar::menu::SidebarAction;

pub(crate) fn handle_sidebar_action(
    app: &mut App,
    ctx: &dyn PlatformContext,
    action: SidebarAction,
    node_id: NodeId,
    root_index: usize,
) {
    match action {
        SidebarAction::NewFile | SidebarAction::NewFolder | SidebarAction::Rename => {
            let mode = match action {
                SidebarAction::NewFile => InlineInputMode::CreateFile { parent_id: node_id },
                SidebarAction::NewFolder => InlineInputMode::CreateFolder { parent_id: node_id },
                _ => InlineInputMode::Rename { node_id },
            };
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
                inline::begin(sidebar, root_index, mode);
            }
        }
        SidebarAction::Delete => {
            delete_sidebar_node(app, ctx, root_index, node_id);
        }
        SidebarAction::Duplicate => {
            duplicate_sidebar_node(app, root_index, node_id);
        }
        SidebarAction::RevealInFinder => {
            reveal_sidebar_node(app, root_index, node_id);
        }
        SidebarAction::OpenInTerminal => {
            open_sidebar_node_in_terminal(app, root_index, node_id);
        }
        SidebarAction::CopyRelativePath => {
            copy_sidebar_node_path(app, ctx, root_index, node_id, true);
        }
        SidebarAction::CopyAbsolutePath => {
            copy_sidebar_node_path(app, ctx, root_index, node_id, false);
        }
        SidebarAction::CloseFolder => {
            super::folders::close(app, root_index);
        }
    }
}

fn node_path(app: &App, root: usize, node_id: NodeId) -> Option<PathBuf> {
    app.ui
        .sidebar
        .as_ref()?
        .tree_at(root)?
        .node_path(node_id)
        .map(Path::to_path_buf)
}

/// Asks, then moves the entry to the trash, both on the sidebar worker.
/// The row leaves when the result arrives. `false` for the folder root.
pub(crate) fn delete_sidebar_node(
    app: &mut App,
    ctx: &dyn PlatformContext,
    root: usize,
    node_id: NodeId,
) -> bool {
    let is_root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.tree_at(root))
        .is_some_and(|tree| tree.root() == node_id);
    let Some(path) = node_path(app, root, node_id).filter(|_| !is_root) else {
        return false;
    };
    submit(
        app,
        worker::Job::Trash {
            path,
            dialogs: ctx.background_dialogs(),
        },
    )
}

pub(crate) fn duplicate_sidebar_node(app: &mut App, root: usize, node_id: NodeId) -> bool {
    let is_file = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.tree_at(root))
        .and_then(|tree| tree.node_kind(node_id))
        == Some(NodeKind::File);
    let Some(src) = node_path(app, root, node_id).filter(|_| is_file) else {
        return false;
    };
    submit(app, worker::Job::Duplicate { src })
}

fn submit(app: &mut App, job: worker::Job) -> bool {
    let waker = app.ui.session.waker();
    app.ui.session.file_ops.submit(job, waker);
    true
}

pub(crate) fn reveal_sidebar_node(app: &App, root: usize, node_id: NodeId) {
    let Some(path) = node_path(app, root, node_id) else {
        return;
    };
    let path = path.as_path();

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let target = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

pub(crate) fn open_sidebar_node_in_terminal(app: &mut App, root: usize, node_id: NodeId) {
    let Some(sidebar) = &app.ui.sidebar else {
        return;
    };
    let Some(tree) = sidebar.tree_at(root) else {
        return;
    };
    let Some(path) = tree.node_path(node_id) else {
        return;
    };
    let dir = if tree.node_kind(node_id) == Some(NodeKind::Directory) {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };

    app.ui.term.visible = true;
    app.ui.term.focused = true;
    let _ = app.ui.term.spawn_session(None, None, Some(&dir));
    app.update_viewport_dimensions();
    app.scheduler.invalidate_all();
}

pub(crate) fn copy_sidebar_node_path(
    app: &mut App,
    ctx: &dyn PlatformContext,
    root: usize,
    node_id: NodeId,
    relative: bool,
) {
    let Some(sidebar) = &app.ui.sidebar else {
        return;
    };
    let Some(tree) = sidebar.tree_at(root) else {
        return;
    };
    let Some(path) = tree.node_path(node_id) else {
        return;
    };

    let text = if relative {
        path.strip_prefix(tree.root_path())
            .unwrap_or(path)
            .display()
            .to_string()
    } else {
        path.display().to_string()
    };

    let _ = ctx.clipboard().write_text(&text);
    app.documents.message = Some(format!("Copied: {text}"));
}
