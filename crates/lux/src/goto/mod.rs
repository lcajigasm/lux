//! Goto Anything (G09 / ST07): unified fuzzy file jumping, :line, @symbol, #term, and transient live preview.

use std::path::PathBuf;

use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_ui::goto_anything::RecentFiles;
use lux_ui::tabs::TabId;

pub(crate) mod display;
pub(crate) mod preview;
mod search;
pub(crate) mod symbols;

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
    /// Byte ranges of `label` the query matched, as the matcher aligned it.
    pub(crate) hits: Vec<std::ops::Range<usize>>,
    pub(crate) detail: String,
    /// Second line of a file row: where the file lives, relative to the
    /// folder or `~`-abbreviated. Empty for symbol and term rows.
    pub(crate) subtitle: String,
    /// Byte ranges of `subtitle` the query matched.
    pub(crate) subtitle_hits: Vec<std::ops::Range<usize>>,
    pub(crate) path: String,
    pub(crate) line: Option<u32>,
    pub(crate) col: Option<u32>,
    pub(crate) kind: GotoItemKind,
}

/// Snapshot of document state before Goto Anything was opened, used for transient preview rollback on Escape.
#[derive(Debug, Clone)]
pub(crate) struct GotoSnapshot {
    pub(crate) active_tab: Option<TabId>,
    /// The focused split group, which dismissing focuses again.
    pub(crate) group: usize,
    pub(crate) cursors: lux_core::multicursor::MultiCursor,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
    /// Other open tabs whose caret a preview moved.
    pub(crate) touched: Vec<preview::TouchedTab>,
}

impl GotoSnapshot {
    fn take(app: &crate::app::App) -> Self {
        Self {
            active_tab: app.documents.active_tab,
            group: app.ui.split.active_pane,
            cursors: app.cursors.clone(),
            scroll_x: app.viewport.scroll_x,
            scroll_y: app.viewport.scroll_y,
            touched: Vec::new(),
        }
    }
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
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected_index: usize,
    pub(crate) recent_files: RecentFiles,
    pub(crate) history: Vec<String>,
    pub(crate) snapshot: Option<GotoSnapshot>,
    pub(crate) cached_candidates: Vec<String>,
    pub(crate) items: Vec<GotoItem>,
    /// Index state shown in the input, e.g. while the first scan runs.
    pub(crate) notice: Option<&'static str>,
    pub(crate) motion: crate::overlay_motion::OverlayMotion,
    /// The preview read the panel waits for; any other result is stale.
    pub(crate) preview: Option<preview::Ticket>,
}

impl Default for GotoState {
    fn default() -> Self {
        Self {
            mode: GotoMode::default(),
            query: crate::field::QueryField::new(),
            selected_index: 0,
            recent_files: RecentFiles::new(),
            history: Vec::new(),
            snapshot: None,
            cached_candidates: Vec::new(),
            items: Vec::new(),
            notice: None,
            motion: crate::overlay_motion::OverlayMotion::open(),
            preview: None,
        }
    }
}

impl GotoState {
    pub(crate) fn new(app: &crate::app::App) -> Self {
        let snapshot = GotoSnapshot::take(app);
        let cached_candidates = search::collect_candidates(app);
        let mut state = Self {
            mode: GotoMode::Anything,
            query: crate::field::QueryField::new(),
            selected_index: 0,
            recent_files: RecentFiles::new(),
            history: Vec::new(),
            snapshot: Some(snapshot),
            cached_candidates,
            items: Vec::new(),
            notice: None,
            motion: crate::overlay_motion::OverlayMotion::open(),
            preview: None,
        };
        state.refresh_results(app);
        state
    }

    pub(crate) fn new_project_symbols(app: &crate::app::App) -> Self {
        let snapshot = GotoSnapshot::take(app);
        // Symbols come from the background index; no file list is needed.
        let cached_candidates = Vec::new();
        let mut state = Self {
            mode: GotoMode::ProjectSymbols,
            query: crate::field::QueryField::new(),
            selected_index: 0,
            recent_files: RecentFiles::new(),
            history: Vec::new(),
            snapshot: Some(snapshot),
            cached_candidates,
            items: Vec::new(),
            notice: None,
            motion: crate::overlay_motion::OverlayMotion::open(),
            preview: None,
        };
        state.refresh_results(app);
        state
    }

