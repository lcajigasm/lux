//! In-process Git integration (Goal G15).
//! Powered by `lux-git` (`gix`-backed, zero shell-outs).
//! Provides gutter change indicators against HEAD, hunk staging/reverting popup,
//! Source Control sidebar panel, diff views, and inline merge conflict resolution.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lux_core::buffer::Buffer;
use lux_git::diff_panel::ChangedFile;
use lux_git::gutter::{GutterMarker, Hunk};
use lux_git::merge_conflicts::Resolution;
use lux_platform::PlatformContext;

use crate::app::App;
use crate::document::DocumentEntry;

pub(crate) mod branch_picker;
mod job;
mod snapshot;

pub(crate) use branch_picker::handle_branch_picker_key;
pub(crate) use job::{
    poll, register_waker, request_blame, request_branch_list, request_commit, request_diff,
    request_history, request_stage_cursor_hunk, request_stage_file, request_status,
    request_unstage_cursor_hunk,
};

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
    pub(crate) branch_picker: Option<branch_picker::BranchPicker>,
    /// Ignores a snapshot from an older refresh.
    pub(crate) generation: u64,
}

#[allow(dead_code)]
impl GitState {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

/// `M`, `A`, or `D` for each changed path, plus the same letter on every
/// parent directory under `root`. The repository root itself is unmarked.
pub(crate) fn git_mark_map(root: &Path, files: &[ChangedFile]) -> HashMap<PathBuf, char> {
    let mut marks = HashMap::new();
    for file in files {
        let letter = match file.status {
            lux_git::diff_panel::ChangeStatus::Added => 'A',
            lux_git::diff_panel::ChangeStatus::Modified => 'M',
            lux_git::diff_panel::ChangeStatus::Deleted => 'D',
        };
        let absolute = root.join(&file.path);
        marks.insert(absolute.clone(), letter);
        let mut parent = absolute.parent();
        while let Some(dir) = parent {
            if dir == root {
                break;
            }
            marks.entry(dir.to_path_buf()).or_insert(letter);
            parent = dir.parent();
        }
    }
    marks
}

/// Probe and test label: relative paths, sorted, `path:M`.
pub(crate) fn git_marks_label(files: &[ChangedFile]) -> String {
    let mut parts = files
        .iter()
        .map(|file| {
            let letter = match file.status {
                lux_git::diff_panel::ChangeStatus::Added => 'A',
                lux_git::diff_panel::ChangeStatus::Modified => 'M',
                lux_git::diff_panel::ChangeStatus::Deleted => 'D',
            };
            format!("{}:{letter}", file.path.display())
        })
        .collect::<Vec<_>>();
    parts.sort();
    parts.join(",")
}

pub(crate) fn publish_git_marks(app: &mut App) {
    let Some(root) = app.ui.git.repo_root.clone() else {
        return;
    };
    let marks = git_mark_map(&root, &app.ui.git.changed_files);
    if let Some(sidebar) = app.ui.sidebar.as_mut() {
        sidebar.git_marks = marks;
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

pub(crate) fn head_label(repo: &lux_git::gix::Repository) -> Option<String> {
    if let Some(name) = repo.head_name().ok().flatten() {
        let short = name.shorten().to_string();
        if !short.is_empty() {
            return Some(short);
        }
    }
    let hex = repo.head_id().ok()?.to_string();
    Some(hex.chars().take(7).collect())
}

pub(crate) fn open_branch_picker(app: &mut App, names: Vec<String>, creating: bool) {
    app.ui.git.branch_picker = Some(branch_picker::BranchPicker::new(names, creating));
    app.scheduler.invalidate_all();
}

pub(crate) fn apply_switched_file(app: &mut App, bytes: &[u8]) {
    if app.document_dirty() {
        return;
    }
    let Ok(buffer) = Buffer::from_bytes(bytes) else {
        return;
    };
    app.buffer = buffer;
    app.history = lux_core::history::History::new();
    app.documents.saved_version = app.history.state_id();
    app.cursors = lux_core::multicursor::MultiCursor::new(lux_core::selection::Selection::cursor(
        lux_core::position::Position::document_start(),
    ));
    app.sync_active_document_to_entry();
    app.update_syntax_on_open();
}

pub(crate) fn refresh_git_state(app: &mut App) {
    app.ui.git.conflict_blocks = find_conflict_blocks(&app.buffer);
    job::request_refresh(app);
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

/// Resolves the conflict block that contains 0-based `line`.
/// Returns false when the caret is not inside a marker block.
pub(crate) fn resolve_conflict_at_line(app: &mut App, line: usize, resolution: Resolution) -> bool {
    app.ui.git.conflict_blocks = find_conflict_blocks(&app.buffer);
    let Some(index) = app
        .ui
        .git
        .conflict_blocks
        .iter()
        .position(|block| line >= block.start_line && line <= block.end_line)
    else {
        app.ui.announcement = Some("No conflict on this line".into());
        return false;
    };
    resolve_conflict(app, index, resolution);
    true
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
    app.sync_active_document_to_entry();
    app.update_syntax_on_open();
    refresh_git_state(app);
    app.scheduler.invalidate_all();
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
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

pub(crate) fn open_text_tab(app: &mut App, title: &str, body: &str) {
    let Ok(buffer) = Buffer::from_bytes(body.as_bytes()) else {
        return;
    };
    let tab_id = app.documents.tabs.open(title);
    app.documents
        .entries
        .push(DocumentEntry::new(tab_id, None, buffer));
    app.switch_to_tab(tab_id);
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
                job::defer_stage_hunk(app, hunk);
                app.ui.git.active_popup = None;
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
mod feature_tests;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_repo_walks_up_from_a_file() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/git/mod.rs");
        let repo = discover_repo(&file).expect("repository");
        let label = head_label(&repo).expect("head");
        assert!(!label.is_empty());
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
        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.buffer = Buffer::from_bytes(content.as_bytes()).unwrap();
        app.ui.git.conflict_blocks = find_conflict_blocks(&app.buffer);

        resolve_conflict(&mut app, 0, Resolution::Ours);
        assert!(app.buffer.text().contains("ours()"));
        assert!(!app.buffer.text().contains("theirs()"));
        assert!(!app.buffer.text().contains("<<<<<<<"));
    }

    #[test]
    fn accept_incoming_uses_the_caret_line() {
        let content = "fn main() {\n<<<<<<<\n    ours();\n=======\n    theirs();\n>>>>>>>\n}\n";
        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.buffer = Buffer::from_bytes(content.as_bytes()).unwrap();
        assert!(resolve_conflict_at_line(&mut app, 4, Resolution::Theirs));
        assert!(app.buffer.text().contains("theirs()"));
        assert!(!app.buffer.text().contains("ours()"));
        assert!(!app.buffer.text().contains("<<<<<<<"));
        assert!(!resolve_conflict_at_line(&mut app, 0, Resolution::Ours));
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
        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
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
