//! Goto Anything (G09 / ST07): unified fuzzy file jumping, :line, @symbol, #term, and transient live preview.

use std::path::PathBuf;

use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_ui::goto_anything::RecentFiles;
use lux_ui::tabs::TabId;

mod search;

/// Classification of a Goto Anything result item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GotoItemKind {
    File {
        line: Option<u32>,
        col: Option<u32>,
    },
    Symbol {
        kind_badge: &'static str,
        line: u32,
        col: u32,
    },
    Term {
        line: u32,
        col: u32,
        preview: String,
    },
}

/// A unified result entry rendered in the Goto Anything floating modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GotoItem {
    pub(crate) label: String,
    pub(crate) detail: String,
    pub(crate) path: String,
    pub(crate) line: Option<u32>,
    pub(crate) col: Option<u32>,
    pub(crate) kind: GotoItemKind,
}

/// Snapshot of document state before Goto Anything was opened, used for transient preview rollback on Escape.
#[derive(Debug, Clone)]
pub(crate) struct GotoSnapshot {
    pub(crate) active_tab: Option<TabId>,
    pub(crate) cursors: lux_core::multicursor::MultiCursor,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
}

/// Mode of operation for Goto Anything / Goto Symbol in Project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum GotoMode {
    #[default]
    Anything,
    ProjectSymbols,
}

pub(crate) struct GotoState {
    pub(crate) mode: GotoMode,
    pub(crate) query: String,
    pub(crate) selected_index: usize,
    pub(crate) recent_files: RecentFiles,
    pub(crate) history: Vec<String>,
    pub(crate) snapshot: Option<GotoSnapshot>,
    pub(crate) cached_candidates: Vec<String>,
    pub(crate) items: Vec<GotoItem>,
}

impl Default for GotoState {
    fn default() -> Self {
        Self {
            mode: GotoMode::default(),
            query: String::new(),
            selected_index: 0,
            recent_files: RecentFiles::new(),
            history: Vec::new(),
            snapshot: None,
            cached_candidates: Vec::new(),
            items: Vec::new(),
        }
    }
}

impl GotoState {
    pub(crate) fn new(app: &crate::app::App) -> Self {
        let snapshot = GotoSnapshot {
            active_tab: app.documents.active_tab,
            cursors: app.cursors.clone(),
            scroll_x: app.viewport.scroll_x,
            scroll_y: app.viewport.scroll_y,
        };
        let cached_candidates = search::collect_candidates(app);
        let mut state = Self {
            mode: GotoMode::Anything,
            query: String::new(),
            selected_index: 0,
            recent_files: RecentFiles::new(),
            history: Vec::new(),
            snapshot: Some(snapshot),
            cached_candidates,
            items: Vec::new(),
        };
        state.refresh_results(app);
        state
    }

    pub(crate) fn new_project_symbols(app: &crate::app::App) -> Self {
        let snapshot = GotoSnapshot {
            active_tab: app.documents.active_tab,
            cursors: app.cursors.clone(),
            scroll_x: app.viewport.scroll_x,
            scroll_y: app.viewport.scroll_y,
        };
        let cached_candidates = search::collect_candidates(app);
        let mut state = Self {
            mode: GotoMode::ProjectSymbols,
            query: String::new(),
            selected_index: 0,
            recent_files: RecentFiles::new(),
            history: Vec::new(),
            snapshot: Some(snapshot),
            cached_candidates,
            items: Vec::new(),
        };
        state.refresh_results(app);
        state
    }

    pub(crate) fn record_open(&mut self, path: &str) {
        self.recent_files.record_open(path);
        if !self.history.contains(&path.to_string()) {
            self.history.push(path.to_string());
        }
    }

    pub(crate) fn refresh_results(&mut self, app: &crate::app::App) {
        self.items = search::compute_items(
            app,
            self.mode,
            &self.query,
            &self.cached_candidates,
            &self.recent_files,
        );
    }

    pub(crate) fn select_next(&mut self, total: usize) {
        if total > 0 && self.selected_index + 1 < total {
            self.selected_index += 1;
        }
    }

