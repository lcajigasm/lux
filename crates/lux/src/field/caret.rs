//! Caret movement and selection for a single-line field. Positions are
//! character indices, not bytes.

use super::QueryField;

impl QueryField {
    /// Caret position.
    pub(crate) fn caret(&self) -> usize {
        self.caret
    }

    /// Selected character range, start first. Empty when nothing is
    /// selected.
    pub(crate) fn selection(&self) -> (usize, usize) {
        self.selection_range()
    }

    fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    /// Selects `start..end`, clamped to the text, with the caret at `end`.
    pub(crate) fn select_range(&mut self, start: usize, end: usize) {
        let len = self.len_chars();
        self.anchor = start.min(len);
        self.caret = end.min(len);
    }

    fn place(&mut self, caret: usize, extend: bool) {
        self.caret = caret;
        if !extend {
            self.anchor = caret;
        }
    }

    /// Left arrow. Without `extend`, a selection collapses to its start.
    pub(crate) fn move_left(&mut self, extend: bool) {
        let (start, end) = self.selection_range();
        if !extend && start != end {
            self.place(start, false);
        } else {
            self.place(self.caret.saturating_sub(1), extend);
        }
    }

    /// Right arrow. Without `extend`, a selection collapses to its end.
    pub(crate) fn move_right(&mut self, extend: bool) {
        let (start, end) = self.selection_range();
        if !extend && start != end {
            self.place(end, false);
        } else {
            self.place((self.caret + 1).min(self.len_chars()), extend);
        }
    }

    pub(crate) fn move_home(&mut self, extend: bool) {
        self.place(0, extend);
    }

    pub(crate) fn move_end(&mut self, extend: bool) {
        self.place(self.len_chars(), extend);
    }

    /// Forward delete: the selection, or the character after the caret.
    pub(crate) fn delete_forward(&mut self) {
        let (start, end) = self.selection_range();
        if start == end {
            if self.caret >= self.len_chars() {
                return;
            }
            self.anchor = self.caret + 1;
        }
        // `pop` deletes a non-empty selection and records undo.
        self.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_replaces_a_selected_base_name_and_keeps_the_extension() {
        let mut field = QueryField::from_text("service.rs");
        field.select_range(0, 7);
        assert_eq!(field.selected_text(), "service");
        field.push_str("handler");
        assert_eq!(field.as_str(), "handler.rs");
        assert_eq!(field.caret(), 7);
    }

    #[test]
    fn arrows_home_and_end_move_and_extend() {
        let mut field = QueryField::from_text("añb");
        field.move_home(false);
        assert_eq!(field.caret(), 0);
        field.move_right(true);
        field.move_right(true);
        assert_eq!(field.selection(), (0, 2));
        assert_eq!(field.selected_text(), "añ");
        field.move_left(false);
        assert_eq!(field.selection(), (0, 0));
        field.move_end(false);
        assert_eq!(field.caret(), 3);
        field.move_left(false);
        field.push_str("X");
        assert_eq!(field.as_str(), "añXb");
        field.delete_forward();
        assert_eq!(field.as_str(), "añX");
        field.move_right(false);
        assert_eq!(field.caret(), 3);
    }
}
