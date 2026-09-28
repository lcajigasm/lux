//! Comment/uncomment line and block. Headless — `lux-core` has no language
//! or syntax awareness (that arrives with tree-sitter in a later phase), so
//! the comment token(s) are parameters, not looked up from a language
//! table.

use super::{line_range, touched_lines};
use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::position::Position;
use crate::selection::Selection;

/// Toggles a line comment (`token`, e.g. `"//"` or `"#"`) on every line the
/// selection touches: if every one of them is already commented, removes
/// the token from each; otherwise adds `token` + one space after each
/// line's leading whitespace.
pub fn toggle_line_comment(
    buffer: &mut Buffer,
    selection: Selection,
    token: &str,
) -> (Vec<Edit>, Selection) {
    let start_line = *touched_lines(selection, buffer.rope()).start();
    let lines: Vec<usize> = touched_lines(selection, buffer.rope()).collect();

    let already_commented: Vec<bool> = lines
        .iter()
        .map(|&line| {
            let range = line_range(buffer.rope(), line);
            let text = buffer.text_range(range);
            text.trim_start().starts_with(token)
        })
        .collect();
    let all_commented = already_commented.iter().all(|&c| c);

    let mut edits = Vec::new();
    for &line in lines.iter().rev() {
        let range = line_range(buffer.rope(), line);
        let text = buffer.text_range(range.clone());
        let leading_ws = text
            .chars()
            .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
            .count();
        let content_start = range.start + leading_ws;

        if all_commented {
            let after = &text[leading_ws..];
            if let Some(rest) = after.strip_prefix(token) {
                let extra_space = usize::from(rest.starts_with(' '));
                let edit = Edit::delete(buffer, content_start, token.chars().count() + extra_space);
                edit.apply(buffer);
                edits.push(edit);
            }
        } else {
            let edit = Edit::insert(content_start, format!("{token} "));
            edit.apply(buffer);
            edits.push(edit);
        }
    }
    // Comment/uncomment shifts char offsets per-line in a way that isn't
    // worth tracking precisely (unlike indent's uniform shift, each line's
    // delta depends on whether it already had a comment and any variable
    // leading whitespace) — collapse to the touched block's start, which is
    // always still valid.
    let new_pos = Position::from_char(buffer, line_range(buffer.rope(), start_line).start);
    (edits, Selection::cursor(new_pos))
}

