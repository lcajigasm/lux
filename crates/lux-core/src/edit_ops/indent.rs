//! Indent/dedent, respecting the buffer's detected indent style and width
//! (`Buffer::indent`) rather than a hardcoded tab or space count.

use super::{line_range, touched_lines};
use crate::buffer::{Buffer, IndentStyle};
use crate::edit::Edit;
use crate::position::Position;
use crate::selection::Selection;

fn indent_unit(buffer: &Buffer) -> String {
    match buffer.indent().style {
        IndentStyle::Tabs => "\t".to_string(),
        IndentStyle::Spaces => " ".repeat(buffer.indent().width as usize),
    }
}

/// Inserts one indent unit at the start of every line the selection
/// touches.
pub fn indent(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    let (first_line, last_line) = (*lines.start(), *lines.end());
    let unit = indent_unit(buffer);
    let unit_len = unit.chars().count();

    let line_starts: Vec<usize> = lines
        .map(|line| line_range(buffer.rope(), line).start)
        .collect();

    let mut edits = Vec::with_capacity(line_starts.len());
    for &line_start in line_starts.iter().rev() {
        let edit = Edit::insert(line_start, unit.clone());
        edit.apply(buffer);
        edits.push(edit);
    }
    // A position past `last_line` (e.g. `end.line` sitting on the phantom
    // trailing line `touched_lines` excluded) still shifts by every indent
    // that landed before it, but no more than the total that happened.
    let shift =
        |char_idx: usize, line: usize| char_idx + unit_len * (line.min(last_line) - first_line + 1);
    let new_anchor =
        Position::from_char(buffer, shift(selection.anchor.char, selection.anchor.line));
    let new_head = Position::from_char(buffer, shift(selection.head.char, selection.head.line));
    (edits, Selection::new(new_anchor, new_head))
}

