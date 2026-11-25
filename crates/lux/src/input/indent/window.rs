//! The slice of a buffer an indent decision parses.
//!
//! The tree the highlighter maintains lives on its worker thread and can be
//! one edit behind, so Enter, a typed closer, and Reindent cannot read it
//! synchronously. Copying and reparsing the whole buffer instead costs time
//! linear in the file on every keystroke. A window is the run of lines from
//! the nearest column-0 line above the edit to the nearest column-0 line
//! below it: the top-level item the edit sits in. That is all the context an
//! indent level depends on, and it stays small however large the file is.
//! Enter only needs the level of the new line relative to the line it
//! splits, so a window that starts at a misindented column-0 line still
//! gives the right difference.

use lux_core::buffer::Buffer;

/// How far a window reaches past the edited lines before it stops looking
/// for a column-0 line. Bounds the copy and the parse on deeply nested or
/// unindented files.
pub(crate) const MAX_CONTEXT_LINES: usize = 2_000;

/// A run of whole lines copied out of the rope.
pub(crate) struct Window {
    /// Buffer line of the window's first row.
    pub(crate) first_line: usize,
    /// Buffer char offset of the window's first char.
    pub(crate) start_char: usize,
    pub(crate) text: String,
}

impl Window {
    /// The window around buffer lines `first..=last`.
    pub(crate) fn around(buffer: &Buffer, first: usize, last: usize) -> Self {
        let rope = buffer.rope();
        let lines = rope.len_lines();
        let last = last.min(lines.saturating_sub(1));
        let first = first.min(last);
        let floor = first.saturating_sub(MAX_CONTEXT_LINES);
        let mut top = first;
        while top > floor && !opens_top_level(rope.line(top).chars()) {
            top -= 1;
        }
        let ceiling = (last + MAX_CONTEXT_LINES).min(lines.saturating_sub(1));
        let mut bottom = last;
        while bottom < ceiling {
            bottom += 1;
            if starts_at_column_zero(rope.line(bottom).chars().next()) {
                break;
            }
        }
        let start_char = rope.line_to_char(top);
        let end_char = if bottom + 1 < lines {
            rope.line_to_char(bottom + 1)
        } else {
            rope.len_chars()
        };
        Self {
            first_line: top,
            start_char,
            text: rope.slice(start_char..end_char).to_string(),
        }
    }

    /// The window's row for buffer line `line`.
    pub(crate) fn row(&self, line: usize) -> usize {
        line - self.first_line
    }
}

fn starts_at_column_zero(first: Option<char>) -> bool {
    first.is_some_and(|c| !c.is_whitespace())
}

/// A column-0 line that can start a parse: not a closer, which would end
/// the item above rather than begin one, and not a clause (`else:`,
/// `except:`) that continues the statement above.
fn opens_top_level(mut chars: impl Iterator<Item = char>) -> bool {
    let first = chars.next();
    if !starts_at_column_zero(first) || matches!(first, Some('}' | ')' | ']')) {
        return false;
    }
    let word: String = first
        .into_iter()
        .chain(chars)
        .take(CLAUSE_PROBE_CHARS)
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    !CONTINUATIONS.contains(&word.as_str())
}

/// Clause keywords that continue the statement on the lines above.
const CONTINUATIONS: [&str; 5] = ["else", "elif", "except", "finally", "catch"];
/// Longer than any [`CONTINUATIONS`] word, so a longer identifier is not
/// cut down to one.
const CLAUSE_PROBE_CHARS: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_file(functions: usize) -> String {
        (0..functions)
            .map(|i| format!("fn f{i}() {{\n    let x = {i};\n    x + 1;\n}}\n"))
            .collect()
    }

    #[test]
    fn a_window_is_the_enclosing_top_level_item() {
        let buffer = Buffer::from_bytes(rust_file(3).as_bytes()).unwrap();
        let window = Window::around(&buffer, 5, 5);
        assert_eq!(window.first_line, 4);
        assert_eq!(window.text, "fn f1() {\n    let x = 1;\n    x + 1;\n}\n");
        assert_eq!(window.row(5), 1);
    }

    #[test]
    fn a_window_on_a_twenty_thousand_line_file_copies_one_item() {
        let source = rust_file(5_000);
        let buffer = Buffer::from_bytes(source.as_bytes()).unwrap();
        assert_eq!(buffer.len_lines(), 20_001);
        let window = Window::around(&buffer, 10_001, 10_001);
        assert_eq!(window.first_line, 10_000);
        assert!(window.text.len() < 100, "copied {}", window.text.len());
    }

    #[test]
    fn a_window_does_not_start_at_a_column_zero_clause() {
        let buffer = Buffer::from_bytes(b"x = 1\nif x:\n    a\nelse:\n    b\n").unwrap();
        assert_eq!(Window::around(&buffer, 4, 4).first_line, 1);
        let buffer = Buffer::from_bytes(b"elsewhere = 1\n    b\n").unwrap();
        assert_eq!(Window::around(&buffer, 1, 1).first_line, 0);
    }

    #[test]
    fn a_window_without_column_zero_lines_stops_at_the_cap() {
        let source = "    x;\n".repeat(10_000);
        let buffer = Buffer::from_bytes(source.as_bytes()).unwrap();
        let window = Window::around(&buffer, 5_000, 5_000);
        assert_eq!(window.first_line, 5_000 - MAX_CONTEXT_LINES);
        assert_eq!(window.text.lines().count(), 2 * MAX_CONTEXT_LINES + 1);
    }
}