/// Toggles a block comment (`start_token`/`end_token`, e.g. `"/*"`/`"*/"`)
/// around the selection: if it already starts and ends with the tokens,
/// removes them; otherwise wraps the selection with them.
pub fn toggle_block_comment(
    buffer: &mut Buffer,
    selection: Selection,
    start_token: &str,
    end_token: &str,
) -> (Vec<Edit>, Selection) {
    let (start, end) = selection.ordered();
    let text = buffer.text_range(start.char..end.char);

    if text.starts_with(start_token)
        && text.ends_with(end_token)
        && text.len() >= start_token.len() + end_token.len()
    {
        let end_start = end.char - end_token.chars().count();
        let end_edit = Edit::delete(buffer, end_start, end_token.chars().count());
        end_edit.apply(buffer);
        let start_edit = Edit::delete(buffer, start.char, start_token.chars().count());
        start_edit.apply(buffer);

        let new_start = Position::from_char(buffer, start.char);
        let new_end = Position::from_char(
            buffer,
            end.char - start_token.chars().count() - end_token.chars().count(),
        );
        (
            vec![end_edit, start_edit],
            Selection::new(new_start, new_end),
        )
    } else {
        let end_edit = Edit::insert(end.char, end_token);
        end_edit.apply(buffer);
        let start_edit = Edit::insert(start.char, start_token);
        start_edit.apply(buffer);

        // Only the start-token insertion (at `start.char`, before both
        // boundaries) shifts anything here: the end-token insertion sits at
        // `end.char`, which is the *right* edge of the new selection, not
        // shifted by an insertion at its own left edge.
        let new_start = Position::from_char(buffer, start.char + start_token.chars().count());
        let new_end = Position::from_char(buffer, end.char + start_token.chars().count());
        (
            vec![end_edit, start_edit],
            Selection::new(new_start, new_end),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn toggle_line_comment_comments_an_uncommented_line() {
        let mut buffer = Buffer::from_bytes(b"let x = 1;\n").unwrap();
        let sel = cursor_at(&buffer, 0);
        toggle_line_comment(&mut buffer, sel, "//");
        assert_eq!(buffer.text(), "// let x = 1;\n");
    }

    #[test]
    fn toggle_line_comment_uncomments_a_commented_line() {
        let mut buffer = Buffer::from_bytes(b"// let x = 1;\n").unwrap();
        let sel = cursor_at(&buffer, 0);
        toggle_line_comment(&mut buffer, sel, "//");
        assert_eq!(buffer.text(), "let x = 1;\n");
    }

    #[test]
    fn toggle_line_comment_preserves_indentation() {
        let mut buffer = Buffer::from_bytes(b"    let x = 1;\n").unwrap();
        let sel = cursor_at(&buffer, 6);
        toggle_line_comment(&mut buffer, sel, "//");
        assert_eq!(buffer.text(), "    // let x = 1;\n");
    }

    #[test]
    fn toggle_line_comment_over_a_block_comments_all_when_any_is_uncommented() {
        let mut buffer = Buffer::from_bytes(b"// a\nb\n").unwrap();
        let sel = range_at(&buffer, 0, 6);
        toggle_line_comment(&mut buffer, sel, "//");
        assert_eq!(buffer.text(), "// // a\n// b\n");
    }

    #[test]
    fn toggle_line_comment_over_a_block_uncomments_all_when_all_are_commented() {
        let mut buffer = Buffer::from_bytes(b"// a\n// b\n").unwrap();
        let sel = range_at(&buffer, 0, 9);
        toggle_line_comment(&mut buffer, sel, "//");
        assert_eq!(buffer.text(), "a\nb\n");
    }

    #[test]
    fn toggle_line_comment_round_trips() {
        let mut buffer = Buffer::from_bytes(b"let x = 1;\n").unwrap();
        let original = buffer.text();
        let sel = cursor_at(&buffer, 0);
        let (_, sel) = toggle_line_comment(&mut buffer, sel, "//");
        toggle_line_comment(&mut buffer, sel, "//");
        assert_eq!(buffer.text(), original);
    }

    #[test]
    fn multiline_line_comment_edits_are_replayable_and_undoable() {
        use crate::history::Transaction;

        let mut buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
        let before = range_at(&buffer, 0, 3);
        let (edits, after) = toggle_line_comment(&mut buffer, before, "//");
        let transaction = Transaction::new(edits, vec![before], vec![after]);
        assert_eq!(buffer.text(), "// a\n// b\n");
        transaction.undo(&mut buffer);
        assert_eq!(buffer.text(), "a\nb\n");
        transaction.apply(&mut buffer);
        assert_eq!(buffer.text(), "// a\n// b\n");
    }

    #[test]
    fn toggle_block_comment_wraps_the_selection() {
        let mut buffer = Buffer::from_bytes(b"let x = 1;").unwrap();
        let sel = range_at(&buffer, 4, 9); // "x = 1"
        let (_, new_sel) = toggle_block_comment(&mut buffer, sel, "/*", "*/");
        assert_eq!(buffer.text(), "let /*x = 1*/;");
        assert_eq!(new_sel.char_range(), 6..11);
    }

    #[test]
    fn toggle_block_comment_unwraps_an_already_wrapped_selection() {
        let mut buffer = Buffer::from_bytes(b"let /*x = 1*/;").unwrap();
        let sel = range_at(&buffer, 4, 13); // "/*x = 1*/"
        let (_, new_sel) = toggle_block_comment(&mut buffer, sel, "/*", "*/");
        assert_eq!(buffer.text(), "let x = 1;");
        assert_eq!(new_sel.char_range(), 4..9);
    }

    #[test]
    fn toggle_block_comment_round_trips() {
        let mut buffer = Buffer::from_bytes(b"let x = 1;").unwrap();
        let original = buffer.text();
        let sel = range_at(&buffer, 4, 9);
        let (_, wrapped) = toggle_block_comment(&mut buffer, sel, "/*", "*/");
        // The wrap branch returns a selection over just the content,
        // excluding the tokens it inserted (so the user's selection isn't
        // suddenly full of comment syntax) — expand back out over the
        // tokens so the second call detects and removes them.
        let full = range_at(
            &buffer,
            wrapped.char_range().start - 2,
            wrapped.char_range().end + 2,
        );
        toggle_block_comment(&mut buffer, full, "/*", "*/");
        assert_eq!(buffer.text(), original);
    }
}