    pub(crate) fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }
}

pub(crate) fn handle_goto_key(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let total = if let Some(goto) = app.ui.goto_anything.as_ref() {
        goto.items.len()
    } else {
        return false;
    };

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            if let Some(mut goto) = app.ui.goto_anything.take()
                && let Some(snapshot) = goto.snapshot.take()
            {
                if let Some(orig_tab) = snapshot.active_tab
                    && app.documents.active_tab != Some(orig_tab)
                {
                    app.switch_to_tab(orig_tab);
                }
                app.cursors = snapshot.cursors;
                app.viewport.scroll_x = snapshot.scroll_x;
                app.viewport.scroll_y = snapshot.scroll_y;
                app.reset_goal_columns();
            }
            app.chrome.focus.escape_to_editor();
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            if let Some(goto) = app.ui.goto_anything.as_mut() {
                goto.select_next(total);
            }
            update_live_preview(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if let Some(goto) = app.ui.goto_anything.as_mut() {
                goto.select_prev();
            }
            update_live_preview(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(goto) = app.ui.goto_anything.as_mut() {
                goto.query.pop();
                goto.selected_index = 0;
            }
            refresh_and_preview(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            let (target_path, line, col) = {
                let Some(goto) = &app.ui.goto_anything else {
                    return false;
                };
                if let Some(item) = goto.items.get(goto.selected_index) {
                    (item.path.clone(), item.line, item.col)
                } else {
                    (String::new(), None, None)
                }
            };

            if let Some(mut goto) = app.ui.goto_anything.take()
                && !target_path.is_empty()
            {
                goto.record_open(&target_path);
                execute_goto(app, ctx, &target_path, line, col);
            }
            app.chrome.focus.escape_to_editor();
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(s) => {
            if !app.modifiers.meta && !app.modifiers.control {
                if let Some(goto) = app.ui.goto_anything.as_mut() {
                    goto.query.push_str(s);
                    goto.selected_index = 0;
                }
                refresh_and_preview(app);
                app.scheduler.invalidate_all();
                return true;
            }
            false
        }
        _ => false,
    }
}

fn refresh_and_preview(app: &mut crate::app::App) {
    let (mode, query, candidates) = if let Some(goto) = &app.ui.goto_anything {
        (
            goto.mode,
            goto.query.clone(),
            goto.cached_candidates.clone(),
        )
    } else {
        return;
    };
    let recent_files = if let Some(goto) = &app.ui.goto_anything {
        goto.recent_files.clone()
    } else {
        RecentFiles::new()
    };

    let items = search::compute_items(app, mode, &query, &candidates, &recent_files);
    if let Some(goto) = app.ui.goto_anything.as_mut() {
        goto.items = items;
    }
    update_live_preview(app);
}

pub(crate) fn update_live_preview(app: &mut crate::app::App) {
    let (target_path, line, col) = {
        let Some(goto) = &app.ui.goto_anything else {
            return;
        };
        if let Some(item) = goto.items.get(goto.selected_index) {
            (item.path.clone(), item.line, item.col)
        } else {
            return;
        }
    };

    if !target_path.is_empty() {
        let path = PathBuf::from(&target_path);
        let is_current = app
            .documents
            .path
            .as_ref()
            .is_some_and(|p| p == &path || p.ends_with(&path));
        if !is_current
            && let Some(entry) = app.documents.entries.iter().find(|e| {
                e.path
                    .as_ref()
                    .is_some_and(|p| p == &path || p.ends_with(&path))
            })
        {
            let tab_id = entry.tab_id;
            app.switch_to_tab(tab_id);
        }
    }

    if let Some(line_num) = line {
        jump_to_line_and_col(app, line_num, col);
    }
}

pub(crate) fn execute_goto(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    target_path: &str,
    line: Option<u32>,
    col: Option<u32>,
) {
    let path = PathBuf::from(target_path);
    let full_path = if path.is_relative()
        && let Some(sidebar) = &app.ui.sidebar
        && let Some(root) = &sidebar.workspace_root
    {
        root.join(&path)
    } else {
        path.clone()
    };

    let existing_tab = app.documents.entries.iter().find(|e| {
        e.path
            .as_ref()
            .is_some_and(|p| p == &path || p == &full_path)
            || app
                .documents
                .tabs
                .tabs()
                .iter()
                .find(|t| t.id == e.tab_id)
                .is_some_and(|t| t.title == target_path)
    });

    if let Some(entry) = existing_tab {
        let tab_id = entry.tab_id;
        app.switch_to_tab(tab_id);
    } else if full_path.exists() {
        if let Ok(buffer) = Buffer::open(&full_path) {
            let file_name = full_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            let tab_id = app.documents.tabs.open(file_name);
            let entry = crate::document::DocumentEntry::new(tab_id, Some(full_path), buffer);
            app.documents.entries.push(entry);
            app.switch_to_tab(tab_id);
        } else {
            app.request_document(ctx, crate::document::Request::Open(Some(full_path)));
        }
    } else if path.exists() {
        if let Ok(buffer) = Buffer::open(&path) {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            let tab_id = app.documents.tabs.open(file_name);
            let entry = crate::document::DocumentEntry::new(tab_id, Some(path), buffer);
            app.documents.entries.push(entry);
            app.switch_to_tab(tab_id);
        } else {
            app.request_document(ctx, crate::document::Request::Open(Some(path)));
        }
    }

    if let Some(line_num) = line {
        jump_to_line_and_col(app, line_num, col);
    }
}

pub(crate) fn jump_to_line(app: &mut crate::app::App, line_one_based: u32) {
    jump_to_line_and_col(app, line_one_based, None);
}

pub(crate) fn jump_to_line_and_col(
    app: &mut crate::app::App,
    line_one_based: u32,
    col_one_based: Option<u32>,
) {
    let total_lines = app.buffer.len_lines();
    if total_lines == 0 {
        return;
    }
    let target_line = (line_one_based.saturating_sub(1) as usize).min(total_lines - 1);
    let target_col = col_one_based
        .map(|c| c.saturating_sub(1) as usize)
        .unwrap_or(0);
    let line_start = app.buffer.line_start_byte(target_line).unwrap_or(0);
    let next_line_start = app
        .buffer
        .line_start_byte(target_line + 1)
        .unwrap_or(line_start + app.buffer.len_chars());
    let byte_offset = (line_start + target_col).min(next_line_start);
    let pos = Position::from_byte(&app.buffer, byte_offset);
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(pos));
    app.reset_goal_columns();

    let line_height = crate::app::LINE_HEIGHT * app.scale_factor;
    let target_y = target_line as f32 * line_height;
    let half_h = app.viewport.height * 0.4;
    app.viewport.scroll_y = (target_y - half_h).max(0.0);
    app.scheduler.invalidate_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goto_state_initializes_empty() {
        let goto = GotoState::default();
        assert!(goto.query.is_empty());
        assert_eq!(goto.selected_index, 0);
        assert!(goto.items.is_empty());
    }

    #[test]
    fn goto_search_matches_query_and_line_number() {
        let goto = GotoState {
            query: "main.rs:42:10".to_string(),
            cached_candidates: vec![
                "src/main.rs".to_string(),
                "src/lib.rs".to_string(),
                "tests/test.rs".to_string(),
            ],
            ..Default::default()
        };
        let query = lux_ui::goto_anything::Query::parse(&goto.query);
        assert_eq!(query.line, Some(42));
        assert_eq!(query.col, Some(10));
    }

    #[test]
    fn goto_navigation_clamps_bounds() {
        let mut goto = GotoState::default();
        assert_eq!(goto.selected_index, 0);
        goto.select_prev();
        assert_eq!(goto.selected_index, 0);
        goto.select_next(3);
        assert_eq!(goto.selected_index, 1);
        goto.select_next(2);
        assert_eq!(goto.selected_index, 1);
    }

    #[test]
    fn goto_mode_project_symbols_initialization() {
        let goto = GotoState {
            mode: GotoMode::ProjectSymbols,
            ..Default::default()
        };
        assert_eq!(goto.mode, GotoMode::ProjectSymbols);
    }
}
