//! In-process Git integration (Goal G15).
//! Powered by `lux-git` (`gix`-backed, zero shell-outs).
//! Provides gutter change indicators against HEAD, hunk staging/reverting popup,
//! Source Control sidebar panel, diff views, and inline merge conflict resolution.

use std::path::{Path, PathBuf};

use lux_core::buffer::Buffer;
use lux_git::diff_panel::{ChangedFile, changed_files};
use lux_git::diff_view::{InlineDiffLineKind, inline_diff};
use lux_git::gutter::{GutterMarker, Hunk, gutter_markers, hunks_against_head};
use lux_git::merge_conflicts::Resolution;
use lux_git::staging::{stage, stage_hunk, unstage};
use lux_git::status::status;
use lux_platform::PlatformContext;

use crate::app::App;
use crate::document::DocumentEntry;

#[derive(Debug, Clone)]
pub(crate) struct GutterHunkPopup {
    pub(crate) hunk: Hunk,
    pub(crate) line_index: u32,
    pub(crate) rect: [f32; 4],
    pub(crate) stage_btn: [f32; 4],
    pub(crate) revert_btn: [f32; 4],
    pub(crate) close_btn: [f32; 4],
}

#[derive(Debug, Clone)]
pub(crate) struct ConflictMarkerBlock {
    pub(crate) start_line: usize,
    pub(crate) sep_line: usize,
    pub(crate) end_line: usize,
    pub(crate) ours: String,
    pub(crate) theirs: String,
    pub(crate) accept_ours_rect: [f32; 4],
    pub(crate) accept_theirs_rect: [f32; 4],
    pub(crate) accept_both_rect: [f32; 4],
}

#[derive(Default)]
#[allow(dead_code)]
pub(crate) struct GitState {
    pub(crate) repo_root: Option<PathBuf>,
    pub(crate) branch_name: Option<String>,
    pub(crate) hunks: Vec<Hunk>,
    pub(crate) markers: Vec<GutterMarker>,
    pub(crate) changed_files: Vec<ChangedFile>,
    pub(crate) selected_file_index: usize,
    pub(crate) active_popup: Option<GutterHunkPopup>,
    pub(crate) conflict_blocks: Vec<ConflictMarkerBlock>,
    pub(crate) commit_message: String,
    pub(crate) source_control_visible: bool,
}

#[allow(dead_code)]
impl GitState {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

pub(crate) fn discover_repo(start: &Path) -> Option<lux_git::gix::Repository> {
    let start = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };
    lux_git::gix::discover(start).ok()
}

pub(crate) fn refresh_git_state(app: &mut App) {
    let target_path = app
        .documents
        .path
        .clone()
        .or_else(|| {
            app.ui
                .sidebar
                .as_ref()
                .and_then(|s| s.workspace_root.clone())
        })
        .unwrap_or_else(|| PathBuf::from("."));

    // A file open only needs the branch name. Walking the worktree here
    // blocks the UI thread; that walk stays on a directory target.
    if target_path.is_file() {
        app.ui.git.hunks.clear();
        app.ui.git.markers.clear();
        app.ui.git.changed_files.clear();
        app.ui.git.conflict_blocks.clear();
        let Some(repo) = discover_repo(&target_path) else {
            app.ui.git.repo_root = None;
            app.ui.git.branch_name = None;
            return;
        };
        app.ui.git.repo_root = repo.workdir().map(Path::to_path_buf);
        app.ui.git.branch_name = repo
            .head_name()
            .ok()
            .flatten()
            .map(|name| name.shorten().to_string());
        return;
    }

    let Some(repo) = discover_repo(&target_path) else {
        app.ui.git.repo_root = None;
        app.ui.git.branch_name = None;
        app.ui.git.hunks.clear();
        app.ui.git.markers.clear();
        app.ui.git.changed_files.clear();
        app.ui.git.conflict_blocks.clear();
        return;
    };

    let repo_workdir = repo.workdir().map(Path::to_path_buf);
    app.ui.git.repo_root = repo_workdir.clone();

    if let Ok(st) = status(&repo) {
        app.ui.git.branch_name = match st.branch {
            lux_git::status::BranchState::Named(n) => Some(n),
            lux_git::status::BranchState::Unborn(n) => Some(format!("{n} (unborn)")),
            lux_git::status::BranchState::Detached(id) => Some(id.to_string()[..7].to_string()),
        };
    }

    if let Ok(changed) = changed_files(&repo) {
        app.ui.git.changed_files = changed;
    }

    // Refresh active document hunks and markers
    if let (Some(workdir), Some(doc_path)) = (&repo_workdir, &app.documents.path) {
        if let Ok(rel_path) = doc_path.strip_prefix(workdir) {
            let buffer_text = app.buffer.text();
            if let Ok(hunks) = hunks_against_head(&repo, rel_path, &buffer_text) {
                app.ui.git.markers = gutter_markers(&hunks);
                app.ui.git.hunks = hunks;
            } else {
                app.ui.git.hunks.clear();
                app.ui.git.markers.clear();
            }
        }
    } else {
        app.ui.git.hunks.clear();
        app.ui.git.markers.clear();
    }

    // Detect merge conflicts in active document
    app.ui.git.conflict_blocks = find_conflict_blocks(&app.buffer);
}

