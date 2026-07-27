//! Paragraph movement: jumps to the next/previous blank-line boundary — a
//! "paragraph" is a run of non-blank lines, same notion `vim`'s `}`/`{`
//! and Emacs's `M-}`/`M-{` use.

use crate::buffer::Buffer;
use crate::edit_ops::{line_range, split_terminator};
use crate::position::Position;

fn is_blank_line(buffer: &Buffer, line: usize) -> bool {
    let range = line_range(buffer.rope(), line);
    let text = buffer.text_range(range);
    let (content, _) = split_terminator(&text);
    content.trim().is_empty()
}

/// Moves forward to the start of the next blank line (skipping past any
/// blank lines right at the cursor first, so repeated presses keep making
/// progress instead of stalling on a blank run), or the document end if
/// there isn't one.
pub fn move_paragraph_next(buffer: &Buffer, pos: Position) -> Position {
    let rope = buffer.rope();
    let total_lines = rope.len_lines();
    let mut line = pos.line;

    while line < total_lines && is_blank_line(buffer, line) {
        line += 1;
    }
    while line < total_lines && !is_blank_line(buffer, line) {
        line += 1;
    }
    if line >= total_lines {
        return Position::document_end(buffer);
    }
    Position::from_char(buffer, rope.line_to_char(line))
}

/// The mirror of `move_paragraph_next`: backward to the start of the
/// nearest blank line above (skipping a blank run immediately above the
/// cursor first, then the current paragraph's content), or the document
/// start.
pub fn move_paragraph_prev(buffer: &Buffer, pos: Position) -> Position {
    let rope = buffer.rope();
    if pos.line == 0 {
        return Position::document_start();
    }
    let mut line = pos.line - 1;

    while line > 0 && is_blank_line(buffer, line) {
        line -= 1;
    }
    while line > 0 && !is_blank_line(buffer, line) {
        line -= 1;
    }
    Position::from_char(buffer, rope.line_to_char(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    #[test]
    fn next_paragraph_stops_at_the_blank_line() {
        let buffer = Buffer::from_bytes(b"a\nb\n\nc\nd\n").unwrap();
        let pos = pos_at(&buffer, 0); // line 0, "a"
        let next = move_paragraph_next(&buffer, pos);
        assert_eq!(next.line, 2); // the blank line
    }

    #[test]
    fn next_paragraph_from_inside_a_blank_run_skips_it_first() {
        let buffer = Buffer::from_bytes(b"a\n\n\nb\nc\n\nd\n").unwrap();
        let pos = pos_at(&buffer, 2); // on the first blank line
        let next = move_paragraph_next(&buffer, pos);
        // Skips the blank run, then the "b\nc\n" paragraph, landing on the
        // next blank line.
        assert_eq!(next.line, 5);
    }

    #[test]
    fn next_paragraph_with_no_more_blank_lines_goes_to_document_end() {
        let buffer = Buffer::from_bytes(b"a\nb\nc").unwrap();
        let pos = pos_at(&buffer, 0);
        let next = move_paragraph_next(&buffer, pos);
        assert_eq!(next, Position::document_end(&buffer));
    }

    #[test]
    fn prev_paragraph_stops_at_the_blank_line_above() {
        let buffer = Buffer::from_bytes(b"a\n\nb\nc\n").unwrap();
        let pos = pos_at(&buffer, 5); // line 3, "c"
        let prev = move_paragraph_prev(&buffer, pos);
        assert_eq!(prev.line, 1); // the blank line
    }

    #[test]
    fn prev_paragraph_at_the_first_line_goes_to_document_start() {
        let buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
        let pos = pos_at(&buffer, 0);
        let prev = move_paragraph_prev(&buffer, pos);
        assert_eq!(prev, Position::document_start());
    }
}
