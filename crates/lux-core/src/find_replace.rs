//! Find and replace within a buffer: literal, case-insensitive, whole-word,
//! and regex search, with capture-group replacement. One `Query` drives all
//! four search modes through a single compiled `regex::Regex` — literal
//! text becomes a regex via `regex::escape`, whole-word wraps it in `\b`,
//! and case-insensitivity is the `(?i)` flag — rather than maintaining a
//! separate literal-search code path alongside a regex one.

use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::history::Transaction;
use crate::position::Position;
use crate::selection::Selection;
use regex::{Captures, Regex};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pattern: String,
    case_sensitive: bool,
    whole_word: bool,
    is_regex: bool,
}

impl Query {
    /// A literal, case-sensitive query — the default; chain `case_sensitive`,
    /// `whole_word`, and/or `regex` to change it.
    pub fn new(pattern: impl Into<String>) -> Query {
        Query {
            pattern: pattern.into(),
            case_sensitive: true,
            whole_word: false,
            is_regex: false,
        }
    }

    pub fn case_sensitive(mut self, case_sensitive: bool) -> Query {
        self.case_sensitive = case_sensitive;
        self
    }

    pub fn whole_word(mut self, whole_word: bool) -> Query {
        self.whole_word = whole_word;
        self
    }

    pub fn regex(mut self, is_regex: bool) -> Query {
        self.is_regex = is_regex;
        self
    }

    fn compile(&self) -> Result<Regex, QueryError> {
        let core = if self.is_regex {
            self.pattern.clone()
        } else {
            regex::escape(&self.pattern)
        };
        let bounded = if self.whole_word {
            format!(r"\b{core}\b")
        } else {
            core
        };
        let flagged = if self.case_sensitive {
            bounded
        } else {
            format!("(?i){bounded}")
        };
        Regex::new(&flagged).map_err(|e| QueryError(e.to_string()))
    }
}

/// An invalid regex pattern (only possible when `Query::regex(true)`; a
/// literal pattern always compiles since it's escaped first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError(pub String);

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid search pattern: {}", self.0)
    }
}

impl std::error::Error for QueryError {}

fn expand_for_match(query: &Query, caps: &Captures, replacement: &str) -> String {
    if query.is_regex {
        let mut dest = String::new();
        caps.expand(replacement, &mut dest);
        dest
    } else {
        // No `$1`-style expansion outside regex mode — a literal `$` in the
        // replacement is just a literal `$`.
        replacement.to_string()
    }
}

/// Every match of `query` in `buffer`, in document order. Empty if the
/// pattern is empty.
pub fn find_all(buffer: &Buffer, query: &Query) -> Result<Vec<Selection>, QueryError> {
    if query.pattern.is_empty() {
        return Ok(Vec::new());
    }
    let regex = query.compile()?;
    let text = buffer.text();
    Ok(regex
        .find_iter(&text)
        .map(|m| {
            Selection::new(
                Position::from_byte(buffer, m.start()),
                Position::from_byte(buffer, m.end()),
            )
        })
        .collect())
}

/// The first match starting at or after `after`, wrapping around to the
/// document start if none does. `None` if there are no matches at all.
pub fn find_next(
    buffer: &Buffer,
    query: &Query,
    after: Position,
) -> Result<Option<Selection>, QueryError> {
    let matches = find_all(buffer, query)?;
    Ok(matches
        .iter()
        .find(|m| m.char_range().start >= after.char)
        .or_else(|| matches.first())
        .copied())
}

/// Replaces the text at `match_selection` (which must be exactly one match
/// of `query`) with `replacement`, expanding `$1`/`${name}` capture
/// references when `query` is a regex.
pub fn replace_one(
    buffer: &mut Buffer,
    query: &Query,
    match_selection: Selection,
    replacement: &str,
) -> Result<Transaction, QueryError> {
    let regex = query.compile()?;
    let range = match_selection.char_range();
    let matched_text = buffer.text_range(range.clone());
    let caps = regex
        .captures(&matched_text)
        .filter(|caps| {
            caps.get(0)
                .is_some_and(|m| m.start() == 0 && m.end() == matched_text.len())
        })
        .ok_or_else(|| QueryError("selection is not an exact match of the query".to_string()))?;
    let replaced = expand_for_match(query, &caps, replacement);

    let delete_edit = Edit::delete(buffer, range.start, range.len());
    delete_edit.apply(buffer);
    let mut edits = vec![delete_edit];
    if !replaced.is_empty() {
        let insert_edit = Edit::insert(range.start, replaced.clone());
        insert_edit.apply(buffer);
        edits.push(insert_edit);
    }

    let new_pos = Position::from_char(buffer, range.start + replaced.chars().count());
    Ok(Transaction::new(
        edits,
        vec![match_selection],
        vec![Selection::cursor(new_pos)],
    ))
}