pub(crate) fn find_conflict_blocks(buffer: &Buffer) -> Vec<ConflictMarkerBlock> {
    let mut blocks = Vec::new();
    let total_lines = buffer.len_lines();

    let mut current_start = None;
    let mut current_sep = None;

    for line_idx in 0..total_lines {
        let line = buffer.line_content(line_idx).unwrap_or_default();
        let trimmed = line.trim_start();
        if trimmed.starts_with("<<<<<<<") {
            current_start = Some(line_idx);
            current_sep = None;
        } else if trimmed.starts_with("=======") && current_start.is_some() {
            current_sep = Some(line_idx);
        } else if trimmed.starts_with(">>>>>>>") && current_start.is_some() && current_sep.is_some()
        {
            let start = current_start.unwrap();
            let sep = current_sep.unwrap();
            let end = line_idx;

            let mut ours = String::new();
            for i in (start + 1)..sep {
                ours.push_str(&buffer.line_content(i).unwrap_or_default());
                ours.push('\n');
            }

            let mut theirs = String::new();
            for i in (sep + 1)..end {
                theirs.push_str(&buffer.line_content(i).unwrap_or_default());
                theirs.push('\n');
            }

            blocks.push(ConflictMarkerBlock {
                start_line: start,
                sep_line: sep,
                end_line: end,
                ours,
                theirs,
                accept_ours_rect: [0.0; 4],
                accept_theirs_rect: [0.0; 4],
                accept_both_rect: [0.0; 4],
            });

            current_start = None;
            current_sep = None;
        }
    }

    blocks
}

pub(crate) fn resolve_conflict(app: &mut App, block_index: usize, resolution: Resolution) {
    let Some(block) = app.ui.git.conflict_blocks.get(block_index).cloned() else {
        return;
    };

    let replacement = match resolution {
        Resolution::Ours => block.ours,
        Resolution::Theirs => block.theirs,
        Resolution::Both => format!("{}{}", block.ours, block.theirs),
    };

    let full_text = app.buffer.text();
    let lines: Vec<&str> = full_text.split('\n').collect();
    let mut result_lines = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i == block.start_line {
            let rep_trimmed = replacement.trim_end_matches('\n');
            if !rep_trimmed.is_empty() {
                result_lines.push(rep_trimmed);
            }
        } else if i > block.start_line && i <= block.end_line {
            continue;
        } else {
            result_lines.push(line);
        }
    }

    let new_text = result_lines.join("\n");
    if let Ok(new_buf) = Buffer::from_bytes(new_text.as_bytes()) {
        app.buffer = new_buf;
    }
    app.update_syntax_on_open();
    refresh_git_state(app);
    app.scheduler.invalidate_all();
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
}

pub(crate) fn stage_gutter_hunk(app: &mut App, hunk: &Hunk) -> bool {
    let Some(repo_root) = &app.ui.git.repo_root else {
        return false;
    };
    let Some(doc_path) = &app.documents.path else {
        return false;
    };
    let Ok(rel_path) = doc_path.strip_prefix(repo_root) else {
        return false;
    };
    let Some(repo) = discover_repo(repo_root) else {
        return false;
    };

    if stage_hunk(&repo, rel_path, hunk).is_ok() {
        app.ui.git.active_popup = None;
        refresh_git_state(app);
        app.scheduler.invalidate_all();
        true
    } else {
        false
    }
}

