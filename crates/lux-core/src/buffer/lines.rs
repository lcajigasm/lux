//! Indexed line access without materializing the rest of the document.

use super::Buffer;

impl Buffer {
    /// Copies one line's content, excluding its line terminator. Uses the
    /// rope's line index, including CR, CRLF and Unicode line boundaries.
    /// Returns `None` for an out-of-range line; the final empty line is valid.
    pub fn line_content(&self, line: usize) -> Option<String> {
        if line >= self.len_lines() {
            return None;
        }
        let mut text = self.rope.line(line).to_string();
        let content_len = text
            .trim_end_matches([
                '\r', '\n', '\u{000b}', '\u{000c}', '\u{0085}', '\u{2028}', '\u{2029}',
            ])
            .len();
        text.truncate(content_len);
        Some(text)
    }

    /// Leading spaces or tabs, then the remaining characters, excluding the
    /// line break. Does not allocate the line.
    pub fn line_columns(&self, line: usize) -> Option<(u32, u32)> {
        if line >= self.len_lines() {
            return None;
        }
        let mut indent = 0u32;
        let mut content = 0u32;
        let mut body = false;
        for ch in self.rope.line(line).chars() {
            if matches!(
                ch,
                '\n' | '\r' | '\u{000b}' | '\u{000c}' | '\u{0085}' | '\u{2028}' | '\u{2029}'
            ) {
                break;
            }
            if !body && (ch == ' ' || ch == '\t') {
                indent = indent.saturating_add(1);
            } else {
                body = true;
                content = content.saturating_add(1);
            }
        }
        Some((indent, content))
    }

    /// Byte offset of the first character of a line, in O(log N) time.
    pub fn line_start_byte(&self, line: usize) -> Option<usize> {
        (line < self.len_lines()).then(|| self.rope.line_to_byte(line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_lines_preserve_utf8_offsets_and_all_rope_terminators() {
        for ending in [
            "\n", "\r\n", "\r", "\u{000b}", "\u{000c}", "\u{0085}", "\u{2028}", "\u{2029}",
        ] {
            let text = format!("é{ending}猫{ending}");
            let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
            assert_eq!(buffer.len_lines(), 3);
            assert_eq!(buffer.line_content(0).as_deref(), Some("é"));
            assert_eq!(buffer.line_content(1).as_deref(), Some("猫"));
            assert_eq!(buffer.line_content(2).as_deref(), Some(""));
            assert_eq!(buffer.line_start_byte(1), Some("é".len() + ending.len()));
            assert_eq!(buffer.line_start_byte(2), Some(text.len()));
            assert_eq!(buffer.line_content(3), None);
            assert_eq!(buffer.line_start_byte(3), None);
        }
    }

    #[test]
    fn indexed_lines_follow_edits_without_a_separate_cache() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo").unwrap();
        buffer.insert(4, "new\n");
        assert_eq!(buffer.line_content(2).as_deref(), Some("two"));
        assert_eq!(buffer.line_start_byte(2), Some(8));
        buffer.delete(0..8);
        assert_eq!(buffer.line_content(0).as_deref(), Some("two"));
        assert_eq!(buffer.line_content(1), None);
    }
}