/// Replaces every match of `query` in the buffer with `replacement`
/// (expanding captures in regex mode), as one atomic transaction. Edits are
/// applied back-to-front so an earlier match's char offsets stay valid
/// while later ones are still being replaced.
pub fn replace_all(
    buffer: &mut Buffer,
    query: &Query,
    replacement: &str,
) -> Result<Transaction, QueryError> {
    if query.pattern.is_empty() {
        return Ok(Transaction::new(Vec::new(), Vec::new(), Vec::new()));
    }
    let regex = query.compile()?;
    let text = buffer.text();

    let planned: Vec<(usize, usize, String)> = regex
        .captures_iter(&text)
        .map(|caps| {
            let m = caps.get(0).expect("capture group 0 always matches");
            let replaced = expand_for_match(query, &caps, replacement);
            let start_char = Position::from_byte(buffer, m.start()).char;
            let end_char = Position::from_byte(buffer, m.end()).char;
            (start_char, end_char, replaced)
        })
        .collect();

    let mut edits = Vec::with_capacity(planned.len() * 2);
    for (start_char, end_char, replaced) in planned.iter().rev() {
        let delete_edit = Edit::delete(buffer, *start_char, end_char - start_char);
        delete_edit.apply(buffer);
        edits.push(delete_edit);
        if !replaced.is_empty() {
            let insert_edit = Edit::insert(*start_char, replaced.clone());
            insert_edit.apply(buffer);
            edits.push(insert_edit);
        }
    }
    // No final reverse: `edits` is already in the exact chronological order
    // it was applied to `buffer` (highest-offset match first, each as
    // delete-then-insert) — that's what `Transaction::apply`/`undo` need to
    // replay or unwind it correctly.
    Ok(Transaction::new(edits, Vec::new(), Vec::new()))
}

