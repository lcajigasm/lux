//! Cursor positions, expressed in three units that must never be silently
//! conflated: a byte offset (what slicing/saving needs), a char offset
//! (Unicode scalar values — what `ropey`'s char-indexed API works in), and a
//! visual column (grapheme clusters from line start — what movement and
//! rendering care about). Moving "one column" always steps one grapheme
//! cluster, so it never splits an emoji or a base character from its
//! combining accents.

use crate::buffer::Buffer;
use ropey::Rope;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete, UnicodeSegmentation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub byte: usize,
    pub char: usize,
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn document_start() -> Position {
        Position {
            byte: 0,
            char: 0,
            line: 0,
            column: 0,
        }
    }

    pub fn document_end(buffer: &Buffer) -> Position {
        Position::from_char(buffer, buffer.rope().len_chars())
    }

    /// Builds a `Position` from an absolute char offset into `buffer`,
    /// clamped to the document's length.
    pub fn from_char(buffer: &Buffer, char_idx: usize) -> Position {
        let rope = buffer.rope();
        let char_idx = char_idx.min(rope.len_chars());
        let byte = rope.char_to_byte(char_idx);
        let line = rope.char_to_line(char_idx);
        let line_start_char = rope.line_to_char(line);
        let column = grapheme_len(&rope.slice(line_start_char..char_idx).to_string());
        Position {
            byte,
            char: char_idx,
            line,
            column,
        }
    }

    /// Builds a `Position` from an absolute byte offset into `buffer`,
    /// clamped to the document's length. The offset does not need to already
    /// land on a char boundary.
    pub fn from_byte(buffer: &Buffer, byte_idx: usize) -> Position {
        let rope = buffer.rope();
        let byte_idx = byte_idx.min(rope.len_bytes());
        Self::from_char(buffer, rope.byte_to_char(byte_idx))
    }

    /// The position one grapheme cluster forward, clamped to the document
    /// end. Never lands inside a multi-codepoint grapheme cluster.
    pub fn next_grapheme(self, buffer: &Buffer) -> Position {
        let rope = buffer.rope();
        match next_grapheme_boundary(rope, self.byte) {
            Some(byte) => Position::from_byte(buffer, byte),
            None => self,
        }
    }

    /// The position one grapheme cluster back, clamped to the document
    /// start. Never lands inside a multi-codepoint grapheme cluster.
    pub fn prev_grapheme(self, buffer: &Buffer) -> Position {
        let rope = buffer.rope();
        match prev_grapheme_boundary(rope, self.byte) {
            Some(byte) => Position::from_byte(buffer, byte),
            None => self,
        }
    }
}

fn grapheme_len(text: &str) -> usize {
    text.graphemes(true).count()
}

/// Finds the next grapheme-cluster boundary after `byte_idx`, feeding the
/// cursor chunks on demand instead of materializing the whole document —
/// `ropey`'s documented pattern for pairing `GraphemeCursor` with a rope.
/// Returns `None` if `byte_idx` is already at (or past) the document end.
fn next_grapheme_boundary(rope: &Rope, byte_idx: usize) -> Option<usize> {
    let len = rope.len_bytes();
    if byte_idx >= len {
        return None;
    }
    let mut cursor = GraphemeCursor::new(byte_idx, len, true);
    let (mut chunk, mut chunk_start, _, _) = rope.chunk_at_byte(byte_idx);
    loop {
        match cursor.next_boundary(chunk, chunk_start) {
            Ok(boundary) => return boundary,
            Err(GraphemeIncomplete::NextChunk) => {
                let next_start = chunk_start + chunk.len();
                let (c, idx, _, _) = rope.chunk_at_byte(next_start.min(len.saturating_sub(1)));
                chunk = c;
                chunk_start = idx;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let (c, idx, _, _) = rope.chunk_at_byte(n.saturating_sub(1));
                cursor.provide_context(c, idx);
            }
            Err(e) => unreachable!("unexpected grapheme cursor error: {e:?}"),
        }
    }
}

