//! Line bookmark data model and navigation operations.
//!
//! Bookmarks allow users to tag specific lines in a document, navigate
//! cyclically forward and backward between them, and place simultaneous
//! cursors across all marked lines.

use std::collections::BTreeSet;

use crate::buffer::Buffer;
use crate::multicursor::MultiCursor;
use crate::position::Position;
use crate::selection::Selection;

/// Tracks 0-indexed line bookmarks for a buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookmarkState {
    lines: BTreeSet<usize>,
}

impl BookmarkState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether any bookmarks are set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Number of active bookmarks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the given 0-indexed line is bookmarked.
    #[must_use]
    pub fn contains(&self, line: usize) -> bool {
        self.lines.contains(&line)
    }

    /// Iterator over sorted 0-indexed bookmarked line numbers.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.lines.iter().copied()
    }

    /// Returns a vector of all bookmarked line numbers in ascending order.
    #[must_use]
    pub fn to_vec(&self) -> Vec<usize> {
        self.lines.iter().copied().collect()
    }

    /// Sets or removes a bookmark at `line`. Returns `true` if added, `false` if removed.
    pub fn toggle(&mut self, line: usize) -> bool {
        if self.lines.remove(&line) {
            false
        } else {
            self.lines.insert(line);
            true
        }
    }

    /// Adds a bookmark at `line`.
    pub fn insert(&mut self, line: usize) -> bool {
        self.lines.insert(line)
    }

    /// Removes a bookmark at `line`.
    pub fn remove(&mut self, line: usize) -> bool {
        self.lines.remove(&line)
    }

    /// Toggles bookmarks at the line of each cursor.
    ///
    /// If all cursor lines already have bookmarks, they are all removed.
    /// If any cursor line does not have a bookmark, bookmarks are added to all of them.
    pub fn toggle_at_cursors(&mut self, cursors: &MultiCursor) -> bool {
        let cursor_lines: Vec<usize> = cursors.cursors().iter().map(|s| s.head.line).collect();
        if cursor_lines.is_empty() {
            return false;
        }

        let all_bookmarked = cursor_lines.iter().all(|line| self.lines.contains(line));
        if all_bookmarked {
            for line in &cursor_lines {
                self.lines.remove(line);
            }
            false
        } else {
            for line in &cursor_lines {
                self.lines.insert(*line);
            }
            true
        }
    }

    /// Finds the next bookmark strictly greater than `current_line`.
    ///
    /// Wraps around to the first bookmark in the document if there are none below `current_line`.
    #[must_use]
    pub fn next_bookmark(&self, current_line: usize) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }

        self.lines
            .range((current_line + 1)..)
            .next()
            .copied()
            .or_else(|| self.lines.iter().next().copied())
    }

    /// Finds the previous bookmark strictly less than `current_line`.
    ///
    /// Wraps around to the last bookmark in the document if there are none above `current_line`.
    #[must_use]
    pub fn prev_bookmark(&self, current_line: usize) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }

        self.lines
            .range(..current_line)
            .next_back()
            .copied()
            .or_else(|| self.lines.iter().next_back().copied())
    }

    /// Clears all bookmarks. Returns `true` if any bookmarks were removed.
    pub fn clear(&mut self) -> bool {
        if self.lines.is_empty() {
            false
        } else {
            self.lines.clear();
            true
        }
    }

    /// Places a cursor on every bookmarked line in the buffer.
    ///
    /// Returns `None` if there are no bookmarks or if all bookmarked lines are out of bounds.
    #[must_use]
    pub fn select_all(&self, buffer: &Buffer) -> Option<MultiCursor> {
        let total_lines = buffer.len_lines();
        if total_lines == 0 || self.lines.is_empty() {
            return None;
        }

        let mut selections = Vec::new();
        for &line in &self.lines {
            if line < total_lines {
                let char_idx = buffer.rope().line_to_char(line);
                selections.push(Selection::cursor(Position::from_char(buffer, char_idx)));
            }
        }

        if selections.is_empty() {
            None
        } else {
            Some(MultiCursor::from_cursors(selections))
        }
    }

    /// Adjusts bookmarks after line insertion or deletion at `at_line`.
    pub fn adjust_for_line_delta(&mut self, at_line: usize, delta_lines: isize) {
        if delta_lines == 0 {
            return;
        }

        let mut updated = BTreeSet::new();
        for &line in &self.lines {
            if line < at_line {
                updated.insert(line);
            } else if delta_lines > 0 {
                updated.insert(line + delta_lines as usize);
            } else {
                let abs_delta = (-delta_lines) as usize;
                if line >= at_line + abs_delta {
                    updated.insert(line - abs_delta);
                }
            }
        }
        self.lines = updated;
    }

    /// Drops any bookmarks with line numbers greater than or equal to `total_lines`.
    pub fn clamp_to_total_lines(&mut self, total_lines: usize) {
        self.lines.retain(|&line| line < total_lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_and_contains() {
        let mut state = BookmarkState::new();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);

        assert!(state.toggle(5));
        assert!(state.contains(5));
        assert_eq!(state.len(), 1);

        assert!(!state.toggle(5));
        assert!(!state.contains(5));
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn test_toggle_at_cursors() {
        let buffer = Buffer::from_bytes(b"line 0\nline 1\nline 2\nline 3\nline 4\n").unwrap();
        let mut state = BookmarkState::new();

        let pos1 = Position::from_char(&buffer, buffer.rope().line_to_char(1));
        let pos3 = Position::from_char(&buffer, buffer.rope().line_to_char(3));
        let cursors =
            MultiCursor::from_cursors(vec![Selection::cursor(pos1), Selection::cursor(pos3)]);

        // Neither line 1 nor 3 is marked -> marks both
        assert!(state.toggle_at_cursors(&cursors));
        assert!(state.contains(1));
        assert!(state.contains(3));
        assert_eq!(state.len(), 2);

        // Now both are marked -> unmarks both
        assert!(!state.toggle_at_cursors(&cursors));
        assert!(!state.contains(1));
        assert!(!state.contains(3));
        assert!(state.is_empty());
    }

    #[test]
    fn test_next_bookmark_cycle() {
        let mut state = BookmarkState::new();
        assert_eq!(state.next_bookmark(0), None);

        state.insert(3);
        state.insert(7);
        state.insert(12);

        // Strictly after current line
        assert_eq!(state.next_bookmark(0), Some(3));
        assert_eq!(state.next_bookmark(3), Some(7));
        assert_eq!(state.next_bookmark(5), Some(7));
        assert_eq!(state.next_bookmark(7), Some(12));

        // Past last bookmark wraps to first
        assert_eq!(state.next_bookmark(12), Some(3));
        assert_eq!(state.next_bookmark(20), Some(3));
    }

    #[test]
    fn test_prev_bookmark_cycle() {
        let mut state = BookmarkState::new();
        assert_eq!(state.prev_bookmark(10), None);

        state.insert(3);
        state.insert(7);
        state.insert(12);

        // Strictly before current line
        assert_eq!(state.prev_bookmark(15), Some(12));
        assert_eq!(state.prev_bookmark(12), Some(7));
        assert_eq!(state.prev_bookmark(9), Some(7));
        assert_eq!(state.prev_bookmark(7), Some(3));

        // Before first bookmark wraps to last
        assert_eq!(state.prev_bookmark(3), Some(12));
        assert_eq!(state.prev_bookmark(1), Some(12));
    }

    #[test]
    fn test_clear() {
        let mut state = BookmarkState::new();
        assert!(!state.clear());

        state.insert(2);
        state.insert(4);
        assert!(state.clear());
        assert!(state.is_empty());
    }

    #[test]
    fn test_select_all() {
        let buffer = Buffer::from_bytes(b"one\ntwo\nthree\nfour\nfive\n").unwrap();
        let mut state = BookmarkState::new();

        assert_eq!(state.select_all(&buffer), None);

        state.insert(1);
        state.insert(3);
        let mc = state.select_all(&buffer).expect("multicursor produced");
        assert_eq!(mc.cursors().len(), 2);
        assert_eq!(mc.cursors()[0].head.line, 1);
        assert_eq!(mc.cursors()[1].head.line, 3);
    }

    #[test]
    fn test_adjust_for_line_delta() {
        let mut state = BookmarkState::new();
        state.insert(2);
        state.insert(5);
        state.insert(8);

        // Inserting 2 lines at line 4: line 2 unchanged, lines 5 and 8 shifted by +2
        state.adjust_for_line_delta(4, 2);
        assert_eq!(state.to_vec(), vec![2, 7, 10]);

        // Deleting 3 lines at line 6: line 2 unchanged, line 7 was inside deleted range [6, 9), line 10 becomes 10 - 3 = 7
        state.adjust_for_line_delta(6, -3);
        assert_eq!(state.to_vec(), vec![2, 7]);
    }
}
