//! Whitespace cleanup operations: trim trailing whitespace and ensure newline at EOF.

use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::edit_ops::{line_range, split_terminator};

/// Trims trailing whitespace (ASCII spaces `' '` and tabs `'\t'`) from lines in `buffer`.
///
/// If `preserve_cursor_lines` is specified, any line index contained within it will be
/// skipped, leaving its trailing whitespace intact.
///
/// Edits are applied in reverse document order (from bottom of buffer to top) so that
/// character offsets for earlier lines remain stable as edits are applied.
pub fn trim_trailing_whitespace(
    buffer: &mut Buffer,
    preserve_cursor_lines: Option<&[usize]>,
) -> Vec<Edit> {
    let num_lines = buffer.len_lines();
    if num_lines == 0 {
        return Vec::new();
    }

    let mut edits = Vec::new();
    for line_idx in (0..num_lines).rev() {
        if let Some(preserve) = preserve_cursor_lines
            && preserve.contains(&line_idx)
        {
            continue;
        }

        let range = line_range(buffer.rope(), line_idx);
        let line_text = buffer.text_range(range.clone());
        let (content, _term) = split_terminator(&line_text);
        let trimmed_content = content.trim_end_matches([' ', '\t']);
        let trimmed_count = trimmed_content.chars().count();
        let content_count = content.chars().count();

        if trimmed_count < content_count {
            let delete_start = range.start + trimmed_count;
            let delete_len = content_count - trimmed_count;
            let edit = Edit::delete(buffer, delete_start, delete_len);
            edit.apply(buffer);
            edits.push(edit);
        }
    }

    edits
}

/// Ensures that a non-empty `buffer` ends with a newline terminator.
///
/// If the buffer is empty or already ends with `\n`, no edits are made.
/// Otherwise, `buffer.eol().as_str()` is inserted at the end of the document.
pub fn ensure_final_newline(buffer: &mut Buffer) -> Vec<Edit> {
    if buffer.len_chars() == 0 {
        return Vec::new();
    }

    let rope = buffer.rope();
    let len = rope.len_chars();
    if len > 0 && rope.char(len - 1) == '\n' {
        return Vec::new();
    }

    let eol = buffer.eol().as_str();
    let edit = Edit::insert(len, eol.to_string());
    edit.apply(buffer);
    vec![edit]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Eol;

    #[test]
    fn test_trim_trailing_whitespace_basic() {
        let mut buffer = Buffer::from_bytes(b"hello   \nworld\t\t\nclean\n").unwrap();
        let edits = trim_trailing_whitespace(&mut buffer, None);
        assert_eq!(edits.len(), 2);
        assert_eq!(buffer.text(), "hello\nworld\nclean\n");
    }

    #[test]
    fn test_trim_trailing_whitespace_preserve_cursor() {
        let mut buffer = Buffer::from_bytes(b"line 0   \nline 1   \nline 2   \n").unwrap();
        let edits = trim_trailing_whitespace(&mut buffer, Some(&[1]));
        assert_eq!(edits.len(), 2);
        assert_eq!(buffer.text(), "line 0\nline 1   \nline 2\n");
    }

    #[test]
    fn test_trim_trailing_whitespace_empty_lines_with_spaces() {
        let mut buffer = Buffer::from_bytes(b"   \n\t\t\ntext\n").unwrap();
        let edits = trim_trailing_whitespace(&mut buffer, None);
        assert_eq!(edits.len(), 2);
        assert_eq!(buffer.text(), "\n\ntext\n");
    }

    #[test]
    fn test_trim_trailing_whitespace_last_line_without_newline() {
        let mut buffer = Buffer::from_bytes(b"first\nlast   ").unwrap();
        let edits = trim_trailing_whitespace(&mut buffer, None);
        assert_eq!(edits.len(), 1);
        assert_eq!(buffer.text(), "first\nlast");
    }

    #[test]
    fn test_trim_trailing_whitespace_no_whitespace_noop() {
        let mut buffer = Buffer::from_bytes(b"clean\nlines\n").unwrap();
        let edits = trim_trailing_whitespace(&mut buffer, None);
        assert!(edits.is_empty());
        assert_eq!(buffer.text(), "clean\nlines\n");
    }

    #[test]
    fn test_ensure_final_newline_adds_newline() {
        let mut buffer = Buffer::from_bytes(b"hello").unwrap();
        let edits = ensure_final_newline(&mut buffer);
        assert_eq!(edits.len(), 1);
        assert_eq!(buffer.text(), "hello\n");
    }

    #[test]
    fn test_ensure_final_newline_crlf() {
        let mut buffer = Buffer::from_bytes(b"hello\r\nworld").unwrap();
        assert_eq!(buffer.eol(), Eol::Crlf);
        let edits = ensure_final_newline(&mut buffer);
        assert_eq!(edits.len(), 1);
        assert_eq!(buffer.text(), "hello\r\nworld\r\n");
    }

    #[test]
    fn test_ensure_final_newline_already_present_noop() {
        let mut buffer = Buffer::from_bytes(b"hello\n").unwrap();
        let edits = ensure_final_newline(&mut buffer);
        assert!(edits.is_empty());
        assert_eq!(buffer.text(), "hello\n");
    }

    #[test]
    fn test_ensure_final_newline_empty_buffer_noop() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let edits = ensure_final_newline(&mut buffer);
        assert!(edits.is_empty());
        assert_eq!(buffer.text(), "");
    }
}
