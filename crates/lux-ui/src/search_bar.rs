//! Search/replace bar (`LUX_PROMPT_MAESTRO.md` Phase 3 deliverable 8):
//! navigation between results and highlighting of every match. The actual
//! search/replace engine — literal, case-insensitive, whole-word, and
//! regex, with capture-group replacement — is `lux-core::find_replace`,
//! already implemented in Phase 1 (`docs/STATUS.md`'s "Find/replace in
//! buffer" row). This module adds no new matching logic; it's the
//! stateful part a UI needs on top of it: which of all the matches is
//! "current" right now, and keeping that in sync as the buffer changes.

use lux_core::buffer::Buffer;
use lux_core::find_replace::{self, Query, QueryError};
use lux_core::history::Transaction;
use lux_core::selection::Selection;

/// Search state for one buffer: the active query, every match it produces
/// (for highlighting all of them), and which one is "current" (for
/// distinct highlighting and for `replace_current`).
#[derive(Debug)]
pub struct SearchBar {
    query: Query,
    matches: Vec<Selection>,
    current: Option<usize>,
    scope: Option<std::ops::Range<usize>>,
}

impl Default for SearchBar {
    fn default() -> Self {
        SearchBar {
            query: Query::new(""),
            matches: Vec::new(),
            current: None,
            scope: None,
        }
    }
}

