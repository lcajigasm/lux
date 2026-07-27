//! Delete-word: a minimal three-class (word / space / punctuation) boundary
//! scanner, just enough to know where "the current word" starts or ends.
//! Full word-movement rules (subword splitting, Unicode word-break tables)
//! belong to the movement deliverable — this only needs *a* boundary,
//! consistently, to delete up to it.

use super::delete_selection;
use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::position::Position;
use crate::selection::Selection;

#[derive(PartialEq, Eq)]
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

/// The char offset one word-boundary before `char_idx`: skip any run of
/// whitespace immediately before it, then skip the run of same-class chars
/// before that. `ropey`'s `Chars` iterator has no `DoubleEndedIterator`
/// impl (it's a chunk-based cursor, not a slice), so this steps backward
/// with its inherent `prev()` method instead of `.rev()`.
fn word_start_before(buffer: &Buffer, char_idx: usize) -> usize {
    let mut idx = char_idx;
    let mut cursor = buffer.rope().chars_at(idx);

    let mut pending = cursor.prev();
    while let Some(c) = pending {
        if classify(c) != CharClass::Space {
            break;
        }
        idx -= 1;
        pending = cursor.prev();
    }
    if let Some(first) = pending {
        let class = classify(first);
        idx -= 1;
        while let Some(c) = cursor.prev() {
            if classify(c) != class {
                break;
            }
            idx -= 1;
        }
    }
    idx
}

/// The char offset one word-boundary after `char_idx`, symmetric to
/// `word_start_before`.
fn word_end_after(buffer: &Buffer, char_idx: usize) -> usize {
    let rope = buffer.rope();
    let mut idx = char_idx;
    let mut chars_after = rope.slice(idx..).chars().peekable();

    while let Some(&c) = chars_after.peek() {
        if classify(c) != CharClass::Space {
            break;
        }
        chars_after.next();
        idx += 1;
    }
    if let Some(&first) = chars_after.peek() {
        let class = classify(first);
        while let Some(&c) = chars_after.peek() {
            if classify(c) != class {
                break;
            }
            chars_after.next();
            idx += 1;
        }
    }
    idx
}

/// Deletes from `selection`'s head back to the start of the previous word
/// (like Ctrl+Backspace). If `selection` isn't empty, deletes the selection
/// instead — word-delete on an active selection just removes it, same as
/// plain backspace would.
pub fn delete_word_backward(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    if !selection.is_empty() {
        return delete_selection(buffer, selection);
    }
    let char_idx = selection.head.char;
    let start = word_start_before(buffer, char_idx);
    if start == char_idx {
        return (Vec::new(), selection);
    }
    let edit = Edit::delete(buffer, start, char_idx - start);
    edit.apply(buffer);
    (
        vec![edit],
        Selection::cursor(Position::from_char(buffer, start)),
    )
}

/// Deletes from `selection`'s head forward to the end of the next word
/// (like Ctrl+Delete). Same selection-collapsing behavior as
/// `delete_word_backward`.
pub fn delete_word_forward(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    if !selection.is_empty() {
        return delete_selection(buffer, selection);
    }
    let char_idx = selection.head.char;
    let end = word_end_after(buffer, char_idx);
    if end == char_idx {
        return (Vec::new(), selection);
    }
    let edit = Edit::delete(buffer, char_idx, end - char_idx);
    edit.apply(buffer);
    (
        vec![edit],
        Selection::cursor(Position::from_char(buffer, char_idx)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor_at(buffer: &Buffer, char_idx: usize) -> Selection {
        Selection::cursor(Position::from_char(buffer, char_idx))
    }

    #[test]
    fn delete_word_backward_removes_the_previous_word() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = cursor_at(&buffer, 11);
        let (edits, sel) = delete_word_backward(&mut buffer, sel);
        assert_eq!(buffer.text(), "hello ");
        assert_eq!(edits.len(), 1);
        assert_eq!(sel.head.char, 6);
    }

    #[test]
    fn delete_word_backward_skips_trailing_whitespace_first() {
        let mut buffer = Buffer::from_bytes(b"hello   world").unwrap();
        let sel = cursor_at(&buffer, 8);
        let (_, sel) = delete_word_backward(&mut buffer, sel); // just after the spaces
        assert_eq!(buffer.text(), "world");
        assert_eq!(sel.head.char, 0);
    }

    #[test]
    fn delete_word_backward_at_document_start_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"hello").unwrap();
        let sel = cursor_at(&buffer, 0);
        let (edits, sel) = delete_word_backward(&mut buffer, sel);
        assert_eq!(buffer.text(), "hello");
        assert!(edits.is_empty());
        assert_eq!(sel.head.char, 0);
    }

    #[test]
    fn delete_word_forward_removes_the_next_word() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = cursor_at(&buffer, 0);
        let (edits, sel) = delete_word_forward(&mut buffer, sel);
        assert_eq!(buffer.text(), " world");
        assert_eq!(edits.len(), 1);
        assert_eq!(sel.head.char, 0);
    }

    #[test]
    fn delete_word_forward_stops_at_punctuation_boundary() {
        let mut buffer = Buffer::from_bytes(b"foo(bar)").unwrap();
        let sel = cursor_at(&buffer, 0);
        let (_, sel) = delete_word_forward(&mut buffer, sel);
        assert_eq!(buffer.text(), "(bar)");
        assert_eq!(sel.head.char, 0);
    }

    #[test]
    fn word_delete_with_active_selection_just_deletes_it() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = Selection::new(
            Position::from_char(&buffer, 0),
            Position::from_char(&buffer, 5),
        );
        let (edits, result) = delete_word_backward(&mut buffer, sel);
        assert_eq!(buffer.text(), " world");
        assert_eq!(edits.len(), 1);
        assert_eq!(result.head.char, 0);
    }
}
