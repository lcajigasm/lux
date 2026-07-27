//! Transpose and case-change: text transforms that read a range and write
//! back a rearranged/recased version of it.

use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::position::Position;
use crate::selection::Selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseChange {
    Upper,
    Lower,
    /// Flips the case of each cased char independently.
    Toggle,
}

/// Swaps the grapheme cluster before the cursor with the one after it (the
/// classic Emacs `C-t` / Sublime "Transpose" command). A no-op if the
/// selection isn't empty, or if the cursor doesn't have a cluster on both
/// sides (start or end of document).
pub fn transpose(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    if !selection.is_empty() {
        return (Vec::new(), selection);
    }
    let cursor = selection.head;
    let prev = cursor.prev_grapheme(buffer);
    let next = cursor.next_grapheme(buffer);
    if prev == cursor || next == cursor {
        return (Vec::new(), selection);
    }

    let before = buffer.text_range(prev.char..cursor.char);
    let after = buffer.text_range(cursor.char..next.char);

    let delete_edit = Edit::delete(buffer, prev.char, next.char - prev.char);
    delete_edit.apply(buffer);
    let insert_edit = Edit::insert(prev.char, format!("{after}{before}"));
    insert_edit.apply(buffer);

    let new_pos = Position::from_char(buffer, next.char);
    (vec![delete_edit, insert_edit], Selection::cursor(new_pos))
}

/// Changes the case of the selected text. A no-op on an empty selection —
/// there's no well-defined single character to act on without a movement
/// model to extend it first.
pub fn change_case(
    buffer: &mut Buffer,
    selection: Selection,
    case: CaseChange,
) -> (Vec<Edit>, Selection) {
    if selection.is_empty() {
        return (Vec::new(), selection);
    }
    let range = selection.char_range();
    let text = buffer.text_range(range.clone());
    let changed: String = match case {
        CaseChange::Upper => text.to_uppercase(),
        CaseChange::Lower => text.to_lowercase(),
        CaseChange::Toggle => text
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().next().unwrap_or(c)
                } else if c.is_lowercase() {
                    c.to_uppercase().next().unwrap_or(c)
                } else {
                    c
                }
            })
            .collect(),
    };

    let delete_edit = Edit::delete(buffer, range.start, range.len());
    delete_edit.apply(buffer);
    let insert_edit = Edit::insert(range.start, changed.clone());
    insert_edit.apply(buffer);

    let new_anchor = Position::from_char(buffer, range.start);
    let new_head = Position::from_char(buffer, range.start + changed.chars().count());
    (
        vec![delete_edit, insert_edit],
        Selection::new(new_anchor, new_head),
    )
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
    fn transpose_swaps_the_two_chars_around_the_cursor() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let sel = cursor_at(&buffer, 1);
        let (_, sel) = transpose(&mut buffer, sel);
        assert_eq!(buffer.text(), "ba");
        assert_eq!(sel.head.char, 2);
    }

    #[test]
    fn transpose_swaps_whole_grapheme_clusters_not_chars() {
        // "e" + combining accent, then "x" — transpose at the cursor
        // between them must move the whole cluster, not split it.
        let text = "e\u{0301}x";
        let mut buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let cursor = Position::from_char(&buffer, 2); // right after "é"
        let (_, sel) = transpose(&mut buffer, Selection::cursor(cursor));
        assert_eq!(buffer.text(), "xe\u{0301}");
        assert_eq!(sel.head.char, 3);
    }

    #[test]
    fn transpose_at_document_start_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let sel = cursor_at(&buffer, 0);
        let (edits, _) = transpose(&mut buffer, sel);
        assert_eq!(buffer.text(), "ab");
        assert!(edits.is_empty());
    }

    #[test]
    fn transpose_at_document_end_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let sel = cursor_at(&buffer, 2);
        let (edits, _) = transpose(&mut buffer, sel);
        assert_eq!(buffer.text(), "ab");
        assert!(edits.is_empty());
    }

    #[test]
    fn transpose_on_a_selection_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"abcd").unwrap();
        let sel = range_at(&buffer, 1, 3);
        let (edits, _) = transpose(&mut buffer, sel);
        assert_eq!(buffer.text(), "abcd");
        assert!(edits.is_empty());
    }

    #[test]
    fn change_case_upper() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = range_at(&buffer, 0, 5);
        let (_, sel) = change_case(&mut buffer, sel, CaseChange::Upper);
        assert_eq!(buffer.text(), "HELLO world");
        assert_eq!(sel.char_range(), 0..5);
    }

    #[test]
    fn change_case_lower() {
        let mut buffer = Buffer::from_bytes(b"HELLO world").unwrap();
        let sel = range_at(&buffer, 0, 5);
        change_case(&mut buffer, sel, CaseChange::Lower);
        assert_eq!(buffer.text(), "hello world");
    }

    #[test]
    fn change_case_toggle() {
        let mut buffer = Buffer::from_bytes(b"Hello World").unwrap();
        let sel = range_at(&buffer, 0, 11);
        change_case(&mut buffer, sel, CaseChange::Toggle);
        assert_eq!(buffer.text(), "hELLO wORLD");
    }

    #[test]
    fn change_case_on_empty_selection_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"hello").unwrap();
        let sel = cursor_at(&buffer, 2);
        let (edits, _) = change_case(&mut buffer, sel, CaseChange::Upper);
        assert_eq!(buffer.text(), "hello");
        assert!(edits.is_empty());
    }

    #[test]
    fn change_case_handles_length_changing_uppercasing() {
        // German ß uppercases to "SS" — two chars from one.
        let mut buffer = Buffer::from_bytes("stra\u{df}e".as_bytes()).unwrap();
        let sel = range_at(&buffer, 0, 6);
        let (_, sel) = change_case(&mut buffer, sel, CaseChange::Upper);
        assert_eq!(buffer.text(), "STRASSE");
        assert_eq!(sel.char_range().end, 7);
    }
}
