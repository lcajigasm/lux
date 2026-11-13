//! Sidebar filesystem operations: create, rename, delete, duplicate, reveal, and open in terminal (T07).

use lux_platform::PlatformContext;
use lux_ui::file_tree::{NodeId, NodeKind};
use std::fs;
use std::path::Path;

use crate::app::App;
use crate::sidebar::menu::SidebarAction;

#[derive(Debug, Clone)]
pub(crate) enum InlineInputMode {
    CreateFile { parent_id: NodeId },
    CreateFolder { parent_id: NodeId },
    Rename { node_id: NodeId },
}

#[derive(Debug, Clone)]
pub(crate) struct InlineInputState {
    pub(crate) mode: InlineInputMode,
    pub(crate) text: String,
    pub(crate) row_idx: usize,
}

pub(crate) fn handle_sidebar_action(
    app: &mut App,
    ctx: &dyn PlatformContext,
    action: SidebarAction,
    node_id: NodeId,
    root_index: usize,
) {
    match action {
        SidebarAction::NewFile => {
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
                let row_idx = sidebar
                    .tree
                    .as_mut()
                    .and_then(|t| {
                        crate::sidebar::listed_rows(t)
                            .iter()
                            .position(|r| r.id == node_id)
                    })
                    .unwrap_or(0);
                sidebar.inline_input = Some(InlineInputState {
                    mode: InlineInputMode::CreateFile { parent_id: node_id },
                    text: String::new(),
                    row_idx,
                });
            }
        }
        SidebarAction::NewFolder => {
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
                let row_idx = sidebar
                    .tree
                    .as_mut()
                    .and_then(|t| {
                        crate::sidebar::listed_rows(t)
                            .iter()
                            .position(|r| r.id == node_id)
                    })
                    .unwrap_or(0);
                sidebar.inline_input = Some(InlineInputState {
                    mode: InlineInputMode::CreateFolder { parent_id: node_id },
                    text: String::new(),
                    row_idx,
                });
            }
        }
        SidebarAction::Rename => {
            if let Some(sidebar) = app.ui.sidebar.as_mut() {
                let (name, row_idx) = if let Some(tree) = &mut sidebar.tree {
                    let name = tree
                        .node_path(node_id)
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let row = crate::sidebar::listed_rows(tree)
                        .iter()
                        .position(|r| r.id == node_id)
                        .unwrap_or(0);
                    (name, row)
                } else {
                    (String::new(), 0)
                };
                sidebar.inline_input = Some(InlineInputState {
                    mode: InlineInputMode::Rename { node_id },
                    text: name,
                    row_idx,
                });
            }
        }
        SidebarAction::Delete => {
            delete_sidebar_node(app, node_id);
        }
        SidebarAction::Duplicate => {
            duplicate_sidebar_node(app, node_id);
        }
        SidebarAction::RevealInFinder => {
            reveal_sidebar_node(app, node_id);
        }
        SidebarAction::OpenInTerminal => {
            open_sidebar_node_in_terminal(app, node_id);
        }
        SidebarAction::CopyRelativePath => {
            copy_sidebar_node_path(app, ctx, node_id, true);
        }
        SidebarAction::CopyAbsolutePath => {
            copy_sidebar_node_path(app, ctx, node_id, false);
        }
        SidebarAction::CloseFolder => {
            super::folders::close(app, root_index);
        }
    }
}

pub(crate) fn commit_inline_input(app: &mut App, ctx: &dyn PlatformContext) -> bool {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return false;
    };
    let Some(input) = sidebar.inline_input.take() else {
        return false;
    };
    let name = input.text.trim();
    if name.is_empty() {
        return true;
    }
    let Some(tree) = sidebar.tree.as_mut() else {
        return false;
    };

    match input.mode {
        InlineInputMode::CreateFile { parent_id } => {
            match tree.create(parent_id, name, NodeKind::File) {
                Ok(new_id) => {
                    if let Some(path) = tree.node_path(new_id).map(Path::to_path_buf) {
                        app.request_document(ctx, crate::document::Request::Open(Some(path)));
                    }
                }
                Err(e) => {
                    app.documents.message = Some(format!("Create file error: {e}"));
                }
            }
        }
        InlineInputMode::CreateFolder { parent_id } => {
            if let Err(e) = tree.create(parent_id, name, NodeKind::Directory) {
                app.documents.message = Some(format!("Create folder error: {e}"));
            }
        }
        InlineInputMode::Rename { node_id } => {
            if let Err(e) = tree.rename(node_id, name) {
                app.documents.message = Some(format!("Rename error: {e}"));
            }
        }
    }
    app.scheduler.invalidate_all();
    true
}

pub(crate) fn delete_sidebar_node(app: &mut App, node_id: NodeId) -> bool {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return false;
    };
    let Some(tree) = sidebar.tree.as_mut() else {
        return false;
    };
    let deleted_path = tree.node_path(node_id).map(Path::to_path_buf);
    match tree.delete_to_trash(node_id) {
        Ok(()) => {
            if let Some(path) = deleted_path {
                app.documents.message = Some(format!("Moved to trash: {}", path.display()));
            }
            app.scheduler.invalidate_all();
            true
        }
        Err(e) => {
            app.documents.message = Some(format!("Delete error: {e}"));
            false
        }
    }
}

pub(crate) fn duplicate_sidebar_node(app: &mut App, node_id: NodeId) -> bool {
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return false;
    };
    let Some(tree) = sidebar.tree.as_mut() else {
        return false;
    };
    let Some(src_path) = tree.node_path(node_id).map(Path::to_path_buf) else {
        return false;
    };
    if src_path.is_dir() {
        return false;
    }
    let parent = src_path.parent().unwrap_or(Path::new("."));
    let stem = src_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = src_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let new_name = if ext.is_empty() {
        format!("{stem} copy")
    } else {
        format!("{stem} copy.{ext}")
    };
    let dst_path = parent.join(&new_name);
    if fs::copy(&src_path, &dst_path).is_ok() {
        let root = tree.root();
        let _ = tree.expand(root);
        app.documents.message = Some(format!("Duplicated: {new_name}"));
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

pub(crate) fn reveal_sidebar_node(app: &App, node_id: NodeId) {
    let Some(sidebar) = &app.ui.sidebar else {
        return;
    };
    let Some(tree) = &sidebar.tree else {
        return;
    };
    let Some(path) = tree.node_path(node_id) else {
        return;
    };

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

pub(crate) fn open_sidebar_node_in_terminal(app: &mut App, node_id: NodeId) {
    let Some(sidebar) = &app.ui.sidebar else {
        return;
    };
    let Some(tree) = &sidebar.tree else {
        return;
    };
    let Some(path) = tree.node_path(node_id) else {
        return;
    };
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };

    app.ui.term.visible = true;
    app.ui.term.focused = true;
    let _ = app.ui.term.spawn_session(None, None, Some(&dir));
    app.scheduler.invalidate_all();
}

pub(crate) fn copy_sidebar_node_path(
    app: &mut App,
    ctx: &dyn PlatformContext,
    node_id: NodeId,
    relative: bool,
) {
    let Some(sidebar) = &app.ui.sidebar else {
        return;
    };
    let Some(tree) = &sidebar.tree else {
        return;
    };
    let Some(path) = tree.node_path(node_id) else {
        return;
    };

    let text = if relative && let Some(root) = &sidebar.workspace_root {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    } else {
        path.display().to_string()
    };

    let _ = ctx.clipboard().write_text(&text);
    app.documents.message = Some(format!("Copied: {text}"));
}
