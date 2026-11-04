//! Project-wide search and replace in files (embedded ripgrep engine & diff preview).

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_search::find_in_files::{SearchMatch, SearchOptions, SearchStream, find_in_files};
use lux_search::replace_preview::{FileApplyResult, ReplacementPreview, build_preview};
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
pub(crate) use crate::find_results::{
    FindResultsNav, format_find_results, next_result, open_find_result_at_line,
    open_find_results_tab, open_match_in_editor, parse_matches_from_buffer, prev_result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SearchField {
    #[default]
    Find,
    Where,
    Replace,
}

pub(crate) struct ProjectSearchState {
    pub(crate) find_query: crate::field::QueryField,
    pub(crate) where_query: crate::field::QueryField,
    pub(crate) replace_query: crate::field::QueryField,
    pub(crate) files_to_include: String,
    pub(crate) files_to_exclude: String,
    pub(crate) is_replace: bool,
    pub(crate) stream: Option<SearchStream>,
    pub(crate) matches: Vec<SearchMatch>,
    pub(crate) selected_index: usize,
    pub(crate) preview: Option<ReplacementPreview>,
    pub(crate) focused_field: SearchField,
    pub(crate) case_insensitive: bool,
    pub(crate) status_message: Option<String>,
}

pub(crate) fn parse_where_query(where_str: &str) -> (Vec<String>, Vec<String>) {
    let mut includes = Vec::new();
    let mut excludes = Vec::new();
    for part in where_str.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(stripped) = trimmed
            .strip_prefix('-')
            .or_else(|| trimmed.strip_prefix('!'))
        {
            let s = stripped.trim();
            if !s.is_empty() {
                excludes.push(s.to_string());
            }
        } else {
            includes.push(trimmed.to_string());
        }
    }
    (includes, excludes)
}

impl ProjectSearchState {
    pub(crate) fn new(is_replace: bool) -> Self {
        Self {
            find_query: crate::field::QueryField::new(),
            where_query: crate::field::QueryField::new(),
            replace_query: crate::field::QueryField::new(),
            files_to_include: String::new(),
            files_to_exclude: String::new(),
            is_replace,
            stream: None,
            matches: Vec::new(),
            selected_index: 0,
            preview: None,
            focused_field: SearchField::Find,
            case_insensitive: true,
            status_message: None,
        }
    }

    pub(crate) fn start_search(&mut self, root: &Path) {
        let trimmed = self.find_query.trim();
        if trimmed.is_empty() {
            self.stream = None;
            self.matches.clear();
            self.preview = None;
            self.status_message = None;
            return;
        }

        let (mut includes, mut excludes) = parse_where_query(&self.where_query);
        for inc in self
            .files_to_include
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            includes.push(inc.to_string());
        }
        for exc in self
            .files_to_exclude
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            excludes.push(exc.to_string());
        }

        let options = SearchOptions {
            case_insensitive: self.case_insensitive,
            includes,
            excludes,
        };

        match find_in_files(root, trimmed, &options) {
            Ok(stream) => {
                self.stream = Some(stream);
                self.matches.clear();
                self.selected_index = 0;
                self.preview = None;
                self.status_message = None;
                self.poll_stream();
            }
            Err(e) => {
                self.status_message = Some(e.to_string());
            }
        }
    }

    pub(crate) fn poll_stream(&mut self) {
        if let Some(stream) = &mut self.stream {
            let new_matches = stream.poll().to_vec();
            self.matches.extend(new_matches);
            if stream.is_finished() {
                self.stream = None;
            }
        }
    }

    pub(crate) fn build_replace_preview(&mut self) {
        if !self.is_replace || self.find_query.is_empty() {
            return;
        }
        match build_preview(
            &self.matches,
            &self.find_query,
            &self.replace_query,
            self.case_insensitive,
        ) {
            Ok(mut p) => {
                let ids: Vec<_> = p.hunks().iter().map(|h| h.id).collect();
                for id in ids {
                    p.accept(id);
                }
                self.preview = Some(p);
            }
            Err(e) => {
                self.status_message = Some(e.to_string());
            }
        }
    }

    pub(crate) fn apply_replace(&mut self) -> Option<Vec<FileApplyResult>> {
        let preview = self.preview.take()?;
        let results = preview.apply();
        self.status_message = Some(format!("Applied changes to {} files", results.len()));
        Some(results)
    }

    pub(crate) fn select_next(&mut self) {
        if !self.matches.is_empty() && self.selected_index + 1 < self.matches.len() {
            self.selected_index += 1;
        }
    }

    pub(crate) fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    #[cfg(test)]
    pub(crate) fn wait_until_done(&mut self) {
        if let Some(stream) = &mut self.stream {
            stream.wait_until_done();
            self.matches = stream.matches().to_vec();
            self.stream = None;
        }
    }
}

