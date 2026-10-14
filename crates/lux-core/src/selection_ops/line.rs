//! Expand selection to line (`Cmd+L` / `Ctrl+L`).
//!
//! If cursor is collapsed or doesn't span full lines: selects the current line
//! including its trailing newline.
//! If selection already spans one or more whole lines: extends downward by one
//! additional line per invocation.

use crate::buffer::Buffer;
use crate::position::Position;
use crate::selection::Selection;

/// Expands `selection` to cover full lines, or extends downward if already full lines.
pub fn expand_to_line(buffer: &Buffer, selection: Selection) -> Selection {
    let rope = buffer.rope();
    let total_lines = rope.len_lines();
    if total_lines == 0 {
        return selection;
    }

    let (start, end) = selection.ordered();
    let start_line = start.line;
    let end_line = end.line;

    let line_start_char = rope.line_to_char(start_line);
    let line_end_char = if end_line + 1 < total_lines {
        rope.line_to_char(end_line + 1)
    } else {
        rope.len_chars()
    };

    let is_whole_line = !selection.is_empty()
        && start.char == line_start_char
        && (end.char == line_end_char
            || (end_line < total_lines && end.char == rope.line_to_char(end_line)));

    if is_whole_line && end.char < rope.len_chars() {
        let target_line = if end.char == line_end_char {
            (end_line + 2).min(total_lines)
        } else {
            (end_line + 1).min(total_lines)
        };
        let new_end_char = if target_line < total_lines {
            rope.line_to_char(target_line)
        } else {
            rope.len_chars()
        };
        Selection::new(
            Position::from_char(buffer, line_start_char),
            Position::from_char(buffer, new_end_char),
        )
    } else {
        Selection::new(
            Position::from_char(buffer, line_start_char),
            Position::from_char(buffer, line_end_char),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buf: &Buffer, c: usize) -> Position {
        Position::from_char(buf, c)
    }

    #[test]
    fn collapsed_cursor_expands_to_single_line() {
        let buf = Buffer::from_bytes(b"hello\nworld\nfoo").unwrap();
        let sel = Selection::cursor(pos_at(&buf, 2)); // in "hello"
        let expanded = expand_to_line(&buf, sel);
        assert_eq!(expanded.char_range(), 0..6); // "hello\n"
        assert_eq!(buf.text_range(expanded.char_range()), "hello\n");
    }

    #[test]
    fn sequential_expand_to_line_extends_downward() {
        let buf = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        let sel = Selection::cursor(pos_at(&buf, 1));

        let sel1 = expand_to_line(&buf, sel);
        assert_eq!(sel1.char_range(), 0..4); // "one\n"

        let sel2 = expand_to_line(&buf, sel1);
        assert_eq!(sel2.char_range(), 0..8); // "one\ntwo\n"

        let sel3 = expand_to_line(&buf, sel2);
        assert_eq!(sel3.char_range(), 0..14); // "one\ntwo\nthree\n"

        // Clamps at doc end
        let sel4 = expand_to_line(&buf, sel3);
        assert_eq!(sel4.char_range(), 0..14);
    }

    #[test]
    fn last_line_without_trailing_newline() {
        let buf = Buffer::from_bytes(b"alpha\nbeta").unwrap();
        let sel = Selection::cursor(pos_at(&buf, 7)); // in "beta"
        let expanded = expand_to_line(&buf, sel);
        assert_eq!(expanded.char_range(), 6..10); // "beta"
    }
}
