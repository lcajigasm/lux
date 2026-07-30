//! Byte-range boundaries for each line in a text snapshot.
//!
//! `lux-core`'s `Buffer` has no public per-line accessor yet (only whole-doc
//! `text()` and a char-range `text_range()` — see this deliverable's
//! receipt for the full finding); this is the smallest thing that turns a
//! whole-document string into "give me line N's bytes" without touching
//! `lux-core`, which is out of this deliverable's scope.

use std::ops::Range;

/// Splits `text` into line byte-ranges the same way `ropey`/
/// `Buffer::len_lines` counts them: a trailing newline (or an empty
/// document) always yields one extra, empty final line, so `ranges().len()`
/// always matches `Buffer::len_lines()` for the same snapshot.
pub(crate) struct LineIndex {
    ranges: Vec<Range<usize>>,
}

impl LineIndex {
    pub(crate) fn build(text: &str) -> Self {
        let mut ranges = Vec::new();
        let mut start = 0;
        for (i, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                ranges.push(start..i);
                start = i + 1;
            }
        }
        ranges.push(start..text.len());
        Self { ranges }
    }

    /// The byte range of line `line_idx`'s content, excluding its `\n` (and
    /// a trailing `\r`, for CRLF documents — full EOL-aware rendering is
    /// `lux-core`'s `Eol` job, not this view's, but leaving a bare `\r` in
    /// the text handed to shaping would render a visible stray glyph for
    /// every CRLF line, so it's stripped here defensively).
    pub(crate) fn line_byte_range(&self, text: &str, line_idx: usize) -> Range<usize> {
        let range = self.ranges.get(line_idx).cloned().unwrap_or(0..0);
        match text[range.clone()].strip_suffix('\r') {
            Some(stripped) => range.start..range.start + stripped.len(),
            None => range,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.ranges.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_buffers_len_lines_convention_for_a_trailing_newline() {
        let text = "a\nb\n";
        let index = LineIndex::build(text);
        // ropey counts a trailing "\n" as introducing one more, empty line.
        assert_eq!(index.len(), 3);
        assert_eq!(&text[index.line_byte_range(text, 0)], "a");
        assert_eq!(&text[index.line_byte_range(text, 1)], "b");
        assert_eq!(&text[index.line_byte_range(text, 2)], "");
    }

    #[test]
    fn empty_document_is_one_empty_line() {
        let index = LineIndex::build("");
        assert_eq!(index.len(), 1);
        assert_eq!(&""[index.line_byte_range("", 0)], "");
    }

    #[test]
    fn no_trailing_newline_still_yields_the_last_line() {
        let text = "one\ntwo";
        let index = LineIndex::build(text);
        assert_eq!(index.len(), 2);
        assert_eq!(&text[index.line_byte_range(text, 1)], "two");
    }

    #[test]
    fn strips_a_trailing_carriage_return_from_crlf_lines() {
        let text = "one\r\ntwo\r\n";
        let index = LineIndex::build(text);
        assert_eq!(&text[index.line_byte_range(text, 0)], "one");
        assert_eq!(&text[index.line_byte_range(text, 1)], "two");
    }

    #[test]
    fn out_of_range_line_returns_an_empty_range() {
        let index = LineIndex::build("only one line");
        assert_eq!(index.line_byte_range("only one line", 5), 0..0);
    }
}
