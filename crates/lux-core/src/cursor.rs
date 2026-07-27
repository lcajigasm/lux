//! Normalization and overlap-merging for a set of cursors. A "cursor" here
//! is just a [`Selection`](crate::selection::Selection) — reused as-is, not
//! duplicated as a new type — so this module is the algorithm that
//! [`MultiCursor`](crate::multicursor::MultiCursor) is built on: given N
//! selections in any order, produce the sorted, non-overlapping set that
//! represents them.

use crate::selection::Selection;

/// Sorts `cursors` by document position (earlier start first, then earlier
/// end), without merging anything.
pub fn normalize(mut cursors: Vec<Selection>) -> Vec<Selection> {
    cursors.sort_by_key(|c| {
        let (start, end) = c.ordered();
        (start.char, end.char)
    });
    cursors
}

/// True when `a` and `b` should collapse into one cursor: they share
/// selected text (`Selection::overlaps`), they're collapsed cursors at the
/// identical position, or one is a collapsed cursor landing inside the
/// other's range. Two selections that merely touch at an edge — one ends
/// exactly where the other starts — do *not* merge, matching
/// `Selection::overlaps`.
fn should_merge(a: &Selection, b: &Selection) -> bool {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => a.head.char == b.head.char,
        (true, false) => b.char_range().contains(&a.head.char),
        (false, true) => a.char_range().contains(&b.head.char),
        (false, false) => a.overlaps(b),
    }
}

/// Merges two cursors known to overlap into one spanning both. The result
/// always reads forward (anchor before head) — merging discards whichever
/// individual direction either input selection had.
fn merge_pair(a: &Selection, b: &Selection) -> Selection {
    let (a_start, a_end) = a.ordered();
    let (b_start, b_end) = b.ordered();
    let start = if a_start.char <= b_start.char {
        a_start
    } else {
        b_start
    };
    let end = if a_end.char >= b_end.char {
        a_end
    } else {
        b_end
    };
    Selection::new(start, end)
}

/// Normalizes `cursors` and merges any that overlap, coincide, or contain
/// one another, so the result never has two cursors covering the same text
/// and never has duplicate collapsed cursors at the same position. This is
/// the invariant [`MultiCursor`](crate::multicursor::MultiCursor) maintains
/// after every change to its cursor set.
pub fn merge_overlapping(cursors: Vec<Selection>) -> Vec<Selection> {
    let sorted = normalize(cursors);
    let mut merged: Vec<Selection> = Vec::with_capacity(sorted.len());
    for cursor in sorted {
        match merged.last_mut() {
            Some(last) if should_merge(last, &cursor) => *last = merge_pair(last, &cursor),
            _ => merged.push(cursor),
        }
    }
    merged
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
    fn normalize_sorts_by_position() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![
            cursor_at(&buffer, 7),
            cursor_at(&buffer, 1),
            cursor_at(&buffer, 4),
        ];
        let sorted = normalize(cursors);
        let positions: Vec<usize> = sorted.iter().map(|c| c.head.char).collect();
        assert_eq!(positions, vec![1, 4, 7]);
    }

    #[test]
    fn non_overlapping_cursors_are_left_alone() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![cursor_at(&buffer, 1), cursor_at(&buffer, 8)];
        let merged = merge_overlapping(cursors);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn touching_but_not_overlapping_selections_stay_separate() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![range_at(&buffer, 0, 5), range_at(&buffer, 5, 9)];
        let merged = merge_overlapping(cursors);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn overlapping_ranges_merge_into_their_union() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![range_at(&buffer, 0, 5), range_at(&buffer, 3, 8)];
        let merged = merge_overlapping(cursors);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].char_range(), 0..8);
        assert!(!merged[0].is_reversed());
    }

    #[test]
    fn duplicate_collapsed_cursors_merge_into_one() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![
            cursor_at(&buffer, 4),
            cursor_at(&buffer, 4),
            cursor_at(&buffer, 4),
        ];
        let merged = merge_overlapping(cursors);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].head.char, 4);
    }

    #[test]
    fn collapsed_cursor_inside_a_range_is_absorbed() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![range_at(&buffer, 2, 8), cursor_at(&buffer, 5)];
        let merged = merge_overlapping(cursors);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].char_range(), 2..8);
    }

    #[test]
    fn collapsed_cursor_at_a_ranges_edge_is_not_absorbed() {
        // The range is a half-open [2, 8); a cursor sitting exactly at 8 is
        // just past it, not inside it.
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![range_at(&buffer, 2, 8), cursor_at(&buffer, 8)];
        let merged = merge_overlapping(cursors);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn reversed_selections_merge_by_position_not_direction() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let a = Selection::new(pos_at(&buffer, 5), pos_at(&buffer, 0)); // reversed
        let b = Selection::new(pos_at(&buffer, 3), pos_at(&buffer, 8));
        let merged = merge_overlapping(vec![a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].char_range(), 0..8);
    }

    #[test]
    fn transitive_chain_of_overlaps_merges_into_a_single_cursor() {
        // A overlaps B, B overlaps C, but A and C alone don't touch — the
        // merge must still chain all three together via B.
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let a = range_at(&buffer, 0, 3);
        let b = range_at(&buffer, 2, 6);
        let c = range_at(&buffer, 5, 9);
        let merged = merge_overlapping(vec![c, a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].char_range(), 0..9);
    }

    #[test]
    fn unrelated_cursors_survive_alongside_a_merge() {
        let buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let cursors = vec![
            cursor_at(&buffer, 9),
            range_at(&buffer, 0, 3),
            range_at(&buffer, 2, 5),
        ];
        let merged = merge_overlapping(cursors);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].char_range(), 0..5);
        assert_eq!(merged[1].head.char, 9);
    }
}