pub(crate) fn execute_find_in_files(app: &mut crate::app::App, _ctx: Option<&dyn PlatformContext>) {
    let (query, _root) = {
        let Some(state) = app.ui.project_search.as_mut() else {
            return;
        };
        let root = app
            .ui
            .sidebar
            .as_ref()
            .and_then(|s| s.workspace_root.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        if state.stream.is_none() && !state.find_query.trim().is_empty() {
            state.start_search(&root);
        }
        #[cfg(test)]
        state.wait_until_done();
        (state.find_query.clone(), root)
    };

    let matches = if let Some(state) = &app.ui.project_search {
        state.matches.clone()
    } else {
        Vec::new()
    };

    let content = format_find_results(&query, &matches);
    open_find_results_tab(app, &content);
    app.ui.find_results_nav = FindResultsNav::new(matches);
    app.ui.project_search = None;
    app.chrome.focus.escape_to_editor();
    app.scheduler.invalidate_all();
}

pub(crate) fn handle_project_search_key(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    if let Some(state) = app.ui.project_search.as_mut() {
        state.poll_stream();
    }

    if app.ui.project_search.is_none() {
        return false;
    }

    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|s| s.workspace_root.clone())
        .unwrap_or_else(|| PathBuf::from("."));

    let state = app.ui.project_search.as_mut().unwrap();

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.ui.project_search = None;
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Tab) => {
            state.focused_field = match state.focused_field {
                SearchField::Find => SearchField::Where,
                SearchField::Where => {
                    if state.is_replace {
                        SearchField::Replace
                    } else {
                        SearchField::Find
                    }
                }
                SearchField::Replace => SearchField::Find,
            };
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            state.select_next();
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            state.select_prev();
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            match state.focused_field {
                SearchField::Find => {
                    state.find_query.pop();
                    state.start_search(&root);
                }
                SearchField::Where => {
                    state.where_query.pop();
                    state.start_search(&root);
                }
                SearchField::Replace => {
                    state.replace_query.pop();
                    state.build_replace_preview();
                }
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            if state.is_replace && state.focused_field == SearchField::Replace {
                state.build_replace_preview();
                let _ = state.apply_replace();
                app.ui.project_search = None;
                app.scheduler.invalidate_all();
                true
            } else {
                execute_find_in_files(app, Some(ctx));
                true
            }
        }
        Key::Character(s) => {
            if !app.modifiers.meta && !app.modifiers.control {
                match state.focused_field {
                    SearchField::Find => {
                        state.find_query.push_str(s.as_str());
                        state.start_search(&root);
                    }
                    SearchField::Where => {
                        state.where_query.push_str(s.as_str());
                        state.start_search(&root);
                    }
                    SearchField::Replace => {
                        state.replace_query.push_str(s.as_str());
                        state.build_replace_preview();
                    }
                }
                app.scheduler.invalidate_all();
                return true;
            }
            false
        }
        _ => false,
    }
}

