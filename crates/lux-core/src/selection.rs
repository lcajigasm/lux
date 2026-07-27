//! A single selection: an anchor (where the selection started) and a head
//! (where it currently ends, and where the cursor blinks). Multicursor — N
//! of these applied atomically — is a separate deliverable; this is the
//! one-selection building block it's built on.

use crate::position::Position;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub head: Position,
}

impl Selection {
    /// A collapsed selection (no text selected) at `pos`.
    pub fn cursor(pos: Position) -> Selection {
        Selection {
            anchor: pos,
            head: pos,
        }
    }

    pub fn new(anchor: Position, head: Position) -> Selection {
        Selection { anchor, head }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor.char == self.head.char
    }

    /// True when the head is before the anchor (the user selected backward).
    pub fn is_reversed(&self) -> bool {
        self.head.char < self.anchor.char
    }

    /// The char range covered, ordered low..high regardless of direction.
    pub fn char_range(&self) -> Range<usize> {
        let (start, end) = self.ordered();
        start.char..end.char
    }

    /// The byte range covered, ordered low..high regardless of direction.
    pub fn byte_range(&self) -> Range<usize> {
        let (start, end) = self.ordered();
        start.byte..end.byte
    }

    /// `(start, end)` with `start` always the earlier position in the
    /// document, independent of which end is the anchor and which the head.
    pub fn ordered(&self) -> (Position, Position) {
        if self.is_reversed() {
            (self.head, self.anchor)
        } else {
            (self.anchor, self.head)
        }
    }

    /// Collapses the selection to its head, discarding the anchor — what
    /// pressing an arrow key does to a non-empty selection.
    pub fn collapse_to_head(&self) -> Selection {
        Selection::cursor(self.head)
    }

    /// True when `self` and `other` cover any of the same char range. Two
    /// empty (cursor) selections never overlap, even at the same position —
    /// overlap is about shared selected text, not coincident cursors.
    pub fn overlaps(&self, other: &Selection) -> bool {
        if self.is_empty() || other.is_empty() {
            return false;
        }
        let a = self.char_range();
        let b = other.char_range();
        a.start < b.end && b.start < a.end
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
    fn cursor_is_empty() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        let sel = Selection::cursor(pos_at(&buffer, 2));
        assert!(sel.is_empty());
        assert_eq!(sel.anchor, sel.head);
    }

    #[test]
    fn forward_selection_is_not_reversed_and_ranges_correctly() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = Selection::new(pos_at(&buffer, 2), pos_at(&buffer, 7));
        assert!(!sel.is_reversed());
        assert!(!sel.is_empty());
        assert_eq!(sel.char_range(), 2..7);
        assert_eq!(sel.byte_range(), 2..7);
    }

    #[test]
    fn backward_selection_is_reversed_but_ranges_stay_ordered() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = Selection::new(pos_at(&buffer, 7), pos_at(&buffer, 2));
        assert!(sel.is_reversed());
        assert_eq!(sel.char_range(), 2..7);
        let (start, end) = sel.ordered();
        assert_eq!(start.char, 2);
        assert_eq!(end.char, 7);
    }

    #[test]
    fn collapse_to_head_drops_the_anchor() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = Selection::new(pos_at(&buffer, 2), pos_at(&buffer, 7));
        let collapsed = sel.collapse_to_head();
        assert!(collapsed.is_empty());
        assert_eq!(collapsed.head.char, 7);
    }

    #[test]
    fn overlaps_detects_shared_range_regardless_of_direction() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let a = Selection::new(pos_at(&buffer, 0), pos_at(&buffer, 5));
        let b = Selection::new(pos_at(&buffer, 8), pos_at(&buffer, 3)); // reversed
        let c = Selection::new(pos_at(&buffer, 5), pos_at(&buffer, 9));
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c)); // touching at 5, not overlapping
    }

    #[test]
    fn adjacent_cursors_never_overlap() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let a = Selection::cursor(pos_at(&buffer, 3));
        let b = Selection::cursor(pos_at(&buffer, 3));
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn selection_never_splits_a_grapheme_cluster_when_extending() {
        // Extending a selection one grapheme at a time (as Shift+Right would)
        // must land the head on a cluster boundary, same guarantee as
        // `Position::next_grapheme` alone.
        let text = "e\u{0301}x"; // "é" (base + combining accent) + "x"
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let anchor = Position::document_start();
        let head = anchor.next_grapheme(&buffer);
        let sel = Selection::new(anchor, head);
        assert_eq!(sel.char_range(), 0..2); // both codepoints of "é"
        assert_eq!(sel.byte_range(), 0..3);
    }
}
