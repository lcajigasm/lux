//! Page movement. `lux-core` has no notion of a viewport (font size,
//! window height, line wrapping are all UI/rendering concerns it must
//! stay ignorant of — see `CLAUDE.md` rule 2), so `lines_per_page` is a
//! parameter the caller supplies, not something computed here.

use super::line::position_at_line_column;
use crate::buffer::Buffer;
use crate::position::Position;

/// Moves up `lines_per_page` lines (at least one), landing at
/// `goal_column` like `movement::line::move_line_up` does. Clamps to the
/// document start rather than going negative.
pub fn move_page_up(
    buffer: &Buffer,
    pos: Position,
    lines_per_page: usize,
    goal_column: usize,
) -> Position {
    let target_line = pos.line.saturating_sub(lines_per_page.max(1));
    position_at_line_column(buffer, target_line, goal_column)
}

/// The mirror of `move_page_up`, clamped to the document's last line.
pub fn move_page_down(
    buffer: &Buffer,
    pos: Position,
    lines_per_page: usize,
    goal_column: usize,
) -> Position {
    let max_line = buffer.rope().len_lines().saturating_sub(1);
    let target_line = (pos.line + lines_per_page.max(1)).min(max_line);
    position_at_line_column(buffer, target_line, goal_column)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    fn doc_with_lines(n: usize) -> Buffer {
        let text: String = (0..n).map(|i| format!("line{i}\n")).collect();
        Buffer::from_bytes(text.as_bytes()).unwrap()
    }

    #[test]
    fn page_down_moves_forward_by_the_given_line_count() {
        let buffer = doc_with_lines(50);
        let pos = pos_at(&buffer, 0);
        let down = move_page_down(&buffer, pos, 20, 0);
        assert_eq!(down.line, 20);
    }

    #[test]
    fn page_down_clamps_to_the_last_line() {
        let buffer = doc_with_lines(10);
        let pos = pos_at(&buffer, 0);
        let down = move_page_down(&buffer, pos, 100, 0);
        assert_eq!(down.line, buffer.len_lines() - 1);
    }

    #[test]
    fn page_up_moves_backward_by_the_given_line_count() {
        let buffer = doc_with_lines(50);
        let pos = Position::from_char(&buffer, buffer.rope().line_to_char(40));
        let up = move_page_up(&buffer, pos, 20, 0);
        assert_eq!(up.line, 20);
    }

    #[test]
    fn page_up_clamps_to_the_first_line() {
        let buffer = doc_with_lines(10);
        let pos = Position::from_char(&buffer, buffer.rope().line_to_char(3));
        let up = move_page_up(&buffer, pos, 100, 0);
        assert_eq!(up.line, 0);
    }

    #[test]
    fn page_movement_preserves_the_goal_column() {
        let buffer = doc_with_lines(50); // each line is "lineN\n", 5-6 chars
        let pos = pos_at(&buffer, 3);
        let down = move_page_down(&buffer, pos, 5, 3);
        assert_eq!(down.column, 3);
    }
}