/// Replaces the specific matches in `matches` with `replacement`
/// (expanding captures in regex mode), as one atomic transaction.
/// `matches` should be in document order; edits are applied back-to-front
/// so that character offsets of preceding matches remain valid.
pub fn replace_matches(
    buffer: &mut Buffer,
    query: &Query,
    matches: &[Selection],
    replacement: &str,
) -> Result<Transaction, QueryError> {
    if matches.is_empty() || query.pattern.is_empty() {
        return Ok(Transaction::new(Vec::new(), Vec::new(), Vec::new()));
    }
    let regex = query.compile()?;
    let mut edits = Vec::with_capacity(matches.len() * 2);

    for sel in matches.iter().rev() {
        let range = sel.char_range();
        let matched_text = buffer.text_range(range.clone());
        if let Some(caps) = regex.captures(&matched_text).filter(|c| {
            c.get(0)
                .is_some_and(|m| m.start() == 0 && m.end() == matched_text.len())
        }) {
            let replaced = expand_for_match(query, &caps, replacement);
            let delete_edit = Edit::delete(buffer, range.start, range.len());
            delete_edit.apply(buffer);
            edits.push(delete_edit);
            if !replaced.is_empty() {
                let insert_edit = Edit::insert(range.start, replaced);
                insert_edit.apply(buffer);
                edits.push(insert_edit);
            }
        }
    }

    Ok(Transaction::new(edits, Vec::new(), Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    #[test]
    fn find_all_literal() {
        let buffer = Buffer::from_bytes(b"cat cats category").unwrap();
        let query = Query::new("cat");
        let matches = find_all(&buffer, &query).unwrap();
        let ranges: Vec<_> = matches.iter().map(|m| m.char_range()).collect();
        assert_eq!(ranges, vec![0..3, 4..7, 9..12]);
    }

    #[test]
    fn find_all_literal_treats_regex_metacharacters_as_plain_text() {
        let buffer = Buffer::from_bytes(b"a.b a.b axb").unwrap();
        let query = Query::new("a.b");
        let matches = find_all(&buffer, &query).unwrap();
        assert_eq!(matches.len(), 2); // not the "axb" that "." would match as regex
    }

    #[test]
    fn find_all_case_insensitive() {
        let buffer = Buffer::from_bytes(b"Cat cat CAT").unwrap();
        let query = Query::new("cat").case_sensitive(false);
        let matches = find_all(&buffer, &query).unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_all_case_sensitive_by_default() {
        let buffer = Buffer::from_bytes(b"Cat cat CAT").unwrap();
        let query = Query::new("cat");
        let matches = find_all(&buffer, &query).unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn find_all_whole_word_excludes_partial_matches() {
        let buffer = Buffer::from_bytes(b"cat cats category cat").unwrap();
        let query = Query::new("cat").whole_word(true);
        let matches = find_all(&buffer, &query).unwrap();
        let ranges: Vec<_> = matches.iter().map(|m| m.char_range()).collect();
        assert_eq!(ranges, vec![0..3, 18..21]);
    }

    #[test]
    fn find_all_regex() {
        let buffer = Buffer::from_bytes(b"a1 b22 c333").unwrap();
        let query = Query::new(r"\d+").regex(true);
        let matches = find_all(&buffer, &query).unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_all_regex_and_case_insensitive_combine() {
        let buffer = Buffer::from_bytes(b"Foo123 foo456").unwrap();
        let query = Query::new(r"foo\d+").regex(true).case_sensitive(false);
        let matches = find_all(&buffer, &query).unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn find_all_invalid_regex_returns_an_error() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        let query = Query::new("(unclosed").regex(true);
        assert!(find_all(&buffer, &query).is_err());
    }

    #[test]
    fn find_all_empty_pattern_matches_nothing() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        let query = Query::new("");
        assert_eq!(find_all(&buffer, &query).unwrap(), Vec::new());
    }

    #[test]
    fn find_next_wraps_around() {
        let buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let query = Query::new("cat");
        let after = pos_at(&buffer, 11); // past the last match
        let next = find_next(&buffer, &query, after).unwrap().unwrap();
        assert_eq!(next.char_range(), 0..3); // wraps to the first
    }

    #[test]
    fn find_next_returns_none_with_no_matches() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        let query = Query::new("xyz");
        assert_eq!(
            find_next(&buffer, &query, pos_at(&buffer, 0)).unwrap(),
            None
        );
    }

    #[test]
    fn replace_one_literal() {
        let mut buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let query = Query::new("cat");
        let m = find_all(&buffer, &query).unwrap()[0];
        replace_one(&mut buffer, &query, m, "dog").unwrap();
        assert_eq!(buffer.text(), "dog bat cat");
    }

    #[test]
    fn replace_one_restores_via_returned_transaction() {
        let mut buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let original = buffer.text();
        let query = Query::new("cat");
        let m = find_all(&buffer, &query).unwrap()[0];
        let transaction = replace_one(&mut buffer, &query, m, "dog").unwrap();
        transaction.undo(&mut buffer);
        assert_eq!(buffer.text(), original);
    }

    #[test]
    fn replace_one_with_capture_group_reference() {
        let mut buffer = Buffer::from_bytes(b"2026-07-27").unwrap();
        let query = Query::new(r"(\d+)-(\d+)-(\d+)").regex(true);
        let m = find_all(&buffer, &query).unwrap()[0];
        replace_one(&mut buffer, &query, m, "$3/$2/$1").unwrap();
        assert_eq!(buffer.text(), "27/07/2026");
    }

    #[test]
    fn replace_one_literal_dollar_sign_is_not_expanded() {
        let mut buffer = Buffer::from_bytes(b"price: X").unwrap();
        let query = Query::new("X");
        let m = find_all(&buffer, &query).unwrap()[0];
        replace_one(&mut buffer, &query, m, "$1").unwrap();
        assert_eq!(buffer.text(), "price: $1");
    }

    #[test]
    fn replace_all_literal() {
        let mut buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let query = Query::new("cat");
        replace_all(&mut buffer, &query, "dog").unwrap();
        assert_eq!(buffer.text(), "dog bat dog");
    }

    #[test]
    fn replace_all_with_capture_groups() {
        let mut buffer = Buffer::from_bytes(b"a=1, b=2, c=3").unwrap();
        let query = Query::new(r"(\w)=(\d)").regex(true);
        replace_all(&mut buffer, &query, "$2=$1").unwrap();
        assert_eq!(buffer.text(), "1=a, 2=b, 3=c");
    }

    #[test]
    fn replace_all_is_undoable_as_one_transaction() {
        let mut buffer = Buffer::from_bytes(b"cat bat cat").unwrap();
        let original = buffer.text();
        let query = Query::new("cat");
        let transaction = replace_all(&mut buffer, &query, "dog").unwrap();
        assert_eq!(buffer.text(), "dog bat dog");
        transaction.undo(&mut buffer);
        assert_eq!(buffer.text(), original);
    }

    #[test]
    fn replace_all_with_shrinking_replacement_keeps_offsets_valid() {
        let mut buffer = Buffer::from_bytes(b"aaa bbb aaa bbb aaa").unwrap();
        let query = Query::new("aaa");
        replace_all(&mut buffer, &query, "x").unwrap();
        assert_eq!(buffer.text(), "x bbb x bbb x");
    }

    #[test]
    fn replace_all_with_growing_replacement_keeps_offsets_valid() {
        let mut buffer = Buffer::from_bytes(b"a b a b a").unwrap();
        let query = Query::new("a");
        replace_all(&mut buffer, &query, "xyz").unwrap();
        assert_eq!(buffer.text(), "xyz b xyz b xyz");
    }

    #[test]
    fn replace_all_empty_pattern_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"hello").unwrap();
        let query = Query::new("");
        let transaction = replace_all(&mut buffer, &query, "x").unwrap();
        assert_eq!(buffer.text(), "hello");
        assert!(transaction.edits.is_empty());
    }

    #[test]
    fn replace_matches_replaces_subset_of_matches() {
        let mut buffer = Buffer::from_bytes(b"one two one two one").unwrap();
        let query = Query::new("one");
        let all_matches = find_all(&buffer, &query).unwrap();
        assert_eq!(all_matches.len(), 3);
        // Replace only the first and third matches
        let subset = vec![all_matches[0], all_matches[2]];
        let tx = replace_matches(&mut buffer, &query, &subset, "ONE").unwrap();
        assert_eq!(buffer.text(), "ONE two one two ONE");
        tx.undo(&mut buffer);
        assert_eq!(buffer.text(), "one two one two one");
    }
}
