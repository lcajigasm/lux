//! Selection and cursor manipulation methods for App.

use lux_core::multicursor::MultiCursor;
use lux_core::selection::Selection;
use lux_core::selection_ops;

use crate::app::App;

impl App {
    pub(crate) fn expand_to_word(&mut self) {
        let selections = self
            .cursors
            .cursors()
            .iter()
            .map(|s| selection_ops::expand_to_word(&self.buffer, *s))
            .collect();
        self.cursors = MultiCursor::from_cursors(selections);
        self.reset_goal_columns();
    }

    pub(crate) fn select_next_occurrence(&mut self) {
        if self.cursors.primary().is_empty() {
            let selections = crate::input::with_primary_last(&self.cursors)
                .into_iter()
                .map(|s| selection_ops::expand_to_word(&self.buffer, s))
                .collect();
            self.cursors = MultiCursor::from_cursors(selections);
        } else {
            self.cursors = selection_ops::select_next_occurrence(&self.buffer, &self.cursors);
        }
        self.reset_goal_columns();
        self.reveal_primary_cursor();
    }

    pub(crate) fn skip_current_occurrence(&mut self) {
        self.cursors = selection_ops::skip_current_occurrence(&self.buffer, &self.cursors);
        self.reset_goal_columns();
        self.reveal_primary_cursor();
    }

    pub(crate) fn select_all_occurrences(&mut self) {
        self.cursors = selection_ops::select_all_occurrences(&self.buffer, &self.cursors);
        self.reset_goal_columns();
        self.reveal_primary_cursor();
    }

    pub(crate) fn split_selection_into_lines(&mut self) {
        let mut new_cursors = Vec::new();
        for &selection in self.cursors.cursors() {
            let split = selection_ops::split_into_lines(&self.buffer, selection);
            new_cursors.extend_from_slice(split.cursors());
        }
        if !new_cursors.is_empty() {
            self.cursors = MultiCursor::from_cursors(new_cursors);
            self.reset_goal_columns();
            self.reveal_primary_cursor();
        }
    }

    pub(crate) fn add_cursor_above(&mut self) {
        let cursors = self.cursors.cursors();
        let min_line = cursors.iter().map(|c| c.head.line).min().unwrap_or(0);
        if min_line > 0 {
            let target_line = min_line - 1;
            let primary_col = self.cursors.primary().head.column;
            let new_pos = selection_ops::position_at_column(&self.buffer, target_line, primary_col);
            let mut all = cursors.to_vec();
            all.push(Selection::cursor(new_pos));
            self.cursors = MultiCursor::from_cursors(all);
            self.reset_goal_columns();
            self.reveal_primary_cursor();
        }
    }

    pub(crate) fn add_cursor_below(&mut self) {
        let cursors = self.cursors.cursors();
        let max_line = cursors.iter().map(|c| c.head.line).max().unwrap_or(0);
        let total_lines = self.buffer.len_lines();
        if max_line + 1 < total_lines {
            let target_line = max_line + 1;
            let primary_col = self.cursors.primary().head.column;
            let new_pos = selection_ops::position_at_column(&self.buffer, target_line, primary_col);
            let mut all = cursors.to_vec();
            all.push(Selection::cursor(new_pos));
            self.cursors = MultiCursor::from_cursors(all);
            self.reset_goal_columns();
            self.reveal_primary_cursor();
        }
    }

    pub(crate) fn expand_to_line(&mut self) {
        let selections = self
            .cursors
            .cursors()
            .iter()
            .map(|s| selection_ops::expand_to_line(&self.buffer, *s))
            .collect();
        self.cursors = MultiCursor::from_cursors(selections);
        self.reset_goal_columns();
        self.reveal_primary_cursor();
    }

    pub(crate) fn expand_to_scope(&mut self) {
        let selections = self
            .cursors
            .cursors()
            .iter()
            .map(|s| selection_ops::expand_to_scope(&self.buffer, *s))
            .collect();
        self.cursors = MultiCursor::from_cursors(selections);
        self.reset_goal_columns();
        self.reveal_primary_cursor();
    }

    pub(crate) fn expand_to_tag(&mut self) {
        let selections = self
            .cursors
            .cursors()
            .iter()
            .map(|s| selection_ops::expand_to_tag(&self.buffer, *s))
            .collect();
        self.cursors = MultiCursor::from_cursors(selections);
        self.reset_goal_columns();
        self.reveal_primary_cursor();
    }

    /// Copies every non-empty selection in document order. Multiple
    /// selections use the document's own line ending so pasting into a
    /// single cursor remains lossless and platform-consistent.
    pub(crate) fn copy_to_clipboard(
        &mut self,
        clipboard: Option<&dyn lux_platform::Clipboard>,
    ) -> bool {
        let Some(clipboard) = clipboard else {
            return false;
        };
        let pieces: Vec<_> = self
            .cursors
            .cursors()
            .iter()
            .filter(|selection| !selection.is_empty())
            .map(|selection| self.buffer.text_range(selection.char_range()))
            .collect();
        if pieces.is_empty() {
            return false;
        }
        let text = pieces.join(self.buffer.eol().as_str());
        match clipboard.write_text(&text) {
            Ok(()) => true,
            Err(error) => {
                self.documents.message = Some(error.to_string());
                false
            }
        }
    }
}
