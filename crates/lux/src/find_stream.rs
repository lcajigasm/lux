//! Streams a Find in Files run into the `Find Results` tab.
//!
//! The search runs on `lux-search` worker threads. Each worker hands over
//! one batch per file and wakes the platform loop through the session
//! waker; `background_event` then calls [`pump`], which appends the new
//! batches to the tab. When the workers finish, the summary line is
//! written. Starting another run drops this one, which cancels its workers.

use std::path::{Path, PathBuf};

use lux_search::find_in_files::{SearchMatch, SearchOptions, SearchStream, find_in_files_notify};

use crate::app::App;

use crate::find_results::TAB_TITLE;

#[derive(Debug)]
pub(crate) struct FindRun {
    stream: SearchStream,
    root: PathBuf,
    last_path: Option<PathBuf>,
    files_with_matches: usize,
}

pub(crate) fn header(query: &str) -> String {
    format!("Searching files for \"{query}\"\n\n")
}

/// Appends `m` in Sublime's layout, opening a new path block when the file
/// changes. The block's path is shown relative to `root` when it is under
/// it. Returns true when `m` starts a new file.
pub(crate) fn push_match(
    out: &mut String,
    last_path: &mut Option<PathBuf>,
    root: Option<&Path>,
    m: &SearchMatch,
) -> bool {
    let new_file = last_path.as_deref() != Some(m.path.as_path());
    if new_file {
        if last_path.is_some() {
            out.push('\n');
        }
        let shown = root
            .and_then(|root| m.path.strip_prefix(root).ok())
            .filter(|relative| !relative.as_os_str().is_empty())
            .unwrap_or(&m.path);
        out.push_str(&format!("{}:\n", shown.display()));
        *last_path = Some(m.path.clone());
    }
    let line = m.line.trim_end_matches(['\r', '\n']);
    out.push_str(&format!("  {:>4}: {}\n", m.line_number, line));
    new_file
}

pub(crate) fn summary(matches: usize, files: usize, searched: usize) -> String {
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let lead = if matches == 0 { "" } else { "\n" };
    format!(
        "{lead}{} across {} ({} searched)\n",
        plural(matches, "match", "matches"),
        plural(files, "file", "files"),
        plural(searched, "file", "files"),
    )
}

/// Starts a run: cancels the previous one, resets the tab to the header,
/// and lets the workers fill it in.
pub(crate) fn start(app: &mut App, root: &Path, query: &str, options: &SearchOptions) {
    app.ui.find_results_nav.run = None;
    app.ui.find_results_nav.matches.clear();
    app.ui.find_results_nav.active_index = None;
    app.ui.find_results_nav.root = Some(root.to_path_buf());
    app.ui.find_results_nav.highlight = Some(crate::find_results::decor::run_matcher(
        query,
        options.case_insensitive,
    ));
    let waker = app.ui.session.waker();
    match find_in_files_notify(root, query, options, waker) {
        Ok(stream) => {
            crate::find_results::open_find_results_tab(app, &header(query));
            app.ui.find_results_nav.run = Some(FindRun {
                stream,
                root: root.to_path_buf(),
                last_path: None,
                files_with_matches: 0,
            });
            pump(app);
        }
        Err(error) => {
            let text = format!("{}{error}\n", header(query));
            crate::find_results::open_find_results_tab(app, &text);
        }
    }
}

/// Drains whatever the workers have delivered. Called on every background
/// wake; does nothing when no run is active or nothing new arrived.
pub(crate) fn pump(app: &mut App) {
    let Some(run) = app.ui.find_results_nav.run.as_mut() else {
        return;
    };
    let fresh = run.stream.poll().to_vec();
    let finished = run.stream.is_finished();
    if fresh.is_empty() && !finished {
        return;
    }
    let mut text = String::new();
    for m in &fresh {
        if push_match(&mut text, &mut run.last_path, Some(&run.root), m) {
            run.files_with_matches += 1;
        }
    }
    if finished {
        let total = run.stream.matches().len();
        text.push_str(&summary(
            total,
            run.files_with_matches,
            run.stream.files_searched(),
        ));
        app.ui.find_results_nav.run = None;
    }
    app.ui.find_results_nav.matches.extend(fresh);
    if !append_to_tab(app, &text) {
        // The tab was closed mid-run: dropping the run cancels the workers.
        app.ui.find_results_nav.run = None;
    }
    app.scheduler.invalidate_all();
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
}

#[cfg(test)]
pub(crate) fn is_running(app: &App) -> bool {
    app.ui.find_results_nav.run.is_some()
}

fn append_to_tab(app: &mut App, text: &str) -> bool {
    let Some(tab_id) = app
        .documents
        .tabs
        .tabs()
        .iter()
        .find(|t| t.title == TAB_TITLE)
        .map(|t| t.id)
    else {
        return false;
    };
    if app.documents.active_tab == Some(tab_id) {
        let end = app.buffer.len_chars();
        app.buffer.insert(end, text);
        return true;
    }
    let Some(entry) = app
        .documents
        .entries
        .iter_mut()
        .find(|e| e.tab_id == tab_id)
    else {
        return false;
    };
    let end = entry.buffer.len_chars();
    entry.buffer.insert(end, text);
    true
}

/// Drains the Find/Replace panel's live result list on a background wake.
pub(crate) fn pump_panel(app: &mut App) {
    let Some(state) = app.ui.project_search.as_mut() else {
        return;
    };
    if state.stream.is_none() {
        return;
    }
    state.poll_stream();
    app.scheduler.invalidate_all();
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
}