    /// File results carry a path line, so the card uses two-line rows.
    pub(crate) fn two_line(&self) -> bool {
        self.items.iter().any(|item| !item.subtitle.is_empty())
    }

    pub(crate) fn record_open(&mut self, path: &str) {
        self.recent_files.record_open(path);
        if !self.history.contains(&path.to_string()) {
            self.history.push(path.to_string());
        }
    }

    pub(crate) fn refresh_results(&mut self, app: &crate::app::App) {
        (self.items, self.notice) = compute(
            app,
            self.mode,
            &self.query,
            &self.cached_candidates,
            &self.recent_files,
        );
    }

    #[cfg(test)]
    pub(crate) fn select_next(&mut self, total: usize) {
        if total > 0 && self.selected_index + 1 < total {
            self.selected_index += 1;
        }
    }

    #[cfg(test)]
    pub(crate) fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }
}

/// Closes the panel the way Escape does: the transient tab is discarded
/// and the editor goes back to where the panel was opened. Every path that
/// closes the panel without Enter comes here. `true` if it was open.
pub(crate) fn close(app: &mut crate::app::App) -> bool {
    let Some(goto) = app.ui.goto_anything.take() else {
        return false;
    };
    preview::dismiss(app, goto);
    true
}

pub(crate) fn handle_goto_key(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let (total, two_line) = if let Some(goto) = app.ui.goto_anything.as_ref() {
        (goto.items.len(), goto.two_line())
    } else {
        return false;
    };
    let metrics = crate::modal::ModalMetrics::from_app(app);
    let visible = crate::overlay_list::visible_rows_for(
        metrics.vp_height,
        metrics.tab_bar_height,
        metrics.status_bar,
        metrics.scale,
        total,
        two_line,
    );

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            close(app);
            crate::focus::focus_editor(app);
            app.scheduler.invalidate_all();
            true
        }
        key if {
            let Some(goto) = app.ui.goto_anything.as_mut() else {
                return false;
            };
            crate::overlay_list::nudge(&mut goto.selected_index, total, visible, key)
        } =>
        {
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

            if target_path.is_empty() {
                // Nothing to open: leave as Escape does.
                close(app);
            } else if let Some(mut goto) = app.ui.goto_anything.take() {
                goto.record_open(&target_path);
                crate::ui_io::cancel_latest(app);
                execute_goto(app, ctx, &target_path, line, col);
            }
            crate::focus::focus_editor(app);
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

    let (items, notice) = compute(app, mode, &query, &candidates, &recent_files);
    if let Some(goto) = app.ui.goto_anything.as_mut() {
        goto.items = items;
        goto.notice = notice;
    }
    update_live_preview(app);
}

fn compute(
    app: &crate::app::App,
    mode: GotoMode,
    query: &str,
    candidates: &[String],
    recent_files: &RecentFiles,
) -> (Vec<GotoItem>, Option<&'static str>) {
    if mode == GotoMode::ProjectSymbols {
        return symbols::items(app, query);
    }
    let items = search::compute_items(app, query, candidates, recent_files);
    (items, None)
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
    // Whatever is selected now, an earlier read is no longer wanted.
    preview::cancel(app);

    if !target_path.is_empty() {
        let path = PathBuf::from(&target_path);
        let matches = |p: &PathBuf| p == &path || p.ends_with(&path);
        // An unsaved tab is listed by its title.
        let titled = |id: TabId| {
            let tabs = app.documents.tabs.tabs();
            tabs.iter().any(|t| t.id == id && t.title == target_path)
        };
        let is_current = match &app.documents.path {
            Some(p) => matches(p),
            None => {
                search::names_unsaved_current(&target_path)
                    || app.documents.active_tab.is_some_and(titled)
            }
        };
        if !is_current {
            let open = app
                .documents
                .entries
                .iter()
                .find(|e| match &e.path {
                    Some(p) => matches(p),
                    None => titled(e.tab_id),
                })
                .map(|e| e.tab_id);
            match open {
                Some(tab_id) => {
                    preview::note_touched(app, tab_id);
                    app.switch_to_tab(tab_id);
                    if !preview::is_transient(&app.documents, tab_id) {
                        preview::discard(app);
                    }
                }
                None => {
                    // Read off the UI thread; the line is applied on arrival.
                    preview::request(app, resolve(app, &path), line, col);
                    return;
                }
            }
        } else if let Some(tab_id) = app.documents.active_tab {
            preview::note_touched(app, tab_id);
        }
    }

    if let Some(line_num) = line {
        jump_to_line_and_col(app, line_num, col);
    }
}

