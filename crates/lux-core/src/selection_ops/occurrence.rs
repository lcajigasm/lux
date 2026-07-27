//! `Cmd+D`-style multi-cursor: select the next occurrence of the current
//! selection's text, select every occurrence at once, or skip the current
//! occurrence for the next one. Literal substring search only — regex and
//! case-insensitive search belong to the separate find/replace deliverable;
//! this only needs "find this exact text again."

use super::word::expand_to_word;
use crate::buffer::Buffer;
use crate::multicursor::MultiCursor;
use crate::position::Position;
use crate::selection::Selection;

/// The text to search for from `selection`: its own text if non-empty,
/// otherwise the word touching a collapsed cursor. `None` if there's
/// nothing meaningful to search for (empty document, or an empty selection
/// that doesn't expand to anything).
fn search_text(buffer: &Buffer, selection: Selection) -> Option<(String, Selection)> {
    if !selection.is_empty() {
        return Some((buffer.text_range(selection.char_range()), selection));
    }
    let expanded = expand_to_word(buffer, selection);
    if expanded.is_empty() {
        return None;
    }
    Some((buffer.text_range(expanded.char_range()), expanded))
}

/// Every non-overlapping occurrence of `needle` in the buffer, in document
/// order.
fn find_occurrences(buffer: &Buffer, needle: &str) -> Vec<Selection> {
    if needle.is_empty() {
        return Vec::new();
    }
    let text = buffer.text();
    text.match_indices(needle)
        .map(|(byte_idx, matched)| {
            let start = Position::from_byte(buffer, byte_idx);
            let end = Position::from_byte(buffer, byte_idx + matched.len());
            Selection::new(start, end)
        })
        .collect()
}

/// Adds the next occurrence of the primary cursor's text (after its own
/// end, wrapping around to the document start if none follows) as a new
/// cursor. If the primary was a collapsed cursor, it's expanded to the word
/// it touches first — the classic `Cmd+D` behavior. A no-op if there's no
/// text to search for or no occurrence exists anywhere.
pub fn select_next_occurrence(buffer: &Buffer, multicursor: &MultiCursor) -> MultiCursor {
    let primary = multicursor.primary();
    let Some((needle, primary_selection)) = search_text(buffer, primary) else {
        return multicursor.clone();
    };
    let occurrences = find_occurrences(buffer, &needle);
    let Some(next) = next_occurrence_after(&occurrences, primary_selection) else {
        return multicursor.clone();
    };

    let mut cursors = multicursor.cursors().to_vec();
    if primary.is_empty() {
        // Replace the collapsed cursor with its word-expansion so it's a
        // real occurrence too, not just a point.
        if let Some(pos) = cursors.iter().position(|&c| c == primary) {
            cursors[pos] = primary_selection;
        }
    }
    cursors.push(next);
    MultiCursor::from_cursors(cursors)
}

/// Replaces the primary cursor with the next occurrence of its text instead
/// of adding one — the same cursor count, just moved past the current
/// match. A no-op under the same conditions as `select_next_occurrence`.
pub fn skip_current_occurrence(buffer: &Buffer, multicursor: &MultiCursor) -> MultiCursor {
    let primary = multicursor.primary();
    let Some((needle, primary_selection)) = search_text(buffer, primary) else {
        return multicursor.clone();
    };
    let occurrences = find_occurrences(buffer, &needle);
    let Some(next) = next_occurrence_after(&occurrences, primary_selection) else {
        return multicursor.clone();
    };

    let mut cursors: Vec<Selection> = multicursor
        .cursors()
        .iter()
        .filter(|&&c| c != primary)
        .copied()
        .collect();
    cursors.push(next);
    MultiCursor::from_cursors(cursors)
}

/// Replaces the entire cursor set with every occurrence of the primary
/// cursor's text. A no-op under the same conditions as
/// `select_next_occurrence`.
pub fn select_all_occurrences(buffer: &Buffer, multicursor: &MultiCursor) -> MultiCursor {
    let primary = multicursor.primary();
    let Some((needle, _)) = search_text(buffer, primary) else {
        return multicursor.clone();
    };
    let occurrences = find_occurrences(buffer, &needle);
    if occurrences.is_empty() {
        return multicursor.clone();
    }
    MultiCursor::from_cursors(occurrences)
}

fn next_occurrence_after(occurrences: &[Selection], after: Selection) -> Option<Selection> {
    if occurrences.is_empty() {
        return None;
    }
    occurrences
        .iter()
        .find(|occ| occ.char_range().start >= after.char_range().end)
        .or_else(|| occurrences.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor_at(buffer: &Buffer, char_idx: usize) -> Selection {
        Selection::cursor(Position::from_char(buffer, char_idx))
    }

    fn range_at(buffer: &Buffer, from: usize, to: usize) -> Selection {
        Selection::new(
            Position::from_char(buffer, from),
            Position::from_char(buffer, to),
        )
    }

    #[test]
    fn select_next_occurrence_expands_a_collapsed_cursor_and_adds_the_next_match() {
        let buffer = Buffer::from_bytes(b"foo bar foo baz foo").unwrap();
        let mc = MultiCursor::new(cursor_at(&buffer, 1)); // inside the first "foo"
        let next = select_next_occurrence(&buffer, &mc);
        assert_eq!(next.count(), 2);
        let ranges: Vec<_> = next.cursors().iter().map(|c| c.char_range()).collect();
        assert_eq!(ranges, vec![0..3, 8..11]);
    }

    #[test]
    fn select_next_occurrence_wraps_around_to_the_start() {
        let buffer = Buffer::from_bytes(b"foo bar foo").unwrap();
        let mc = MultiCursor::new(range_at(&buffer, 8, 11)); // the last "foo"
        let next = select_next_occurrence(&buffer, &mc);
        assert_eq!(next.count(), 2);
        let ranges: Vec<_> = next.cursors().iter().map(|c| c.char_range()).collect();
        assert_eq!(ranges, vec![0..3, 8..11]);
    }

    #[test]
    fn select_next_occurrence_is_a_noop_with_no_other_match() {
        let buffer = Buffer::from_bytes(b"only one foo here").unwrap();
        let mc = MultiCursor::new(range_at(&buffer, 9, 12)); // "foo"
        let next = select_next_occurrence(&buffer, &mc);
        assert_eq!(next.count(), 1);
    }

    #[test]
    fn skip_current_occurrence_moves_without_growing_the_cursor_count() {
        let buffer = Buffer::from_bytes(b"foo bar foo baz foo").unwrap();
        let mc = MultiCursor::new(range_at(&buffer, 0, 3)); // the first "foo"
        let skipped = skip_current_occurrence(&buffer, &mc);
        assert_eq!(skipped.count(), 1);
        assert_eq!(skipped.primary().char_range(), 8..11);
    }

    #[test]
    fn select_all_occurrences_replaces_the_cursor_set() {
        let buffer = Buffer::from_bytes(b"foo bar foo baz foo").unwrap();
        let mc = MultiCursor::new(cursor_at(&buffer, 1));
        let all = select_all_occurrences(&buffer, &mc);
        assert_eq!(all.count(), 3);
        let ranges: Vec<_> = all.cursors().iter().map(|c| c.char_range()).collect();
        assert_eq!(ranges, vec![0..3, 8..11, 16..19]);
    }

    #[test]
    fn select_all_occurrences_on_a_word_under_the_cursor() {
        let buffer = Buffer::from_bytes(b"cat dog cat").unwrap();
        let mc = MultiCursor::new(cursor_at(&buffer, 0));
        let all = select_all_occurrences(&buffer, &mc);
        assert_eq!(all.count(), 2);
    }
}