pub(crate) fn revert_gutter_hunk(app: &mut App, hunk: &Hunk) -> bool {
    let Some(repo_root) = &app.ui.git.repo_root else {
        return false;
    };
    let Some(doc_path) = &app.documents.path else {
        return false;
    };
    let Ok(rel_path) = doc_path.strip_prefix(repo_root) else {
        return false;
    };
    let Some(repo) = discover_repo(repo_root) else {
        return false;
    };

    let head_text = match lux_git::gutter::hunks_against_head(&repo, rel_path, &app.buffer.text()) {
        Ok(_) => {
            let tree_id = match repo.head_tree_id_or_empty() {
                Ok(id) => id,
                Err(_) => return false,
            };
            let tree = match repo.find_tree(tree_id) {
                Ok(t) => t,
                Err(_) => return false,
            };
            match tree.lookup_entry_by_path(rel_path) {
                Ok(Some(entry)) => match entry.object() {
                    Ok(obj) => String::from_utf8(obj.data.clone()).unwrap_or_default(),
                    Err(_) => String::new(),
                },
                _ => String::new(),
            }
        }
        Err(_) => return false,
    };

    let head_lines: Vec<&str> = head_text.lines().collect();
    let before_content = if hunk.before_len > 0 {
        let start = (hunk.before_start.saturating_sub(1)) as usize;
        let end = (start + hunk.before_len as usize).min(head_lines.len());
        let mut text = head_lines[start..end].join("\n");
        text.push('\n');
        text
    } else {
        String::new()
    };

    let full_text = app.buffer.text();
    let lines: Vec<&str> = full_text.split('\n').collect();
    let mut result_lines = Vec::new();
    let start_line = hunk.after_start.saturating_sub(1) as usize;
    let end_line = start_line + hunk.after_len as usize;

    for (i, line) in lines.iter().enumerate() {
        if i == start_line {
            let rep_trimmed = before_content.trim_end_matches('\n');
            if !rep_trimmed.is_empty() {
                result_lines.push(rep_trimmed);
            }
            if hunk.after_len == 0 {
                result_lines.push(line);
            }
        } else if i > start_line && i < end_line {
            continue;
        } else {
            result_lines.push(line);
        }
    }

    let new_text = result_lines.join("\n");
    if let Ok(new_buf) = Buffer::from_bytes(new_text.as_bytes()) {
        app.buffer = new_buf;
    }
    app.ui.git.active_popup = None;
    app.update_syntax_on_open();
    refresh_git_state(app);
    app.scheduler.invalidate_all();
    true
}

pub(crate) fn stage_or_unstage_file(app: &mut App, path: &Path, to_stage: bool) -> bool {
    let Some(repo_root) = &app.ui.git.repo_root else {
        return false;
    };
    let Some(repo) = discover_repo(repo_root) else {
        return false;
    };

    let res = if to_stage {
        stage(&repo, path).is_ok()
    } else {
        unstage(&repo, path).is_ok()
    };

    if res {
        refresh_git_state(app);
        app.scheduler.invalidate_all();
    }
    res
}

pub(crate) fn commit_all_staged(app: &mut App, message: &str) -> bool {
    let Some(repo_root) = &app.ui.git.repo_root else {
        return false;
    };
    let Some(repo) = discover_repo(repo_root) else {
        return false;
    };

    if lux_git::commit::commit(&repo, message, "Lux User", "lux@example.com").is_ok() {
        app.ui.git.commit_message.clear();
        refresh_git_state(app);
        app.scheduler.invalidate_all();
        true
    } else {
        false
    }
}

