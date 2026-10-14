//! Column (box) selection: drag a rectangle and get one cursor per line it
//! spans, each at the corresponding column.

use crate::buffer::Buffer;
use crate::multicursor::MultiCursor;
use crate::position::Position;
use crate::selection::Selection;

/// The position on `line` at grapheme-`column`, clamped to that line's own
/// content if it's shorter than `column` (never crossing into the next
/// line). Reuses `Position::next_grapheme` — the same cluster-aware
/// stepping used everywhere else — instead of a byte/char count, so a wide
/// grapheme cluster still counts as one column.
pub fn position_at_column(buffer: &Buffer, line: usize, column: usize) -> Position {
    let rope = buffer.rope();
    if line >= rope.len_lines() {
        return Position::document_end(buffer);
    }
    let mut pos = Position::from_char(buffer, rope.line_to_char(line));
    for _ in 0..column {
        let next = pos.next_grapheme(buffer);
        if next == pos || next.line != line {
            break;
        }
        pos = next;
    }
    pos
}

/// One cursor per line between `anchor` and `head` (inclusive), each
/// positioned at `anchor`'s column through `head`'s column — a single
/// collapsed cursor per line if the two columns are equal, otherwise a
/// same-shaped selection on every line.
pub fn box_select(buffer: &Buffer, anchor: Position, head: Position) -> MultiCursor {
    let (first_line, last_line) = if anchor.line <= head.line {
        (anchor.line, head.line)
    } else {
        (head.line, anchor.line)
    };
    let (from_column, to_column) = if anchor.column <= head.column {
        (anchor.column, head.column)
    } else {
        (head.column, anchor.column)
    };

    let cursors: Vec<Selection> = (first_line..=last_line)
        .map(|line| {
            let from = position_at_column(buffer, line, from_column);
            let to = position_at_column(buffer, line, to_column);
            Selection::new(from, to)
        })
        .collect();
    MultiCursor::from_cursors(cursors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    #[test]
    fn box_select_places_one_cursor_per_line_at_the_same_column() {
        let buffer = Buffer::from_bytes(b"abcd\nefgh\nijkl\n").unwrap();
        let anchor = pos(&buffer, 1); // line 0, column 1
        let head = pos(&buffer, 11); // line 2, column 1
        let mc = box_select(&buffer, anchor, head);
        assert_eq!(mc.count(), 3);
        let columns: Vec<usize> = mc.cursors().iter().map(|c| c.head.column).collect();
        assert_eq!(columns, vec![1, 1, 1]);
        let lines: Vec<usize> = mc.cursors().iter().map(|c| c.head.line).collect();
        assert_eq!(lines, vec![0, 1, 2]);
    }

    #[test]
    fn box_select_clamps_to_shorter_lines() {
        let buffer = Buffer::from_bytes(b"abcdef\nab\nabcdef\n").unwrap();
        let anchor = pos(&buffer, 4); // line 0, column 4
        let head = Position::from_char(&buffer, 7 + 2); // line 1 is "ab" (2 chars), way past its end
        let mc = box_select(&buffer, anchor, head);
        // Middle line ("ab") should clamp to its own length, not panic or
        // run into the next line.
        assert_eq!(mc.cursors()[1].head.line, 1);
        assert_eq!(mc.cursors()[1].head.column, 2);
    }

    #[test]
    fn box_select_with_different_columns_makes_a_selection_per_line() {
        let buffer = Buffer::from_bytes(b"abcdef\nghijkl\n").unwrap();
        let anchor = pos(&buffer, 1); // line 0, column 1
        let head = Position::from_char(&buffer, 7 + 4); // line 1, column 4
        let mc = box_select(&buffer, anchor, head);
        assert_eq!(mc.count(), 2);
        assert!(!mc.cursors()[0].is_empty());
        assert_eq!(mc.cursors()[0].char_range(), 1..4);
    }

    #[test]
    fn box_select_normalizes_a_reversed_drag() {
        let buffer = Buffer::from_bytes(b"abcd\nefgh\n").unwrap();
        // Dragging from bottom-right to top-left should give the same
        // result as top-left to bottom-right.
        let anchor = Position::from_char(&buffer, 5 + 3); // line 1, column 3
        let head = pos(&buffer, 0); // line 0, column 0
        let mc = box_select(&buffer, anchor, head);
        assert_eq!(mc.count(), 2);
        assert_eq!(mc.cursors()[0].head.line, 0);
        assert_eq!(mc.cursors()[1].head.line, 1);
    }
}
