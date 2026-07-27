//! Line movement (up/down, preserving a caller-tracked goal column) and
//! smart line start/end.

use crate::buffer::Buffer;
use crate::edit_ops::{line_range, split_terminator};
use crate::position::Position;

/// The position on `line` at grapheme-`column`, clamped to that line's own
/// content if it's shorter — the same approach
/// `selection_ops::box_select` uses for the same reason (grapheme-cluster
/// columns, not byte/char counts), reimplemented here rather than shared
/// since the two live in unrelated deliverables.
pub(crate) fn position_at_line_column(buffer: &Buffer, line: usize, column: usize) -> Position {
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

/// Moves up one visual line, landing at `goal_column` (clamped to the
/// line's length) rather than `pos`'s own column — `goal_column` is
/// whatever column the caller has been tracking across a run of
/// consecutive up/down presses (typically `pos.column` on the first
/// press), so moving through a short line and back doesn't lose the
/// original horizontal position. A no-op on the first line.
pub fn move_line_up(buffer: &Buffer, pos: Position, goal_column: usize) -> Position {
    if pos.line == 0 {
        return pos;
    }
    position_at_line_column(buffer, pos.line - 1, goal_column)
}

/// The mirror of `move_line_up`. A no-op on the last line.
pub fn move_line_down(buffer: &Buffer, pos: Position, goal_column: usize) -> Position {
    let max_line = buffer.rope().len_lines().saturating_sub(1);
    if pos.line >= max_line {
        return pos;
    }
    position_at_line_column(buffer, pos.line + 1, goal_column)
}

/// Smart line-start (`Home`): the first press goes to the first
/// non-whitespace column; pressing it again from there goes to column 0.
/// A blank line (no non-whitespace content) always goes straight to
/// column 0.
pub fn line_start(buffer: &Buffer, pos: Position) -> Position {
    let range = line_range(buffer.rope(), pos.line);
    let text = buffer.text_range(range.clone());
    let leading_ws = text.chars().take_while(|c| *c == ' ' || *c == '\t').count();
    let first_nonws = range.start + leading_ws;
    let target = if pos.char == first_nonws {
        range.start
    } else {
        first_nonws
    };
    Position::from_char(buffer, target)
}

/// End of line content — before the line's own terminator, never after it.
pub fn line_end(buffer: &Buffer, pos: Position) -> Position {
    let range = line_range(buffer.rope(), pos.line);
    let text = buffer.text_range(range.clone());
    let (content, _) = split_terminator(&text);
    Position::from_char(buffer, range.start + content.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    #[test]
    fn move_line_up_preserves_the_goal_column() {
        let buffer = Buffer::from_bytes(b"abcdef\nabcdef\n").unwrap();
        let pos = pos_at(&buffer, 11); // line 1, column 4
        let up = move_line_up(&buffer, pos, 4);
        assert_eq!(up.line, 0);
        assert_eq!(up.column, 4);
    }

    #[test]
    fn move_line_up_clamps_goal_column_to_a_shorter_line() {
        let buffer = Buffer::from_bytes(b"ab\nabcdef\n").unwrap();
        let pos = pos_at(&buffer, 8); // line 1, column 5
        let up = move_line_up(&buffer, pos, 5);
        assert_eq!(up.line, 0);
        assert_eq!(up.column, 2); // clamped to "ab"'s length
    }

    #[test]
    fn move_line_up_on_the_first_line_is_a_noop() {
        let buffer = Buffer::from_bytes(b"abc\ndef\n").unwrap();
        let pos = pos_at(&buffer, 1);
        let up = move_line_up(&buffer, pos, 1);
        assert_eq!(up, pos);
    }

    #[test]
    fn move_line_down_preserves_the_goal_column() {
        let buffer = Buffer::from_bytes(b"abcdef\nabcdef\n").unwrap();
        let pos = pos_at(&buffer, 2); // line 0, column 2
        let down = move_line_down(&buffer, pos, 2);
        assert_eq!(down.line, 1);
        assert_eq!(down.column, 2);
    }

    #[test]
    fn move_line_down_on_the_last_line_is_a_noop() {
        let buffer = Buffer::from_bytes(b"abc\ndef").unwrap();
        let pos = pos_at(&buffer, 5);
        let down = move_line_down(&buffer, pos, 1);
        assert_eq!(down, pos);
    }

    #[test]
    fn line_start_goes_to_first_non_whitespace_then_to_column_zero() {
        let buffer = Buffer::from_bytes(b"    let x = 1;\n").unwrap();
        let pos = pos_at(&buffer, 10); // somewhere in "let x = 1;"
        let first = line_start(&buffer, pos);
        assert_eq!(first.char, 4); // start of "let"
        let second = line_start(&buffer, first);
        assert_eq!(second.char, 0);
        let third = line_start(&buffer, second);
        assert_eq!(third.char, 4); // toggles back
    }

    #[test]
    fn line_start_on_a_blank_line_goes_to_column_zero() {
        let buffer = Buffer::from_bytes(b"\n").unwrap();
        let pos = pos_at(&buffer, 0);
        assert_eq!(line_start(&buffer, pos).char, 0);
    }

    #[test]
    fn line_end_stops_before_the_terminator() {
        let buffer = Buffer::from_bytes(b"hello\nworld\n").unwrap();
        let pos = pos_at(&buffer, 2);
        assert_eq!(line_end(&buffer, pos).char, 5);
    }

    #[test]
    fn line_end_on_the_last_line_without_a_terminator() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        let pos = pos_at(&buffer, 0);
        assert_eq!(line_end(&buffer, pos).char, 5);
    }
}
