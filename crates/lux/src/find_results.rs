//! Find Results virtual buffer generation and navigation (ST11).

use lux_platform::PlatformContext;
use lux_search::find_in_files::SearchMatch;
use std::path::{Path, PathBuf};

pub(crate) mod decor;

/// Title of the results tab.
pub(crate) const TAB_TITLE: &str = "Find Results";

#[derive(Debug, Default)]
pub(crate) struct FindResultsNav {
    pub(crate) matches: Vec<SearchMatch>,
    pub(crate) active_index: Option<usize>,
    /// The search still streaming into the tab, if any.
    pub(crate) run: Option<crate::find_stream::FindRun>,
    /// Folder the tab's file paths are relative to.
    pub(crate) root: Option<PathBuf>,
    /// Query of the tab and its compiled matcher, for the match boxes.
    pub(crate) highlight: Option<(String, Option<regex::Regex>)>,
}

/// The `N matches across M files (K files searched)` line.
pub(crate) fn is_summary(trimmed: &str) -> bool {
    trimmed.contains(" across ") && trimmed.ends_with(" searched)")
}

/// Folder relative result paths are read against: the search's own root,
/// or the workspace for a tab restored from a previous session.
fn results_root(app: &crate::app::App) -> PathBuf {
    app.ui
        .find_results_nav
        .root
        .clone()
        .unwrap_or_else(|| crate::project_search::workspace_root(app))
}

/// A header line's path. Older tabs hold absolute paths, which `join`
/// keeps as they are.
fn header_path(root: &Path, header: &str) -> PathBuf {
    root.join(header.trim_end_matches(':').trim())
}

#[cfg(test)]
/// The complete tab text for a finished search, built from the same pieces
/// the streaming run appends one batch at a time.
pub(crate) fn format_find_results(
    query: &str,
    root: Option<&Path>,
    matches: &[SearchMatch],
    searched: usize,
) -> String {
    use crate::find_stream::{header, push_match, summary};
    let mut out = header(query);
    let mut last_path: Option<PathBuf> = None;
    let mut files = 0;
    for m in matches {
        if push_match(&mut out, &mut last_path, root, m) {
            files += 1;
        }
    }
    out.push_str(&summary(matches.len(), files, searched));
    out
}

pub(crate) fn open_find_results_tab(app: &mut crate::app::App, content: &str) {
    use lux_core::buffer::Buffer;
    use lux_core::multicursor::MultiCursor;
    use lux_core::position::Position;
    use lux_core::selection::Selection;

    let existing_tab = app
        .documents
        .entries
        .iter()
        .find(|e| {
            app.documents
                .tabs
                .tabs()
                .iter()
                .find(|t| t.id == e.tab_id)
                .is_some_and(|t| t.title == "Find Results")
        })
        .map(|e| e.tab_id);

    let buffer = Buffer::from_bytes(content.as_bytes()).unwrap();

    if let Some(tab_id) = existing_tab {
        if app.documents.active_tab == Some(tab_id) {
            app.buffer = buffer;
            app.history = lux_core::history::History::new();
            app.documents.saved_version = app.history.state_id();
            app.cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
            app.reset_goal_columns();
        } else {
            if let Some(entry) = app
                .documents
                .entries
                .iter_mut()
                .find(|e| e.tab_id == tab_id)
            {
                entry.buffer = buffer;
                entry.history = lux_core::history::History::new();
                entry.saved_version = entry.history.state_id();
                entry.cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
                entry.goal_columns = vec![0];
                entry.scroll_x = 0.0;
                entry.scroll_y = 0.0;
            }
            app.switch_to_tab(tab_id);
        }
    } else {
        let tab_id = app.documents.tabs.open("Find Results");
        let entry = crate::document::DocumentEntry::new(tab_id, None, buffer);
        app.documents.entries.push(entry);
        app.switch_to_tab(tab_id);
    }
}

/// Shows `matched` in an editor tab. An already open file is switched to
/// at once; any other file goes through the document worker's open queue
/// (no disk access here), and the CLI location gate places the cursor on
/// the matched line once the worker has loaded it. Without a platform
/// context there is no worker to open it, so nothing happens.
pub(crate) fn open_match_in_editor(
    app: &mut crate::app::App,
    ctx: Option<&dyn PlatformContext>,
    matched: &SearchMatch,
) {
    let path = matched.path.clone();
    let line_num = matched.line_number as u32;

    let existing_tab = app
        .documents
        .entries
        .iter()
        .find(|e| e.path.as_ref() == Some(&path))
        .map(|e| e.tab_id);

    if let Some(tab_id) = existing_tab {
        app.switch_to_tab(tab_id);
        crate::goto::jump_to_line(app, line_num);
    } else if let Some(ctx) = ctx {
        crate::document::open_queue::open_at(app, Some(ctx), path, Some(line_num), None);
    }
}

