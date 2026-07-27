//! Expand-selection-to-word: the "double-click a word" operation, and the
//! seed `select_next_occurrence`/`select_all_occurrences` use to turn a
//! collapsed cursor into search text. Deliberately not a reuse of
//! `edit_ops::word`'s boundary scanner — that one *skips leading
//! whitespace first* (right for "delete the previous word"), whereas this
//! one has to pick which class of run the cursor is actually touching (so
//! double-clicking whitespace selects the whitespace run, not a
//! neighboring word) — different enough to not force through one shared
//! shape.

use crate::buffer::Buffer;
use crate::position::Position;
use crate::selection::Selection;

#[derive(PartialEq, Eq, Clone, Copy)]
enum CharClass {
    Word,
    Space,
    Other,
}

fn classify(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Space
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Other
    }
}

/// The class of run touching char offset `idx`: prefers the char to the
/// right if it's a word char (the common "cursor just before/inside a
/// word" case), else falls back to the char on whichever side exists.
fn class_touching(buffer: &Buffer, idx: usize) -> Option<CharClass> {
    let rope = buffer.rope();
    if idx < rope.len_chars() {
        let right = classify(rope.char(idx));
        if right == CharClass::Word {
            return Some(right);
        }
    }
    if idx > 0 {
        return Some(classify(rope.char(idx - 1)));
    }
    if idx < rope.len_chars() {
        return Some(classify(rope.char(idx)));
    }
    None
}

fn expand_left_through_class(buffer: &Buffer, idx: usize, class: CharClass) -> usize {
    let mut idx = idx;
    let mut cursor = buffer.rope().chars_at(idx);
    while let Some(c) = cursor.prev() {
        if classify(c) != class {
            break;
        }
        idx -= 1;
    }
    idx
}

fn expand_right_through_class(buffer: &Buffer, idx: usize, class: CharClass) -> usize {
    let mut idx = idx;
    let cursor = buffer.rope().chars_at(idx);
    for c in cursor {
        if classify(c) != class {
            break;
        }
        idx += 1;
    }
    idx
}

/// Expands `selection` to cover the full "word" (or whitespace run, or
/// punctuation run) it touches: a collapsed cursor selects the run it sits
/// in or right after; a non-empty selection extends each edge outward to
/// its run boundary. A no-op on an empty document.
pub fn expand_to_word(buffer: &Buffer, selection: Selection) -> Selection {
    let (start, end) = selection.ordered();
    let Some(class) = class_touching(buffer, start.char) else {
        return selection;
    };
    let new_start = expand_left_through_class(buffer, start.char, class);
    let new_end = expand_right_through_class(buffer, end.char, class);
    Selection::new(
        Position::from_char(buffer, new_start),
        Position::from_char(buffer, new_end),
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
    fn expands_a_cursor_inside_a_word() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = expand_to_word(&buffer, cursor_at(&buffer, 2)); // inside "hello"
        assert_eq!(sel.char_range(), 0..5);
    }

    #[test]
    fn expands_a_cursor_right_after_a_word() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = expand_to_word(&buffer, cursor_at(&buffer, 5)); // right after "hello"
        assert_eq!(sel.char_range(), 0..5);
    }

    #[test]
    fn expands_a_cursor_on_whitespace_to_the_whitespace_run() {
        let buffer = Buffer::from_bytes(b"a   b").unwrap();
        let sel = expand_to_word(&buffer, cursor_at(&buffer, 2)); // middle of the spaces
        assert_eq!(sel.char_range(), 1..4);
    }

    #[test]
    fn expands_a_cursor_on_punctuation_to_the_punctuation_run() {
        let buffer = Buffer::from_bytes(b"foo() + bar").unwrap();
        let sel = expand_to_word(&buffer, cursor_at(&buffer, 4)); // on "()"
        assert_eq!(sel.char_range(), 3..5);
    }

    #[test]
    fn expands_an_existing_selection_outward_to_word_boundaries() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = expand_to_word(&buffer, range_at(&buffer, 2, 8)); // "llo wo"
        assert_eq!(sel.char_range(), 0..11);
    }

    #[test]
    fn is_a_noop_on_an_empty_document() {
        let buffer = Buffer::from_bytes(b"").unwrap();
        let sel = expand_to_word(&buffer, cursor_at(&buffer, 0));
        assert!(sel.is_empty());
        assert_eq!(sel.head.char, 0);
    }
}