/// Removes up to one indent unit's worth of leading whitespace from the
/// start of every line the selection touches — a full unit if that much
/// matching whitespace is there, otherwise whatever leading whitespace
/// exists (never more than what's actually there).
pub fn dedent(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    let (first_line, last_line) = (*lines.start(), *lines.end());
    let width = buffer.indent().width as usize;

    let removals: Vec<(usize, usize)> = lines
        .map(|line| {
            let range = line_range(buffer.rope(), line);
            let text = buffer.text_range(range.clone());
            let remove_len = if text.starts_with('\t') {
                1
            } else {
                text.chars().take_while(|&c| c == ' ').count().min(width)
            };
            (range.start, remove_len)
        })
        .collect();

    let mut edits = Vec::new();
    for &(line_start, remove_len) in removals.iter().rev() {
        if remove_len == 0 {
            continue;
        }
        let edit = Edit::delete(buffer, line_start, remove_len);
        edit.apply(buffer);
        edits.push(edit);
    }
    // Same phantom-trailing-line concern as `indent`'s shift: a position on
    // a line past `last_line` (not in `removals`) still owes the full
    // total, not a per-line lookup that would index out of bounds.
    let shift_for = |char_idx: usize, line: usize| {
        let mut shift = 0usize;
        for (i, l) in (first_line..=line.min(last_line)).enumerate() {
            let (line_start, remove_len) = removals[i];
            if l == line {
                shift += remove_len.min(char_idx.saturating_sub(line_start));
            } else {
                shift += remove_len;
            }
        }
        char_idx - shift
    };
    let new_anchor = Position::from_char(
        buffer,
        shift_for(selection.anchor.char, selection.anchor.line),
    );
    let new_head = Position::from_char(buffer, shift_for(selection.head.char, selection.head.line));
    (edits, Selection::new(new_anchor, new_head))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;
    use crate::multicursor::MultiCursor;

    fn cursor_at(buffer: &Buffer, char_idx: usize) -> Selection {
        Selection::cursor(Position::from_char(buffer, char_idx))
    }

    fn range_at(buffer: &Buffer, from: usize, to: usize) -> Selection {
        Selection::new(
            Position::from_char(buffer, from),
            Position::from_char(buffer, to),
        )
    }

    #[test]
    fn indent_adds_the_detected_unit_to_a_single_line() {
        let mut buffer = Buffer::from_bytes(b"    let x = 1;\n    let y = 2;\n").unwrap();
        let sel = cursor_at(&buffer, 10);
        let (_, sel) = indent(&mut buffer, sel);
        assert_eq!(buffer.text(), "        let x = 1;\n    let y = 2;\n");
        assert_eq!(sel.head.char, 14);
    }

    #[test]
    fn multiline_indent_and_dedent_transactions_undo_without_corrupting_text() {
        let original = "    one\n    two";
        let mut buffer = Buffer::from_bytes(original.as_bytes()).unwrap();
        let selection = range_at(&buffer, 0, buffer.rope().len_chars());
        let cursors = MultiCursor::new(selection);
        let mut history = History::new();

        let (dedent_transaction, dedented) =
            super::super::apply_to_each(&mut buffer, &cursors, dedent);
        history.record(dedent_transaction);
        assert_eq!(buffer.text(), "one\ntwo");

        let (indent_transaction, _) = super::super::apply_to_each(&mut buffer, &dedented, indent);
        history.record(indent_transaction);
        assert_eq!(buffer.text(), original);
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "one\ntwo");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), original);
    }

    #[test]
    fn indent_over_a_multiline_selection_indents_every_line() {
        let mut buffer = Buffer::from_bytes(b"  a\n  b\n  c\n").unwrap();
        let sel = range_at(&buffer, 0, 9);
        let (_, new_sel) = indent(&mut buffer, sel);
        assert_eq!(buffer.text(), "    a\n    b\n    c\n");
        // Each of the 3 touched lines shifts the tail cursor by one unit
        // more than the ones before it.
        assert_eq!(new_sel.char_range().end, 9 + 2 * 3);
    }

    #[test]
    fn indent_uses_tabs_when_the_buffer_detected_tabs() {
        let mut buffer = Buffer::from_bytes(b"\tlet x = 1;\n\tlet y = 2;\n").unwrap();
        let sel = cursor_at(&buffer, 0);
        indent(&mut buffer, sel);
        assert_eq!(buffer.text(), "\t\tlet x = 1;\n\tlet y = 2;\n");
    }

    #[test]
    fn dedent_removes_a_full_unit() {
        let mut buffer = Buffer::from_bytes(b"    let x = 1;\n    let y = 2;\n").unwrap();
        let sel = cursor_at(&buffer, 10);
        let (_, sel) = dedent(&mut buffer, sel);
        assert_eq!(buffer.text(), "let x = 1;\n    let y = 2;\n");
        assert_eq!(sel.head.char, 6);
    }

    #[test]
    fn dedent_removes_only_the_whitespace_that_exists() {
        let mut buffer = Buffer::from_bytes(b"  let x = 1;\n").unwrap(); // only 2 spaces, width 4
        let sel = cursor_at(&buffer, 5);
        let (_, sel) = dedent(&mut buffer, sel);
        assert_eq!(buffer.text(), "let x = 1;\n");
        assert_eq!(sel.head.char, 3);
    }

    #[test]
    fn dedent_on_a_line_with_no_leading_whitespace_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"let x = 1;\n").unwrap();
        let sel = cursor_at(&buffer, 3);
        let (edits, sel) = dedent(&mut buffer, sel);
        assert_eq!(buffer.text(), "let x = 1;\n");
        assert!(edits.is_empty());
        assert_eq!(sel.head.char, 3);
    }

    #[test]
    fn dedent_removes_a_single_leading_tab() {
        let mut buffer = Buffer::from_bytes(b"\tlet x = 1;\n").unwrap();
        let sel = cursor_at(&buffer, 5);
        let (_, sel) = dedent(&mut buffer, sel);
        assert_eq!(buffer.text(), "let x = 1;\n");
        assert_eq!(sel.head.char, 4);
    }

    #[test]
    fn dedent_over_a_multiline_selection_dedents_every_line() {
        let mut buffer = Buffer::from_bytes(b"    a\n    b\n    c\n").unwrap();
        let sel = range_at(&buffer, 0, 17);
        let (_, new_sel) = dedent(&mut buffer, sel);
        assert_eq!(buffer.text(), "a\nb\nc\n");
        assert_eq!(new_sel.char_range().end, 5); // "a\nb\nc" == 5 chars
    }

    #[test]
    fn indent_then_dedent_round_trips() {
        let mut buffer = Buffer::from_bytes(b"abc\ndef\n").unwrap();
        let sel = range_at(&buffer, 0, 8);
        let (_, sel) = indent(&mut buffer, sel);
        assert_eq!(buffer.text(), "    abc\n    def\n");
        dedent(&mut buffer, sel);
        assert_eq!(buffer.text(), "abc\ndef\n");
    }
}