pub(crate) fn open_find_result_at_line(
    app: &mut crate::app::App,
    ctx: Option<&dyn PlatformContext>,
    line_idx: usize,
) -> bool {
    let line_content = app.buffer.line_content(line_idx).unwrap_or_default();
    let trimmed = line_content.trim();
    let root = results_root(app);

    if let Some((num_str, _)) = trimmed.split_once(':')
        && let Ok(line_num) = num_str.trim().parse::<u32>()
    {
        for prev in (0..line_idx).rev() {
            let prev_line = app.buffer.line_content(prev).unwrap_or_default();
            let prev_trimmed = prev_line.trim();
            if prev_trimmed.ends_with(':')
                && !prev_trimmed.starts_with("Searching ")
                && !is_summary(prev_trimmed)
            {
                let path = header_path(&root, prev_trimmed);
                open_match_in_editor(
                    app,
                    ctx,
                    &SearchMatch {
                        path,
                        line_number: line_num as u64,
                        line: String::new(),
                    },
                );
                return true;
            }
        }
    }

    if trimmed.ends_with(':') && !trimmed.starts_with("Searching ") && !is_summary(trimmed) {
        let path = header_path(&root, trimmed);
        open_match_in_editor(
            app,
            ctx,
            &SearchMatch {
                path,
                line_number: 1,
                line: String::new(),
            },
        );
        return true;
    }

    false
}

pub(crate) fn parse_matches_from_buffer(
    buffer: &lux_core::buffer::Buffer,
    root: &Path,
) -> Vec<SearchMatch> {
    let mut matches = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let lines_count = buffer.len_lines();

    for i in 0..lines_count {
        let line = buffer.line_content(i).unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.starts_with("Searching ") || is_summary(trimmed) || trimmed.is_empty() {
            continue;
        }
        if trimmed.ends_with(':') && !line.starts_with(' ') {
            current_path = Some(header_path(root, trimmed));
            continue;
        }
        if let Some(path) = &current_path
            && let Some((num_str, rest)) = trimmed.split_once(':')
            && let Ok(line_number) = num_str.trim().parse::<u64>()
        {
            matches.push(SearchMatch {
                path: path.clone(),
                line_number,
                line: rest.trim_start().to_string(),
            });
        }
    }

    matches
}

pub(crate) fn next_result(app: &mut crate::app::App, ctx: Option<&dyn PlatformContext>) {
    if app.ui.find_results_nav.matches.is_empty()
        && let Some(buf) = app.find_results_buffer()
    {
        app.ui.find_results_nav.matches = parse_matches_from_buffer(&buf, &results_root(app));
    }
    if app.ui.find_results_nav.matches.is_empty() {
        return;
    }
    let count = app.ui.find_results_nav.matches.len();
    let next_idx = match app.ui.find_results_nav.active_index {
        Some(curr) => (curr + 1) % count,
        None => 0,
    };
    app.ui.find_results_nav.active_index = Some(next_idx);
    let matched = app.ui.find_results_nav.matches[next_idx].clone();
    open_match_in_editor(app, ctx, &matched);
    crate::accessibility::announce(
        app,
        format!(
            "{}/{} matches: {}:{}",
            next_idx + 1,
            count,
            matched.path.display(),
            matched.line_number
        ),
    );
}

pub(crate) fn prev_result(app: &mut crate::app::App, ctx: Option<&dyn PlatformContext>) {
    if app.ui.find_results_nav.matches.is_empty()
        && let Some(buf) = app.find_results_buffer()
    {
        app.ui.find_results_nav.matches = parse_matches_from_buffer(&buf, &results_root(app));
    }
    if app.ui.find_results_nav.matches.is_empty() {
        return;
    }
    let count = app.ui.find_results_nav.matches.len();
    let prev_idx = match app.ui.find_results_nav.active_index {
        Some(curr) => (curr + count - 1) % count,
        None => count.saturating_sub(1),
    };
    app.ui.find_results_nav.active_index = Some(prev_idx);
    let matched = app.ui.find_results_nav.matches[prev_idx].clone();
    open_match_in_editor(app, ctx, &matched);
    crate::accessibility::announce(
        app,
        format!(
            "{}/{} matches: {}:{}",
            prev_idx + 1,
            count,
            matched.path.display(),
            matched.line_number
        ),
    );
}

pub(crate) fn is_find_results_active(app: &crate::app::App) -> bool {
    let Some(active_id) = app.documents.active_tab else {
        return false;
    };
    app.documents
        .tabs
        .tabs()
        .iter()
        .any(|t| t.id == active_id && t.title == "Find Results")
}

pub(crate) fn find_results_buffer(app: &crate::app::App) -> Option<lux_core::buffer::Buffer> {
    // The active tab's live text is `app.buffer`; its entry is only a
    // snapshot taken when another tab was activated.
    if is_find_results_active(app) {
        return Some(app.buffer.clone());
    }
    app.documents
        .entries
        .iter()
        .find(|e| {
            app.documents
                .tabs
                .tabs()
                .iter()
                .any(|t| t.id == e.tab_id && t.title == "Find Results")
        })
        .map(|e| e.buffer.clone())
}