/// `path` as given, or under the workspace root when it is relative.
fn resolve(app: &crate::app::App, path: &std::path::Path) -> PathBuf {
    if path.is_relative()
        && let Some(sidebar) = &app.ui.sidebar
        && let Some(root) = &sidebar.workspace_root
    {
        root.join(path)
    } else {
        path.to_path_buf()
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
    let full_path = resolve(app, &path);

    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.workspace_root.as_deref());
    let existing_tab = app.documents.entries.iter().find(|e| {
        e.path.as_ref().is_some_and(|p| {
            p == &path
                || p == &full_path
                || (path.is_relative()
                    && root
                        .and_then(|root| display::folder_relative(p, root))
                        .as_ref()
                        == Some(&path))
        }) || app
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
        if preview::is_transient(&app.documents, tab_id) {
            // Enter on the previewed file keeps it, as an ordinary tab.
            preview::promote(app);
            crate::lsp::sync_opened_document(app, ctx);
        } else {
            preview::discard(app);
        }
        if let Some(line_num) = line {
            jump_to_line_and_col(app, line_num, col);
        }
    } else {
        preview::discard(app);
        // The document worker reads the file; the cursor moves when it lands.
        crate::document::open_queue::open_at(app, Some(ctx), full_path, line, col);
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

    let line_height = app.editor_view.metrics().line_height.max(1.0);
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
            query: crate::field::QueryField::from_text("main.rs:42:10"),
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
    fn file_results_carry_the_ranked_match_positions() {
        let app = crate::app::App::new(
            include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
        );
        let mut goto = GotoState {
            query: crate::field::QueryField::from_text("apprs"),
            cached_candidates: vec!["crates/lux/src/app.rs".to_string()],
            ..Default::default()
        };
        goto.refresh_results(&app);
        let item = &goto.items[0];
        let lit: Vec<&str> = item
            .hits
            .iter()
            .map(|span| &item.label[span.clone()])
            .collect();
        // `a`, `p`, `p` of `app` and `r`, `s` of `rs` — not the `a` of `crates`.
        // The title is the file name; the path line keeps the same hits.
        assert_eq!(item.label, "app.rs");
        assert_eq!(lit, ["a", "p", "p", "r", "s"]);
        assert_eq!(item.hits[0], 0..1);
        assert_eq!(item.hits[4], 5..6);
        assert_eq!(item.subtitle, "crates/lux/src/app.rs");
        assert_eq!(item.subtitle_hits[0], 15..16);
        assert_eq!(item.subtitle_hits[4], 20..21);
        assert!(goto.two_line());
    }

    #[test]
    fn an_open_tab_and_its_folder_entry_are_one_result() {
        let dir = std::env::temp_dir().join(format!("lux-goto-dedupe-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let file = dir.join("src/status_bar.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();
        let mut app = crate::app::App::new(
            include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
        );
        app.ui.sidebar = crate::sidebar::SidebarState::open_folder(dir.clone());
        // The open file spells the folder differently (`/private/var` for
        // `/var` on macOS); a plain prefix would list it twice.
        app.documents.initial_path = Some(dir.canonicalize().unwrap().join("src/status_bar.rs"));
        let candidates = search::collect_candidates(&app);
        let hits: Vec<_> = candidates
            .iter()
            .filter(|c| c.ends_with("status_bar.rs"))
            .collect();
        assert_eq!(hits, ["src/status_bar.rs"], "{candidates:?}");
        std::fs::remove_dir_all(&dir).ok();
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
