//! Split a multi-line selection into one selection per line — reuses
//! `edit_ops`'s line-boundary helpers (phantom-trailing-line handling
//! included) rather than re-deriving them.

use crate::buffer::Buffer;
use crate::edit_ops::{line_range, split_terminator, touched_lines};
use crate::multicursor::MultiCursor;
use crate::position::Position;
use crate::selection::Selection;

/// One selection per line `selection` touches, each clamped to that line's
/// own content (never including its terminator).
pub fn split_into_lines(buffer: &Buffer, selection: Selection) -> MultiCursor {
    let (start, end) = selection.ordered();
    let lines = touched_lines(selection, buffer.rope());

    let cursors: Vec<Selection> = lines
        .map(|line| {
            let range = line_range(buffer.rope(), line);
            let text = buffer.text_range(range.clone());
            let (content, _) = split_terminator(&text);
            let content_end = range.start + content.chars().count();

            let seg_start = start.char.max(range.start).min(content_end);
            let seg_end = end.char.min(content_end).max(range.start);
            Selection::new(
                Position::from_char(buffer, seg_start),
                Position::from_char(buffer, seg_end),
            )
        })
        .collect();
    MultiCursor::from_cursors(cursors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range_at(buffer: &Buffer, from: usize, to: usize) -> Selection {
        Selection::new(
            Position::from_char(buffer, from),
            Position::from_char(buffer, to),
        )
    }

    #[test]
    fn splits_a_multiline_selection_one_per_line() {
        let buffer = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        let sel = range_at(&buffer, 1, 10); // spans lines 0 ("one"), 1 ("two"), 2 ("three")
        let mc = split_into_lines(&buffer, sel);
        assert_eq!(mc.count(), 3);
        let ranges: Vec<_> = mc.cursors().iter().map(|c| c.char_range()).collect();
        assert_eq!(ranges, vec![1..3, 4..7, 8..10]);
    }

    #[test]
    fn splits_never_include_the_line_terminator() {
        let buffer = Buffer::from_bytes(b"ab\ncd\n").unwrap();
        let sel = range_at(&buffer, 0, 6); // whole document, both terminators
        let mc = split_into_lines(&buffer, sel);
        let texts: Vec<String> = mc
            .cursors()
            .iter()
            .map(|c| buffer.text_range(c.char_range()))
            .collect();
        assert_eq!(texts, vec!["ab".to_string(), "cd".to_string()]);
    }

    #[test]
    fn a_single_line_selection_splits_into_itself() {
        let buffer = Buffer::from_bytes(b"hello world\n").unwrap();
        let sel = range_at(&buffer, 2, 8);
        let mc = split_into_lines(&buffer, sel);
        assert_eq!(mc.count(), 1);
        assert_eq!(mc.cursors()[0].char_range(), 2..8);
    }
}