pub(crate) fn open_file_diff(app: &mut App, file_path: &Path) {
    let Some(repo_root) = &app.ui.git.repo_root else {
        return;
    };
    let Some(repo) = discover_repo(repo_root) else {
        return;
    };

    let head_text = match repo.head_tree_id_or_empty() {
        Ok(tree_id) => match repo.find_tree(tree_id) {
            Ok(tree) => match tree.lookup_entry_by_path(file_path) {
                Ok(Some(entry)) => entry
                    .object()
                    .ok()
                    .and_then(|o| String::from_utf8(o.data.clone()).ok())
                    .unwrap_or_default(),
                _ => String::new(),
            },
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    };

    let working_text = std::fs::read_to_string(repo_root.join(file_path)).unwrap_or_default();
    let diff_lines = inline_diff(&head_text, &working_text);

    let mut content = String::new();
    content.push_str(&format!("--- a/{}\n", file_path.display()));
    content.push_str(&format!("+++ b/{}\n", file_path.display()));
    for l in diff_lines {
        match l.kind {
            InlineDiffLineKind::Context => content.push_str(&format!(" {}\n", l.text)),
            InlineDiffLineKind::Added => content.push_str(&format!("+{}\n", l.text)),
            InlineDiffLineKind::Removed => content.push_str(&format!("-{}\n", l.text)),
        }
    }

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("diff");
    let tab_title = format!("Diff: {file_name}");
    if let Ok(diff_buf) = Buffer::from_bytes(content.as_bytes()) {
        let tab_id = app.documents.tabs.open(&tab_title);
        let entry = DocumentEntry::new(tab_id, None, diff_buf);
        app.documents.entries.push(entry);
        app.switch_to_tab(tab_id);
    }
}

pub(crate) fn handle_git_click(
    app: &mut App,
    _ctx: &dyn PlatformContext,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    // 1. Check Gutter Popup click
    if let Some(popup) = &app.ui.git.active_popup.clone() {
        let [x, y, w, h] = popup.rect;
        if mouse_x >= x && mouse_x <= x + w && mouse_y >= y && mouse_y <= y + h {
            let [sx, sy, sw, sh] = popup.stage_btn;
            if mouse_x >= sx && mouse_x <= sx + sw && mouse_y >= sy && mouse_y <= sy + sh {
                let hunk = popup.hunk;
                stage_gutter_hunk(app, &hunk);
                return true;
            }
            let [rx, ry, rw, rh] = popup.revert_btn;
            if mouse_x >= rx && mouse_x <= rx + rw && mouse_y >= ry && mouse_y <= ry + rh {
                let hunk = popup.hunk;
                revert_gutter_hunk(app, &hunk);
                return true;
            }
            let [cx, cy, cw, ch] = popup.close_btn;
            if mouse_x >= cx && mouse_x <= cx + cw && mouse_y >= cy && mouse_y <= cy + ch {
                app.ui.git.active_popup = None;
                app.scheduler.invalidate_all();
                return true;
            }
            return true;
        }
    }

    // 2. Check Merge Conflict action buttons click
    let blocks = app.ui.git.conflict_blocks.clone();
    for (idx, b) in blocks.iter().enumerate() {
        let [ax, ay, aw, ah] = b.accept_ours_rect;
        if mouse_x >= ax && mouse_x <= ax + aw && mouse_y >= ay && mouse_y <= ay + ah {
            resolve_conflict(app, idx, Resolution::Ours);
            return true;
        }
        let [tx, ty, tw, th] = b.accept_theirs_rect;
        if mouse_x >= tx && mouse_x <= tx + tw && mouse_y >= ty && mouse_y <= ty + th {
            resolve_conflict(app, idx, Resolution::Theirs);
            return true;
        }
        let [bx, by, bw, bh] = b.accept_both_rect;
        if mouse_x >= bx && mouse_x <= bx + bw && mouse_y >= by && mouse_y <= by + bh {
            resolve_conflict(app, idx, Resolution::Both);
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_repo_walks_up_from_a_file() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/git.rs");
        let repo = discover_repo(&file).expect("repository");
        let name = repo.head_name().ok().flatten().expect("named head");
        assert!(!name.shorten().is_empty());
    }

    #[test]
    fn parse_conflict_blocks_detects_standard_markers() {
        let content = "fn main() {\n<<<<<<<\n    ours();\n=======\n    theirs();\n>>>>>>>\n}\n";
        let buf = Buffer::from_bytes(content.as_bytes()).unwrap();
        let blocks = find_conflict_blocks(&buf);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start_line, 1);
        assert_eq!(blocks[0].sep_line, 3);
        assert_eq!(blocks[0].end_line, 5);
        assert!(blocks[0].ours.contains("ours()"));
        assert!(blocks[0].theirs.contains("theirs()"));
    }

    #[test]
    fn resolve_conflict_replaces_text() {
        let content = "fn main() {\n<<<<<<<\n    ours();\n=======\n    theirs();\n>>>>>>>\n}\n";
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.buffer = Buffer::from_bytes(content.as_bytes()).unwrap();
        app.ui.git.conflict_blocks = find_conflict_blocks(&app.buffer);

        resolve_conflict(&mut app, 0, Resolution::Ours);
        assert!(app.buffer.text().contains("ours()"));
        assert!(!app.buffer.text().contains("theirs()"));
        assert!(!app.buffer.text().contains("<<<<<<<"));
    }

    struct DummyContext;
    impl PlatformContext for DummyContext {
        fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
            std::sync::Arc::new(|| {})
        }
        fn background_dialogs(&self) -> std::sync::Arc<dyn lux_platform::FileDialogs> {
            unimplemented!()
        }
        fn create_window(
            &self,
            _: lux_platform::WindowAttributes,
        ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
            unimplemented!()
        }
        fn clipboard(&self) -> &dyn lux_platform::Clipboard {
            unimplemented!()
        }
        fn file_dialogs(&self) -> &dyn lux_platform::FileDialogs {
            unimplemented!()
        }
        fn exit(&self) {}
        fn request_wakeup(&self, _: std::time::Instant) {}
    }

    #[test]
    fn gutter_hunk_popup_hit_testing() {
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        let hunk = Hunk {
            before_start: 1,
            before_len: 2,
            after_start: 1,
            after_len: 3,
        };
        app.ui.git.active_popup = Some(GutterHunkPopup {
            hunk,
            line_index: 1,
            rect: [100.0, 100.0, 200.0, 80.0],
            stage_btn: [110.0, 140.0, 60.0, 25.0],
            revert_btn: [180.0, 140.0, 60.0, 25.0],
            close_btn: [250.0, 140.0, 40.0, 25.0],
        });

        let dummy_ctx = DummyContext;
        // Click on close button
        let handled = handle_git_click(&mut app, &dummy_ctx, 260.0, 150.0);
        assert!(handled);
        assert!(app.ui.git.active_popup.is_none());
    }
}
