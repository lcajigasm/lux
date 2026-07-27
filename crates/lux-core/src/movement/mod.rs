//! Cursor movement: character, word (with subword rules), line, paragraph,
//! page, smart line start/end, document start/end — each with or without
//! extending a selection; plus bracket matching and jump-to-match.
//!
//! Every movement function computes a destination `Position` from a
//! `Buffer` and a starting `Position`; none of them touch `Selection` or
//! `MultiCursor` directly. `extend_or_move` is the one place that decides
//! whether a destination becomes a new collapsed cursor or extends the
//! current selection (`Shift+<key>` vs. plain `<key>`), so no individual
//! movement has to implement that choice itself.

mod bracket;
mod line;
mod page;
mod paragraph;
mod word;

pub use bracket::matching_bracket;
pub use line::{line_end, line_start, move_line_down, move_line_up};
pub use page::{move_page_down, move_page_up};
pub use paragraph::{move_paragraph_next, move_paragraph_prev};
pub use word::{move_word_left, move_word_right};

use crate::buffer::Buffer;
use crate::position::Position;
use crate::selection::Selection;

/// One grapheme cluster left — a thin wrapper around
/// `Position::prev_grapheme` (already cluster-aware, from the positions
/// deliverable) so every movement is reachable from this one module.
pub fn move_char_left(buffer: &Buffer, pos: Position) -> Position {
    pos.prev_grapheme(buffer)
}

/// One grapheme cluster right.
pub fn move_char_right(buffer: &Buffer, pos: Position) -> Position {
    pos.next_grapheme(buffer)
}

pub fn document_start() -> Position {
    Position::document_start()
}

pub fn document_end(buffer: &Buffer) -> Position {
    Position::document_end(buffer)
}

/// Turns a movement's destination into the resulting selection: with
/// `extend`, the current selection's anchor stays put and its head moves
/// to `to` (`Shift+<key>`); without it, the selection collapses to a
/// cursor at `to` (plain `<key>`).
pub fn extend_or_move(current: Selection, to: Position, extend: bool) -> Selection {
    if extend {
        Selection::new(current.anchor, to)
    } else {
        Selection::cursor(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    #[test]
    fn move_char_right_steps_one_grapheme_cluster() {
        let buffer = Buffer::from_bytes("e\u{0301}x".as_bytes()).unwrap(); // "é" + "x"
        let next = move_char_right(&buffer, Position::document_start());
        assert_eq!(next.char, 2); // past both codepoints of "é"
    }

    #[test]
    fn move_char_left_steps_one_grapheme_cluster() {
        let buffer = Buffer::from_bytes("e\u{0301}x".as_bytes()).unwrap();
        let end = Position::document_end(&buffer);
        let back = move_char_left(&buffer, end);
        assert_eq!(back.char, 2); // right after "é", before "x"
    }

    #[test]
    fn document_start_and_end_are_the_documents_boundaries() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        assert_eq!(document_start().char, 0);
        assert_eq!(document_end(&buffer).char, 5);
    }

    #[test]
    fn extend_or_move_without_extend_collapses_to_a_cursor() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let current = Selection::new(pos_at(&buffer, 2), pos_at(&buffer, 5));
        let result = extend_or_move(current, pos_at(&buffer, 8), false);
        assert!(result.is_empty());
        assert_eq!(result.head.char, 8);
    }

    #[test]
    fn extend_or_move_with_extend_keeps_the_anchor() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let current = Selection::new(pos_at(&buffer, 2), pos_at(&buffer, 5));
        let result = extend_or_move(current, pos_at(&buffer, 8), true);
        assert_eq!(result.anchor.char, 2);
        assert_eq!(result.head.char, 8);
    }

    #[test]
    fn extend_or_move_from_a_collapsed_cursor_starts_a_new_selection() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let current = Selection::cursor(pos_at(&buffer, 3));
        let result = extend_or_move(current, pos_at(&buffer, 7), true);
        assert_eq!(result.anchor.char, 3);
        assert_eq!(result.head.char, 7);
    }
}