pub(crate) fn open_project_find(app: &mut crate::app::App) {
    let initial_query = if !app.cursors.primary().is_empty() {
        app.buffer.text_range(app.cursors.primary().char_range())
    } else {
        String::new()
    };
    let mut state = ProjectSearchState::new(false);
    state.find_query = crate::field::QueryField::from_text(initial_query);
    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|s| s.workspace_root.clone())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    state.start_search(&root);
    app.ui.project_search = Some(state);
    app.scheduler.invalidate_all();
}

pub(crate) fn open_project_replace(app: &mut crate::app::App) {
    let initial_query = if !app.cursors.primary().is_empty() {
        app.buffer.text_range(app.cursors.primary().char_range())
    } else {
        String::new()
    };
    let mut state = ProjectSearchState::new(true);
    state.find_query = crate::field::QueryField::from_text(initial_query);
    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|s| s.workspace_root.clone())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    state.start_search(&root);
    app.ui.project_search = Some(state);
    app.scheduler.invalidate_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_search_state_lifecycle() {
        let mut state = ProjectSearchState::new(false);
        assert_eq!(state.focused_field, SearchField::Find);
        assert!(state.matches.is_empty());

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "apple\nbanana\napricot\n").unwrap();

        state.find_query = crate::field::QueryField::from_text("ap");
        state.start_search(temp_dir.path());
        state.wait_until_done();
        assert!(!state.matches.is_empty());

        state.select_next();
        assert_eq!(state.selected_index, 1);
        state.select_prev();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn project_replace_preview_and_apply() {
        let mut state = ProjectSearchState::new(true);
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("replace_me.txt");
        std::fs::write(&file_path, "hello world\n").unwrap();

        state.find_query = crate::field::QueryField::from_text("hello");
        state.replace_query = crate::field::QueryField::from_text("hi");
        state.start_search(temp_dir.path());
        state.wait_until_done();
        assert_eq!(state.matches.len(), 1);

        state.build_replace_preview();
        assert!(state.preview.is_some());

        let results = state.apply_replace().unwrap();
        assert_eq!(results.len(), 1);
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hi world\n");
    }

    #[test]
    fn project_search_with_file_filters() {
        let temp_dir = tempfile::tempdir().unwrap();
        let rs_file = temp_dir.path().join("main.rs");
        let toml_file = temp_dir.path().join("Cargo.toml");
        std::fs::write(&rs_file, "find_target in rust").unwrap();
        std::fs::write(&toml_file, "find_target in toml").unwrap();

        let mut state = ProjectSearchState::new(false);
        state.find_query = crate::field::QueryField::from_text("find_target");
        state.where_query = crate::field::QueryField::from_text("*.toml, -*.rs");
        state.start_search(temp_dir.path());
        state.wait_until_done();

        assert_eq!(state.matches.len(), 1);
        assert!(state.matches[0].path.ends_with("Cargo.toml"));
    }

    #[test]
    fn format_and_parse_find_results() {
        let matches = vec![
            SearchMatch {
                path: PathBuf::from("/path/to/file1.rs"),
                line_number: 10,
                line: "pub fn test1() {}".to_string(),
            },
            SearchMatch {
                path: PathBuf::from("/path/to/file1.rs"),
                line_number: 25,
                line: "test1();".to_string(),
            },
            SearchMatch {
                path: PathBuf::from("/path/to/file2.rs"),
                line_number: 5,
                line: "use test1;".to_string(),
            },
        ];

        let formatted = format_find_results("test1", &matches);
        assert!(formatted.contains("Searching 2 files for \"test1\""));
        assert!(formatted.contains("/path/to/file1.rs:"));
        assert!(formatted.contains("    10: pub fn test1() {}"));
        assert!(formatted.contains("3 matches across 2 files"));

        let buffer = lux_core::buffer::Buffer::from_bytes(formatted.as_bytes()).unwrap();
        let parsed = parse_matches_from_buffer(&buffer);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].line_number, 10);
        assert_eq!(parsed[1].line_number, 25);
        assert_eq!(parsed[2].line_number, 5);
    }
}
