//! N cursors treated as one first-class unit: always sorted, never
//! overlapping (see [`cursor`](crate::cursor) for the merge algorithm), and
//! always tracking which one is "primary" — the cursor an operation like
//! "select next occurrence" acts on. Applying an editing operation to all N
//! cursors atomically is a separate deliverable; this is the cursor-set data
//! structure that one will drive.

use crate::cursor;
use crate::selection::Selection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiCursor {
    cursors: Vec<Selection>,
    /// The char offset of the primary cursor's head. Tracked as a position
    /// rather than a vector index so it survives merges, which can shift or
    /// shrink `cursors` — after a merge the primary is whichever surviving
    /// cursor's range now contains this offset.
    primary_head: usize,
}

impl MultiCursor {
    /// A multicursor with a single cursor, which is trivially primary.
    pub fn new(cursor: Selection) -> MultiCursor {
        MultiCursor {
            primary_head: cursor.head.char,
            cursors: vec![cursor],
        }
    }

    /// Builds a multicursor from N cursors in any order, normalizing and
    /// merging overlaps immediately. The last cursor in `cursors` is treated
    /// as the one that was "just added" and becomes primary.
    ///
    /// # Panics
    /// Panics if `cursors` is empty — a `MultiCursor` always has at least
    /// one cursor.
    pub fn from_cursors(cursors: Vec<Selection>) -> MultiCursor {
        let primary_head = cursors
            .last()
            .expect("MultiCursor must have at least one cursor")
            .head
            .char;
        MultiCursor {
            cursors: cursor::merge_overlapping(cursors),
            primary_head,
        }
    }

    /// All cursors, sorted by document position, none overlapping.
    pub fn cursors(&self) -> &[Selection] {
        &self.cursors
    }

    pub fn count(&self) -> usize {
        self.cursors.len()
    }

    /// The primary cursor: whichever cursor's range currently contains the
    /// head of the most recently added or moved cursor.
    pub fn primary(&self) -> Selection {
        self.cursors
            .iter()
            .copied()
            .find(|c| {
                let (start, end) = c.ordered();
                start.char <= self.primary_head && self.primary_head <= end.char
            })
            .unwrap_or_else(|| {
                *self
                    .cursors
                    .last()
                    .expect("MultiCursor must have at least one cursor")
            })
    }

    /// Adds `cursor`, then re-normalizes and merges overlaps. `cursor`
    /// becomes (or is absorbed into) the new primary.
    pub fn add(&mut self, cursor: Selection) {
        self.primary_head = cursor.head.char;
        let mut all = std::mem::take(&mut self.cursors);
        all.push(cursor);
        self.cursors = cursor::merge_overlapping(all);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::position::Position;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    fn cursor_at(buffer: &Buffer, char_idx: usize) -> Selection {
        Selection::cursor(pos_at(buffer, char_idx))
    }

    fn range_at(buffer: &Buffer, from: usize, to: usize) -> Selection {
        Selection::new(pos_at(buffer, from), pos_at(buffer, to))
    }

    #[test]
    fn new_creates_a_single_primary_cursor() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let mc = MultiCursor::new(cursor_at(&buffer, 3));
        assert_eq!(mc.count(), 1);
        assert_eq!(mc.primary().head.char, 3);
    }

    #[test]
    fn add_grows_the_cursor_set_and_keeps_it_sorted() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let mut mc = MultiCursor::new(cursor_at(&buffer, 7));
        mc.add(cursor_at(&buffer, 2));
        mc.add(cursor_at(&buffer, 5));
        assert_eq!(mc.count(), 3);
        let positions: Vec<usize> = mc.cursors().iter().map(|c| c.head.char).collect();
        assert_eq!(positions, vec![2, 5, 7]);
    }

    #[test]
    fn adding_a_cursor_at_an_existing_position_does_not_duplicate_it() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let mut mc = MultiCursor::new(cursor_at(&buffer, 4));
        mc.add(cursor_at(&buffer, 4));
        assert_eq!(mc.count(), 1);
    }

    #[test]
    fn adding_an_overlapping_range_merges_it_and_becomes_primary() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let mut mc = MultiCursor::new(range_at(&buffer, 0, 4));
        mc.add(range_at(&buffer, 2, 6));
        assert_eq!(mc.count(), 1);
        let primary = mc.primary();
        assert_eq!(primary.char_range(), 0..6);
    }

    #[test]
    fn primary_survives_a_merge_triggered_by_an_unrelated_add() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let mut mc = MultiCursor::new(cursor_at(&buffer, 9)); // primary at 9
        mc.add(range_at(&buffer, 0, 3)); // unrelated add, new primary
        mc.add(range_at(&buffer, 2, 5)); // merges with the range at 0..3
        assert_eq!(mc.count(), 2);
        assert_eq!(mc.primary().char_range(), 0..5);
    }

    #[test]
    fn from_cursors_normalizes_and_merges_unsorted_input() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let mc = MultiCursor::from_cursors(vec![
            cursor_at(&buffer, 9),
            range_at(&buffer, 0, 3),
            range_at(&buffer, 2, 5),
        ]);
        assert_eq!(mc.count(), 2);
        assert_eq!(mc.cursors()[0].char_range(), 0..5);
        assert_eq!(mc.cursors()[1].head.char, 9);
        // Last element of the input (the range 2..5) is what becomes primary.
        assert_eq!(mc.primary().char_range(), 0..5);
    }

    #[test]
    #[should_panic(expected = "at least one cursor")]
    fn from_cursors_rejects_an_empty_set() {
        MultiCursor::from_cursors(vec![]);
    }
}