/// Finds the previous grapheme-cluster boundary before `byte_idx`, using the
/// same chunk-feeding protocol as `next_grapheme_boundary`. Returns `None`
/// if `byte_idx` is already at the document start.
fn prev_grapheme_boundary(rope: &Rope, byte_idx: usize) -> Option<usize> {
    if byte_idx == 0 {
        return None;
    }
    let len = rope.len_bytes();
    let mut cursor = GraphemeCursor::new(byte_idx, len, true);
    let (mut chunk, mut chunk_start, _, _) =
        rope.chunk_at_byte(byte_idx.saturating_sub(1).min(len.saturating_sub(1)));
    loop {
        match cursor.prev_boundary(chunk, chunk_start) {
            Ok(boundary) => return boundary,
            Err(GraphemeIncomplete::PrevChunk) => {
                let prev_start = chunk_start.saturating_sub(1);
                let (c, idx, _, _) = rope.chunk_at_byte(prev_start);
                chunk = c;
                chunk_start = idx;
            }
            Err(GraphemeIncomplete::PreContext(n)) => {
                let (c, idx, _, _) = rope.chunk_at_byte(n.saturating_sub(1));
                cursor.provide_context(c, idx);
            }
            Err(e) => unreachable!("unexpected grapheme cursor error: {e:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_char_tracks_byte_char_line_and_column() {
        let buffer = Buffer::from_bytes("ab\ncd".as_bytes()).unwrap();
        let pos = Position::from_char(&buffer, 4); // the 'd' in "cd"
        assert_eq!(pos.char, 4);
        assert_eq!(pos.byte, 4);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 1);
    }

    #[test]
    fn distinguishes_byte_and_char_index_for_multibyte_text() {
        // "café" — 'é' is 2 bytes (U+00E9) but 1 char.
        let buffer = Buffer::from_bytes("café".as_bytes()).unwrap();
        let end = Position::from_char(&buffer, 4);
        assert_eq!(end.char, 4);
        assert_eq!(end.byte, 5);
        assert_eq!(end.column, 4);
    }

    #[test]
    fn from_byte_and_from_char_agree() {
        let buffer = Buffer::from_bytes("café".as_bytes()).unwrap();
        let by_char = Position::from_char(&buffer, 3); // before 'é'
        let by_byte = Position::from_byte(&buffer, 3);
        assert_eq!(by_char, by_byte);
    }

    #[test]
    fn next_grapheme_steps_over_combining_accent_as_one_unit() {
        // "e" + U+0301 (combining acute accent) is one grapheme cluster,
        // two chars, three bytes.
        let text = "e\u{0301}x";
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let start = Position::document_start();
        let after_first = start.next_grapheme(&buffer);
        assert_eq!(after_first.byte, 3); // past both codepoints of "é"
        assert_eq!(after_first.char, 2);
        assert_eq!(after_first.column, 1); // one visual column, not two
    }

    #[test]
    fn next_grapheme_steps_over_multi_codepoint_emoji_as_one_unit() {
        // Family emoji: four codepoints joined by ZWJ, one grapheme cluster.
        let emoji = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let text = format!("{emoji}x");
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let start = Position::document_start();
        let after_first = start.next_grapheme(&buffer);
        assert_eq!(after_first.byte, emoji.len());
        assert_eq!(after_first.column, 1);
    }

    #[test]
    fn prev_grapheme_is_the_inverse_of_next_grapheme() {
        let text = "e\u{0301}x";
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let start = Position::document_start();
        let after_first = start.next_grapheme(&buffer);
        let back = after_first.prev_grapheme(&buffer);
        assert_eq!(back, start);
    }

    #[test]
    fn next_grapheme_at_document_end_stays_put() {
        let buffer = Buffer::from_bytes(b"ab").unwrap();
        let end = Position::document_end(&buffer);
        assert_eq!(end.next_grapheme(&buffer), end);
    }

    #[test]
    fn prev_grapheme_at_document_start_stays_put() {
        let buffer = Buffer::from_bytes(b"ab").unwrap();
        let start = Position::document_start();
        assert_eq!(start.prev_grapheme(&buffer), start);
    }

    #[test]
    fn stepping_across_a_long_document_never_lands_mid_cluster() {
        // A long-ish document made of repeated grapheme clusters, to exercise
        // the chunked boundary search across `ropey`'s internal chunk edges
        // (not just a single-chunk toy string).
        let cluster = "e\u{0301}"; // "é" as base + combining accent
        let text: String = cluster.repeat(5000);
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();

        let mut pos = Position::document_start();
        let mut steps = 0;
        loop {
            let next = pos.next_grapheme(&buffer);
            if next == pos {
                break;
            }
            assert_eq!(
                next.byte - pos.byte,
                cluster.len(),
                "stepped by a partial cluster at byte {}",
                pos.byte
            );
            pos = next;
            steps += 1;
        }
        assert_eq!(steps, 5000);
    }
}