impl SearchBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query(&self) -> &Query {
        &self.query
    }

    /// The optional search scope restricting matches to a character range.
    pub fn scope(&self) -> Option<&std::ops::Range<usize>> {
        self.scope.as_ref()
    }

    /// Sets or clears the search scope and refreshes matches against `buffer`.
    pub fn set_scope(
        &mut self,
        buffer: &Buffer,
        scope: Option<std::ops::Range<usize>>,
    ) -> Result<(), QueryError> {
        self.scope = scope;
        self.refresh(buffer)
    }

    /// Every match of the current query, in document order — the set to
    /// highlight in the buffer.
    pub fn matches(&self) -> &[Selection] {
        &self.matches
    }

    /// The index into `matches` that's currently active, if any.
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    /// The currently active match, if any.
    pub fn current(&self) -> Option<Selection> {
        self.current.map(|i| self.matches[i])
    }

    /// Sets a new query and recomputes matches against `buffer`, starting
    /// at the first match.
    pub fn set_query(&mut self, buffer: &Buffer, query: Query) -> Result<(), QueryError> {
        self.set_query_scoped(buffer, query, self.scope.clone())
    }

    /// Sets a new query and explicit scope, recomputing matches against `buffer`.
    pub fn set_query_scoped(
        &mut self,
        buffer: &Buffer,
        query: Query,
        scope: Option<std::ops::Range<usize>>,
    ) -> Result<(), QueryError> {
        let mut matches = find_replace::find_all(buffer, &query)?;
        if let Some(ref range) = scope {
            matches.retain(|m| {
                let char_r = m.char_range();
                char_r.start >= range.start && char_r.end <= range.end
            });
        }
        self.current = if matches.is_empty() { None } else { Some(0) };
        self.matches = matches;
        self.query = query;
        self.scope = scope;
        Ok(())
    }

    /// Recomputes matches for the current query against `buffer`.
    pub fn refresh(&mut self, buffer: &Buffer) -> Result<(), QueryError> {
        let mut matches = find_replace::find_all(buffer, &self.query)?;
        if let Some(ref range) = self.scope {
            matches.retain(|m| {
                let char_r = m.char_range();
                char_r.start >= range.start && char_r.end <= range.end
            });
        }
        self.current = if matches.is_empty() {
            None
        } else {
            Some(self.current.unwrap_or(0).min(matches.len() - 1))
        };
        self.matches = matches;
        Ok(())
    }

    /// Moves to the next match, wrapping past the last back to the first.
    /// `None` if there are no matches.
    pub fn next_match(&mut self) -> Option<Selection> {
        let len = self.matches.len();
        if len == 0 {
            return None;
        }
        let i = self.current.map_or(0, |i| (i + 1) % len);
        self.current = Some(i);
        Some(self.matches[i])
    }

    /// Moves to the previous match, wrapping past the first back to the
    /// last. `None` if there are no matches.
    pub fn previous_match(&mut self) -> Option<Selection> {
        let len = self.matches.len();
        if len == 0 {
            return None;
        }
        let i = self.current.map_or(len - 1, |i| (i + len - 1) % len);
        self.current = Some(i);
        Some(self.matches[i])
    }

    /// Replaces the current match, then refreshes.
    pub fn replace_current(
        &mut self,
        buffer: &mut Buffer,
        replacement: &str,
    ) -> Result<Option<Transaction>, QueryError> {
        let Some(i) = self.current else {
            return Ok(None);
        };
        let transaction =
            find_replace::replace_one(buffer, &self.query, self.matches[i], replacement)?;
        self.refresh(buffer)?;
        Ok(Some(transaction))
    }

    /// Replaces every match as one atomic, undoable transaction, then
    /// refreshes.
    pub fn replace_all(
        &mut self,
        buffer: &mut Buffer,
        replacement: &str,
    ) -> Result<Transaction, QueryError> {
        let transaction = if self.scope.is_some() {
            find_replace::replace_matches(buffer, &self.query, &self.matches, replacement)?
        } else {
            find_replace::replace_all(buffer, &self.query, replacement)?
        };
        self.refresh(buffer)?;
        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(bar: &SearchBar) -> Vec<std::ops::Range<usize>> {
        bar.matches().iter().map(|m| m.char_range()).collect()
    }

    #[test]
    fn set_query_computes_all_matches_and_starts_at_the_first() {
        let buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("cat")).unwrap();
        assert_eq!(ranges(&bar), vec![0..3, 8..11]);
        assert_eq!(bar.current_index(), Some(0));
        assert_eq!(bar.current().unwrap().char_range(), 0..3);
    }

    #[test]
    fn set_query_with_no_matches_has_no_current() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("xyz")).unwrap();
        assert!(bar.matches().is_empty());
        assert_eq!(bar.current(), None);
    }

    #[test]
    fn set_query_respects_regex_and_case_options() {
        let buffer = Buffer::from_bytes(b"Foo1 foo22 FOO333").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(
            &buffer,
            Query::new(r"foo\d+").regex(true).case_sensitive(false),
        )
        .unwrap();
        assert_eq!(bar.matches().len(), 3);
    }

    #[test]
    fn set_query_with_invalid_regex_keeps_previous_results() {
        let buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("cat")).unwrap();

        assert!(
            bar.set_query(&buffer, Query::new("(unclosed").regex(true))
                .is_err()
        );
        // The last good query/results are still there — an in-progress,
        // temporarily-invalid pattern doesn't blank the search bar.
        assert_eq!(ranges(&bar), vec![0..3, 8..11]);
        assert_eq!(bar.query().clone(), Query::new("cat"));
    }

    #[test]
    fn next_and_previous_wrap_around() {
        let buffer = Buffer::from_bytes(b"cat bat cat hat cat").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("cat")).unwrap();
        assert_eq!(bar.matches().len(), 3);

        assert_eq!(bar.next_match().unwrap().char_range(), 8..11);
        assert_eq!(bar.next_match().unwrap().char_range(), 16..19);
        assert_eq!(bar.next_match().unwrap().char_range(), 0..3); // wraps

        assert_eq!(bar.previous_match().unwrap().char_range(), 16..19); // wraps back
        assert_eq!(bar.previous_match().unwrap().char_range(), 8..11);
    }

    #[test]
    fn next_and_previous_with_no_matches_return_none() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("xyz")).unwrap();
        assert_eq!(bar.next_match(), None);
        assert_eq!(bar.previous_match(), None);
    }

    #[test]
    fn refresh_reflects_edits_made_outside_the_search_bar() {
        let mut buffer = Buffer::from_bytes(b"cat bat").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("cat")).unwrap();
        assert_eq!(bar.matches().len(), 1);

        lux_core::edit::Edit::insert(7, " cat".to_string()).apply(&mut buffer);
        bar.refresh(&buffer).unwrap();
        assert_eq!(ranges(&bar), vec![0..3, 8..11]);
    }

    #[test]
    fn replace_current_replaces_and_advances_to_what_was_the_next_match() {
        let mut buffer = Buffer::from_bytes(b"cat bat cat hat cat").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("cat")).unwrap();
        bar.next_match(); // move to the second "cat" (index 1)

        bar.replace_current(&mut buffer, "dog").unwrap().unwrap();
        assert_eq!(buffer.text(), "cat bat dog hat cat");
        // The replaced occurrence is gone; index 1 now lands on what was
        // the third "cat".
        assert_eq!(bar.matches().len(), 2);
        assert_eq!(bar.current_index(), Some(1));
        assert_eq!(bar.current().unwrap().char_range(), 16..19);
    }

    #[test]
    fn replace_current_with_no_current_match_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"hello").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("xyz")).unwrap();
        assert_eq!(bar.replace_current(&mut buffer, "x").unwrap(), None);
        assert_eq!(buffer.text(), "hello");
    }

    #[test]
    fn replace_current_is_undoable() {
        let mut buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let original = buffer.text();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("cat")).unwrap();
        let transaction = bar.replace_current(&mut buffer, "dog").unwrap().unwrap();
        transaction.undo(&mut buffer);
        assert_eq!(buffer.text(), original);
    }

    #[test]
    fn replace_all_replaces_every_match_and_leaves_no_matches() {
        let mut buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let mut bar = SearchBar::new();
        bar.set_query(&buffer, Query::new("cat")).unwrap();
        bar.replace_all(&mut buffer, "dog").unwrap();
        assert_eq!(buffer.text(), "dog bat dog");
        assert!(bar.matches().is_empty());
        assert_eq!(bar.current(), None);
    }

    #[test]
    fn scoped_search_and_replace() {
        let mut buffer = Buffer::from_bytes(b"cat cat cat cat").unwrap();
        let mut bar = SearchBar::new();
        // Restrict scope to characters 4..12 ("cat cat ")
        bar.set_query_scoped(&buffer, Query::new("cat"), Some(4..12))
            .unwrap();
        assert_eq!(bar.matches().len(), 2);
        assert_eq!(ranges(&bar), vec![4..7, 8..11]);

        bar.replace_all(&mut buffer, "dog").unwrap();
        assert_eq!(buffer.text(), "cat dog dog cat");
    }
}
