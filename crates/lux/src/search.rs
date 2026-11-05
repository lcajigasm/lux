//! In-Buffer Find & Replace Bar (G10).
//!
//! Provides the state machine, keyboard shortcut routing, and interaction logic
//! for searching and replacing text directly inside the active buffer.

use lux_core::buffer::Buffer;
use lux_core::find_replace::Query;
use lux_core::history::Transaction;
use lux_core::multicursor::MultiCursor;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_ui::search_bar::SearchBar;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchField {
    Find,
    Replace,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SearchBarLayout {
    pub(crate) bar_rect: [f32; 4],
    pub(crate) find_input_rect: [f32; 4],
    pub(crate) replace_input_rect: [f32; 4],
    pub(crate) prev_btn_rect: [f32; 4],
    pub(crate) next_btn_rect: [f32; 4],
    pub(crate) find_btn_rect: [f32; 4],
    pub(crate) find_all_btn_rect: [f32; 4],
    pub(crate) replace_btn_rect: [f32; 4],
    pub(crate) replace_all_btn_rect: [f32; 4],
    pub(crate) case_btn_rect: [f32; 4],
    pub(crate) word_btn_rect: [f32; 4],
    pub(crate) regex_btn_rect: [f32; 4],
    pub(crate) selection_btn_rect: [f32; 4],
    pub(crate) close_btn_rect: [f32; 4],
}

pub(crate) struct SearchBarState {
    pub(crate) search_bar: SearchBar,
    pub(crate) find_query: crate::field::QueryField,
    pub(crate) replace_text: crate::field::QueryField,
    pub(crate) replace_mode: bool,
    pub(crate) focused_field: SearchField,
    pub(crate) case_sensitive: bool,
    pub(crate) whole_word: bool,
    pub(crate) regex: bool,
    pub(crate) in_selection: bool,
    pub(crate) selection_range: Option<std::ops::Range<usize>>,
    pub(crate) layout: SearchBarLayout,
}

impl SearchBarState {
    pub(crate) fn new(replace_mode: bool) -> Self {
        Self {
            search_bar: SearchBar::new(),
            find_query: crate::field::QueryField::new(),
            replace_text: crate::field::QueryField::new(),
            replace_mode,
            focused_field: SearchField::Find,
            case_sensitive: false,
            whole_word: false,
            regex: false,
            in_selection: false,
            selection_range: None,
            layout: SearchBarLayout::default(),
        }
    }

    pub(crate) fn height(&self, scale_factor: f32) -> f32 {
        let base = if self.replace_mode { 64.0 } else { 34.0 };
        base * scale_factor
    }

    pub(crate) fn sync_query(&mut self, buffer: &Buffer) {
        let query = Query::new(self.find_query.as_str())
            .case_sensitive(self.case_sensitive)
            .whole_word(self.whole_word)
            .regex(self.regex);
        let scope = if self.in_selection {
            self.selection_range.clone()
        } else {
            None
        };
        let _ = self.search_bar.set_query_scoped(buffer, query, scope);
    }

    pub(crate) fn toggle_case(&mut self, buffer: &Buffer) {
        self.case_sensitive = !self.case_sensitive;
        self.sync_query(buffer);
    }

    pub(crate) fn toggle_word(&mut self, buffer: &Buffer) {
        self.whole_word = !self.whole_word;
        self.sync_query(buffer);
    }

    pub(crate) fn toggle_regex(&mut self, buffer: &Buffer) {
        self.regex = !self.regex;
        self.sync_query(buffer);
    }

    pub(crate) fn toggle_in_selection(
        &mut self,
        buffer: &Buffer,
        active_range: Option<std::ops::Range<usize>>,
    ) {
        self.in_selection = !self.in_selection;
        if self.in_selection && self.selection_range.is_none() {
            self.selection_range = active_range;
        }
        self.sync_query(buffer);
    }

    pub(crate) fn next_match(&mut self) -> Option<Selection> {
        self.search_bar.next_match()
    }

    pub(crate) fn previous_match(&mut self) -> Option<Selection> {
        self.search_bar.previous_match()
    }

    pub(crate) fn replace_current(&mut self, buffer: &mut Buffer) -> Option<Transaction> {
        self.search_bar
            .replace_current(buffer, &self.replace_text)
            .ok()
            .flatten()
    }

    pub(crate) fn replace_all(&mut self, buffer: &mut Buffer) -> Option<Transaction> {
        self.search_bar.replace_all(buffer, &self.replace_text).ok()
    }
}

pub(crate) fn select_match(app: &mut crate::app::App, selection: Selection) {
    app.cursors = MultiCursor::new(selection);
    app.reset_goal_columns();
    if app.fold_state.is_line_hidden(selection.head.line) {
        app.fold_state.unfold_cursor(selection.head.line);
    }
    app.reveal_primary_cursor();
    if let Some(search) = &app.ui.search_bar {
        let total = search.search_bar.matches().len();
        let current = search.search_bar.current_index().map_or(0, |i| i + 1);
        crate::accessibility::announce(app, format!("{current} of {total} matches"));
    }
    app.scheduler.invalidate_all();
}

pub(crate) fn open_find(app: &mut crate::app::App, replace_mode: bool) {
    let mut initial_query = String::new();
    let mut initial_range = None;
    let mut in_sel = false;

    let primary = app.cursors.primary();
    if !primary.is_empty() {
        if primary.anchor.line == primary.head.line {
            initial_query = app.buffer.text_range(primary.char_range());
        } else {
            initial_range = Some(primary.char_range());
            in_sel = true;
        }
    }

    if let Some(search) = app.ui.search_bar.as_mut() {
        search.replace_mode = replace_mode;
        if !initial_query.is_empty() {
            search.find_query = crate::field::QueryField::from_text(initial_query);
            search.sync_query(&app.buffer);
        }
        if in_sel {
            search.in_selection = true;
            search.selection_range = initial_range;
            search.sync_query(&app.buffer);
        }
        search.focused_field = SearchField::Find;
    } else {
        let mut search = SearchBarState::new(replace_mode);
        if !initial_query.is_empty() {
            search.find_query = crate::field::QueryField::from_text(initial_query);
        }
        if in_sel {
            search.in_selection = true;
            search.selection_range = initial_range;
        }
        search.sync_query(&app.buffer);
        if let Some(first_match) = search.search_bar.current() {
            select_match(app, first_match);
        }
        app.ui.search_bar = Some(search);
        app.update_viewport_dimensions();
    }
    app.chrome
        .focus
        .set_focus(lux_ui::focus::FocusTarget::Search);
    app.scheduler.invalidate_all();
}

/// Sublime's Use Selection for Find. An empty cursor contributes the word
/// under it. A selection on one line is already copied by `open_find`.
pub(crate) fn use_selection_for_find(app: &mut crate::app::App) {
    let primary = app.cursors.primary();
    let word = if primary.is_empty() {
        let word = lux_core::selection_ops::expand_to_word(&app.buffer, primary);
        if word.is_empty() {
            String::new()
        } else {
            app.buffer.text_range(word.char_range())
        }
    } else {
        String::new()
    };
    open_find(app, false);
    if word.is_empty() {
        return;
    }
    if let Some(search) = app.ui.search_bar.as_mut() {
        search.find_query = crate::field::QueryField::from_text(word);
        search.sync_query(&app.buffer);
    }
}

pub(crate) fn find_next(app: &mut crate::app::App) {
    if let Some(search) = app.ui.search_bar.as_mut() {
        if let Some(sel) = search.next_match() {
            select_match(app, sel);
        }
    } else {
        open_find(app, false);
    }
}

pub(crate) fn find_prev(app: &mut crate::app::App) {
    if let Some(search) = app.ui.search_bar.as_mut() {
        if let Some(sel) = search.previous_match() {
            select_match(app, sel);
        }
    } else {
        open_find(app, false);
    }
}

pub(crate) fn find_all(app: &mut crate::app::App) {
    let matches = if let Some(search) = &app.ui.search_bar {
        search.search_bar.matches().to_vec()
    } else {
        Vec::new()
    };
    if !matches.is_empty() {
        let count = matches.len();
        app.cursors = MultiCursor::from_cursors(matches);
        app.reset_goal_columns();
        app.reveal_primary_cursor();
        crate::accessibility::announce(app, format!("Selected {count} matches"));
        app.ui.search_bar = None;
        app.chrome.focus.escape_to_editor();
        app.update_viewport_dimensions();
        app.scheduler.invalidate_all();
    }
}

pub(crate) fn handle_search_key(
    app: &mut crate::app::App,
    _ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let (focused_field, replace_mode) = {
        let Some(search) = app.ui.search_bar.as_ref() else {
            return false;
        };
        (search.focused_field, search.replace_mode)
    };

    if crate::field::search_option_toggle(app, event) {
        return true;
    }

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.ui.search_bar = None;
            app.chrome.focus.escape_to_editor();
            app.update_viewport_dimensions();
            app.reveal_primary_cursor();
            app.scheduler.invalidate_all();
            true
        }

        Key::Named(NamedKey::Tab) => {
            if replace_mode {
                if let Some(search) = app.ui.search_bar.as_mut() {
                    search.focused_field = match search.focused_field {
                        SearchField::Find => SearchField::Replace,
                        SearchField::Replace => SearchField::Find,
                    };
                }
                app.scheduler.invalidate_all();
                true
            } else {
                false
            }
        }
        Key::Named(NamedKey::Enter) => {
            if app.modifiers.alt {
                if focused_field == SearchField::Find {
                    find_all(app);
                    return true;
                } else {
                    let replaced_tx = app
                        .ui
                        .search_bar
                        .as_mut()
                        .and_then(|s| s.replace_all(&mut app.buffer));
                    if let Some(tx) = replaced_tx {
                        app.sync_syntax_incremental(&tx.edits);
                        app.history.record(tx);
                        app.scheduler.invalidate_all();
                    }
                    return true;
                }
            }

            if focused_field == SearchField::Find {
                let matched = if let Some(search) = app.ui.search_bar.as_mut() {
                    if app.modifiers.shift {
                        search.previous_match()
                    } else {
                        search.next_match()
                    }
                } else {
                    None
                };
                if let Some(sel) = matched {
                    select_match(app, sel);
                }
                true
            } else {
                let (replaced_tx, next_sel) = {
                    let Some(search) = app.ui.search_bar.as_mut() else {
                        return true;
                    };
                    let tx = search.replace_current(&mut app.buffer);
                    let sel = search.search_bar.current();
                    (tx, sel)
                };
                if let Some(tx) = replaced_tx {
                    app.sync_syntax_incremental(&tx.edits);
                    app.history.record(tx);
                    if let Some(sel) = next_sel {
                        select_match(app, sel);
                    }
                }
                app.scheduler.invalidate_all();
                true
            }
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(search) = app.ui.search_bar.as_mut() {
                match search.focused_field {
                    SearchField::Find => {
                        search.find_query.pop();
                        search.sync_query(&app.buffer);
                        if let Some(sel) = search.search_bar.current() {
                            select_match(app, sel);
                        }
                    }
                    SearchField::Replace => {
                        search.replace_text.pop();
                    }
                }
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(s) => {
            if !app.modifiers.meta && !app.modifiers.control {
                let next_sel = if let Some(search) = app.ui.search_bar.as_mut() {
                    match search.focused_field {
                        SearchField::Find => {
                            search.find_query.push_str(s);
                            search.sync_query(&app.buffer);
                            search.search_bar.current()
                        }
                        SearchField::Replace => {
                            search.replace_text.push_str(s);
                            None
                        }
                    }
                } else {
                    None
                };
                if let Some(sel) = next_sel {
                    select_match(app, sel);
                } else {
                    app.scheduler.invalidate_all();
                }
                return true;
            }
            false
        }
        _ => false,
    }
}

pub(crate) fn handle_search_click(app: &mut crate::app::App, mouse_x: f32, mouse_y: f32) -> bool {
    let (layout, replace_mode) = {
        let Some(search) = app.ui.search_bar.as_ref() else {
            return false;
        };
        (search.layout.clone(), search.replace_mode)
    };

    if !rect_contains(layout.bar_rect, mouse_x, mouse_y) {
        return false;
    }

    if rect_contains(layout.close_btn_rect, mouse_x, mouse_y) {
        app.ui.search_bar = None;
        app.chrome.focus.escape_to_editor();
        app.update_viewport_dimensions();
        app.reveal_primary_cursor();
        app.scheduler.invalidate_all();
        return true;
    }

    if rect_contains(layout.find_input_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.focused_field = SearchField::Find;
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if replace_mode && rect_contains(layout.replace_input_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.focused_field = SearchField::Replace;
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.prev_btn_rect, mouse_x, mouse_y) {
        if let Some(sel) = app.ui.search_bar.as_mut().and_then(|s| s.previous_match()) {
            select_match(app, sel);
        }
        return true;
    }

    if rect_contains(layout.next_btn_rect, mouse_x, mouse_y) {
        if let Some(sel) = app.ui.search_bar.as_mut().and_then(|s| s.next_match()) {
            select_match(app, sel);
        }
        return true;
    }

    if rect_contains(layout.case_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.toggle_case(&app.buffer);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.word_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.toggle_word(&app.buffer);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.regex_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.toggle_regex(&app.buffer);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.selection_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            let active_range =
                (!app.cursors.primary().is_empty()).then(|| app.cursors.primary().char_range());
            s.toggle_in_selection(&app.buffer, active_range);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if replace_mode && rect_contains(layout.find_btn_rect, mouse_x, mouse_y) {
        if let Some(sel) = app.ui.search_bar.as_mut().and_then(|s| s.next_match()) {
            select_match(app, sel);
        }
        return true;
    }

    if replace_mode && rect_contains(layout.find_all_btn_rect, mouse_x, mouse_y) {
        find_all(app);
        return true;
    }

    if replace_mode && rect_contains(layout.replace_btn_rect, mouse_x, mouse_y) {
        let (replaced_tx, next_sel) = {
            let Some(s) = app.ui.search_bar.as_mut() else {
                return true;
            };
            let tx = s.replace_current(&mut app.buffer);
            let sel = s.search_bar.current();
            (tx, sel)
        };
        if let Some(tx) = replaced_tx {
            app.sync_syntax_incremental(&tx.edits);
            app.history.record(tx);
            if let Some(sel) = next_sel {
                select_match(app, sel);
            }
        }
        return true;
    }

    if replace_mode && rect_contains(layout.replace_all_btn_rect, mouse_x, mouse_y) {
        let replaced_tx = app
            .ui
            .search_bar
            .as_mut()
            .and_then(|s| s.replace_all(&mut app.buffer));
        if let Some(tx) = replaced_tx {
            app.sync_syntax_incremental(&tx.edits);
            app.history.record(tx);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    true
}

fn rect_contains(rect: [f32; 4], x: f32, y: f32) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_bar_state_syncs_query_and_matches() {
        let buffer = Buffer::from_bytes(b"hello world hello").unwrap();
        let mut state = SearchBarState::new(false);
        state.find_query = crate::field::QueryField::from_text("hello");
        state.sync_query(&buffer);
        assert_eq!(state.search_bar.matches().len(), 2);
        assert_eq!(state.search_bar.current_index(), Some(0));

        let next = state.next_match();
        assert!(next.is_some());
        assert_eq!(state.search_bar.current_index(), Some(1));
    }

    #[test]
    fn search_bar_replace_current_and_all() {
        let mut buffer = Buffer::from_bytes(b"foo bar foo").unwrap();
        let mut state = SearchBarState::new(true);
        state.find_query = crate::field::QueryField::from_text("foo");
        state.replace_text = crate::field::QueryField::from_text("baz");
        state.sync_query(&buffer);

        let tx = state.replace_current(&mut buffer);
        assert!(tx.is_some());
        assert_eq!(buffer.text(), "baz bar foo");

        let tx_all = state.replace_all(&mut buffer);
        assert!(tx_all.is_some());
        assert_eq!(buffer.text(), "baz bar baz");
    }
}
