//! Line-oriented editing commands: delete/duplicate/move/join/sort whole
//! lines. All work in terms of the *lines a selection touches*
//! (`touched_lines`), so they behave the same whether given a collapsed
//! cursor or a multi-line selection.

use super::{line_range, split_lines_keep_terminators, split_terminator, touched_lines};
use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::position::Position;
use crate::selection::Selection;
use std::ops::Range;

/// Deletes every line the selection touches, terminator included.
pub fn delete_line(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    let range = {
        let rope = buffer.rope();
        line_range(rope, *lines.start()).start..line_range(rope, *lines.end()).end
    };
    let edit = Edit::delete(buffer, range.start, range.end - range.start);
    edit.apply(buffer);
    let new_pos = Position::from_char(buffer, range.start);
    (vec![edit], Selection::cursor(new_pos))
}

/// Duplicates every line the selection touches, inserting the copy right
/// after. The selection lands on the duplicate, at the same relative
/// offsets it had in the original.
pub fn duplicate_line(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    if !selection.is_empty() {
        let range = selection.char_range();
        let text = buffer.text_range(range.clone());
        let count = text.chars().count();
        let edit = Edit::insert(range.end, text);
        edit.apply(buffer);
        let new_anchor = Position::from_char(buffer, range.end);
        let new_head = Position::from_char(buffer, range.end + count);
        return (vec![edit], Selection::new(new_anchor, new_head));
    }

    let lines = touched_lines(selection, buffer.rope());
    let block = {
        let rope = buffer.rope();
        line_range(rope, *lines.start()).start..line_range(rope, *lines.end()).end
    };
    let original = buffer.text_range(block.clone());
    let (content, term) = split_terminator(&original);

    // If the block is the document's last line and has no terminator, the
    // copy needs one inserted *before* it (to separate it from the
    // original) rather than appended to its own end — appending would just
    // run the two copies together with nothing between them.
    let (insert_text, content_offset) = if term.is_empty() {
        let eol = buffer.eol().as_str();
        (format!("{eol}{content}"), eol.chars().count())
    } else {
        (original.clone(), 0)
    };

    let anchor_offset = selection.anchor.char - block.start;
    let head_offset = selection.head.char - block.start;

    let edit = Edit::insert(block.end, insert_text);
    edit.apply(buffer);

    let duplicate_start = block.end + content_offset;
    let new_anchor = Position::from_char(buffer, duplicate_start + anchor_offset);
    let new_head = Position::from_char(buffer, duplicate_start + head_offset);
    (vec![edit], Selection::new(new_anchor, new_head))
}

struct SwapResult {
    edits: Vec<Edit>,
    /// Where `first_text`'s own content now starts in the buffer.
    first_content_start: usize,
    /// Where `second_text`'s own content now starts in the buffer.
    second_content_start: usize,
}

/// Replaces `[range.start, range.end)` — which holds exactly `first_text`
/// followed by `second_text` — with the two swapped. Handles the one edge
/// case where a naive swap would lose a separator: if `second_text` is the
/// document's last line and has no terminator, the swap borrows
/// `first_text`'s terminator as the connector instead of duplicating one
/// that was never there.
fn swap_adjacent_blocks(
    buffer: &mut Buffer,
    range: Range<usize>,
    first_text: &str,
    second_text: &str,
    doc_end: usize,
) -> SwapResult {
    let (first_content, first_term) = split_terminator(first_text);
    let (second_content, second_term) = split_terminator(second_text);

    let (swapped, first_content_start, second_content_start) = if second_term.is_empty()
        && range.end == doc_end
    {
        let text = format!("{second_content}{first_term}{first_content}");
        let first_start = range.start + second_content.chars().count() + first_term.chars().count();
        (text, first_start, range.start)
    } else {
        let text = format!("{second_content}{second_term}{first_content}{first_term}");
        let first_start =
            range.start + second_content.chars().count() + second_term.chars().count();
        (text, first_start, range.start)
    };

    let delete_edit = Edit::delete(buffer, range.start, range.end - range.start);
    delete_edit.apply(buffer);
    let insert_edit = Edit::insert(range.start, swapped);
    insert_edit.apply(buffer);
    SwapResult {
        edits: vec![delete_edit, insert_edit],
        first_content_start,
        second_content_start,
    }
}

/// Swaps the selection's lines with the line immediately above. A no-op if
/// the selection already touches the document's first line.
pub fn move_line_up(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    let (first_line, last_line) = (*lines.start(), *lines.end());
    if first_line == 0 {
        return (Vec::new(), selection);
    }
    let (prev_range, block_range, doc_end) = {
        let rope = buffer.rope();
        let prev_range = line_range(rope, first_line - 1);
        let block_range = line_range(rope, first_line).start..line_range(rope, last_line).end;
        (prev_range, block_range, rope.len_chars())
    };
    let prev_text = buffer.text_range(prev_range.clone());
    let block_text = buffer.text_range(block_range.clone());

    let full_range = prev_range.start..block_range.end;
    let result = swap_adjacent_blocks(buffer, full_range, &prev_text, &block_text, doc_end);

    let anchor_offset = selection.anchor.char - block_range.start;
    let head_offset = selection.head.char - block_range.start;
    let new_anchor = Position::from_char(buffer, result.second_content_start + anchor_offset);
    let new_head = Position::from_char(buffer, result.second_content_start + head_offset);
    (result.edits, Selection::new(new_anchor, new_head))
}

/// Swaps the selection's lines with the line immediately below. A no-op if
/// the selection already touches the document's last line.
pub fn move_line_down(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    let (first_line, last_line) = (*lines.start(), *lines.end());
    let (block_range, next_range, doc_end, has_next) = {
        let rope = buffer.rope();
        let block_range = line_range(rope, first_line).start..line_range(rope, last_line).end;
        let has_next =
            last_line + 1 < rope.len_lines() && line_range(rope, last_line).end < rope.len_chars();
        let next_range = if has_next {
            line_range(rope, last_line + 1)
        } else {
            block_range.end..block_range.end
        };
        (block_range, next_range, rope.len_chars(), has_next)
    };
    if !has_next {
        return (Vec::new(), selection);
    }
    let block_text = buffer.text_range(block_range.clone());
    let next_text = buffer.text_range(next_range.clone());

    let full_range = block_range.start..next_range.end;
    let result = swap_adjacent_blocks(buffer, full_range, &block_text, &next_text, doc_end);

    let anchor_offset = selection.anchor.char - block_range.start;
    let head_offset = selection.head.char - block_range.start;
    let new_anchor = Position::from_char(buffer, result.first_content_start + anchor_offset);
    let new_head = Position::from_char(buffer, result.first_content_start + head_offset);
    (result.edits, Selection::new(new_anchor, new_head))
}

/// Joins every line the selection touches into one, replacing each line
/// break (and the following line's leading whitespace) with a single
/// space. A collapsed cursor joins its line with the next one.
pub fn join_lines(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    let (start_line, end_line) = (*lines.start(), *lines.end());

    let mut available = 0usize;
    let mut probe_line = start_line;
    loop {
        let probe_end = line_range(buffer.rope(), probe_line).end;
        if probe_end >= buffer.rope().len_chars() {
            break;
        }
        available += 1;
        probe_line += 1;
    }
    let num_joins = (end_line - start_line).max(1).min(available);

    let mut edits = Vec::with_capacity(num_joins * 2);
    let mut join_at = 0;
    for _ in 0..num_joins {
        let this_line = line_range(buffer.rope(), start_line);
        let this_line_text = buffer.text_range(this_line.clone());
        let (content, _) = split_terminator(&this_line_text);
        let content_end = this_line.start + content.chars().count();
        let next_line_end = line_range(buffer.rope(), start_line + 1).end;
        let leading_ws = buffer
            .text_range(this_line.end..next_line_end)
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();

        let delete_edit = Edit::delete(
            buffer,
            content_end,
            this_line.end + leading_ws - content_end,
        );
        delete_edit.apply(buffer);
        let insert_edit = Edit::insert(content_end, " ");
        insert_edit.apply(buffer);
        edits.push(delete_edit);
        edits.push(insert_edit);
        join_at = content_end;
    }

    if edits.is_empty() {
        return (edits, selection);
    }
    let new_pos = Position::from_char(buffer, join_at);
    (edits, Selection::cursor(new_pos))
}

/// Sorts the lines the selection touches. A no-op if the selection doesn't
/// span more than one line — there's nothing to reorder.
pub fn sort_lines(
    buffer: &mut Buffer,
    selection: Selection,
    ascending: bool,
) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    let (start_line, end_line) = (*lines.start(), *lines.end());
    if end_line <= start_line {
        return (Vec::new(), selection);
    }
    let block = {
        let rope = buffer.rope();
        line_range(rope, start_line).start..line_range(rope, end_line).end
    };
    let text = buffer.text_range(block.clone());
    let split = split_lines_keep_terminators(&text);
    // Whether the block's last physical line had a terminator — preserved
    // positionally after sorting (on whichever content ends up last), not
    // carried along with whatever content originally had it. Otherwise
    // sorting would leave two lines glued together with no separator.
    let had_trailing_terminator = split.last().is_some_and(|(_, term)| !term.is_empty());

    let mut contents: Vec<String> = split.into_iter().map(|(content, _)| content).collect();
    if ascending {
        contents.sort();
    } else {
        contents.sort_by(|a, b| b.cmp(a));
    }

    let eol = buffer.eol().as_str();
    let last_idx = contents.len() - 1;
    let sorted_text: String = contents
        .into_iter()
        .enumerate()
        .map(|(i, content)| {
            if i == last_idx && !had_trailing_terminator {
                content
            } else {
                format!("{content}{eol}")
            }
        })
        .collect();

    let delete_edit = Edit::delete(buffer, block.start, block.end - block.start);
    delete_edit.apply(buffer);
    let insert_edit = Edit::insert(block.start, sorted_text);
    insert_edit.apply(buffer);

    let new_pos = Position::from_char(buffer, block.start);
    (vec![delete_edit, insert_edit], Selection::cursor(new_pos))
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
    fn delete_line_removes_the_whole_line_and_terminator() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        let sel = cursor_at(&buffer, 5); // inside "two"
        let (_, sel) = delete_line(&mut buffer, sel);
        assert_eq!(buffer.text(), "one\nthree\n");
        assert_eq!(sel.head.char, 4);
    }

    #[test]
    fn delete_line_on_the_last_line_without_terminator() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo").unwrap();
        let sel = cursor_at(&buffer, 5);
        delete_line(&mut buffer, sel);
        assert_eq!(buffer.text(), "one\n");
    }

    #[test]
    fn duplicate_line_inserts_a_copy_right_after() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        let sel = cursor_at(&buffer, 5); // inside "two"
        let (_, sel) = duplicate_line(&mut buffer, sel);
        assert_eq!(buffer.text(), "one\ntwo\ntwo\nthree\n");
        assert_eq!(sel.head.char, 9); // same column, on the duplicate
    }

    #[test]
    fn duplicate_line_on_the_last_line_without_terminator_adds_one() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo").unwrap();
        let sel = cursor_at(&buffer, 5);
        duplicate_line(&mut buffer, sel);
        assert_eq!(buffer.text(), "one\ntwo\ntwo");
    }

    #[test]
    fn duplicate_selection_inserts_copy_adjacent() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = range_at(&buffer, 0, 5); // "hello"
        let (_, sel) = duplicate_line(&mut buffer, sel);
        assert_eq!(buffer.text(), "hellohello world");
        assert_eq!(sel.char_range(), 5..10);
    }

    #[test]
    fn move_line_up_swaps_with_the_previous_line() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        let sel = cursor_at(&buffer, 5); // inside "two"
        let (_, sel) = move_line_up(&mut buffer, sel);
        assert_eq!(buffer.text(), "two\none\nthree\n");
        assert_eq!(sel.head.char, 1);
    }

    #[test]
    fn move_line_up_at_document_start_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo\n").unwrap();
        let sel = cursor_at(&buffer, 1);
        let (edits, _) = move_line_up(&mut buffer, sel);
        assert_eq!(buffer.text(), "one\ntwo\n");
        assert!(edits.is_empty());
    }

    #[test]
    fn move_line_up_handles_a_final_line_with_no_terminator() {
        let mut buffer = Buffer::from_bytes(b"a\nb\nc").unwrap();
        let sel = cursor_at(&buffer, 4); // on "c"
        let (_, sel) = move_line_up(&mut buffer, sel);
        assert_eq!(buffer.text(), "a\nc\nb");
        assert_eq!(sel.head.char, 2);
    }

    #[test]
    fn move_line_down_swaps_with_the_next_line() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        let sel = cursor_at(&buffer, 1); // inside "one"
        let (_, sel) = move_line_down(&mut buffer, sel);
        assert_eq!(buffer.text(), "two\none\nthree\n");
        assert_eq!(sel.head.char, 5);
    }

    #[test]
    fn move_line_down_at_document_end_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"one\ntwo\n").unwrap();
        let sel = cursor_at(&buffer, 5);
        let (edits, _) = move_line_down(&mut buffer, sel);
        assert_eq!(buffer.text(), "one\ntwo\n");
        assert!(edits.is_empty());
    }

    #[test]
    fn move_line_down_into_a_final_line_with_no_terminator() {
        let mut buffer = Buffer::from_bytes(b"a\nb\nc").unwrap();
        let sel = cursor_at(&buffer, 2); // on "b"
        let (_, sel) = move_line_down(&mut buffer, sel);
        assert_eq!(buffer.text(), "a\nc\nb");
        assert_eq!(sel.head.char, 4);
    }

    #[test]
    fn join_lines_replaces_break_and_leading_whitespace_with_a_space() {
        let mut buffer = Buffer::from_bytes(b"hello\n   world\n").unwrap();
        let sel = cursor_at(&buffer, 2);
        let (_, sel) = join_lines(&mut buffer, sel);
        assert_eq!(buffer.text(), "hello world\n");
        assert_eq!(sel.head.char, 5);
    }

    #[test]
    fn join_lines_over_a_multiline_selection_joins_them_all() {
        let mut buffer = Buffer::from_bytes(b"a\nb\nc\nd\n").unwrap();
        let sel = range_at(&buffer, 0, 5); // spans lines a, b, c
        join_lines(&mut buffer, sel);
        assert_eq!(buffer.text(), "a b c\nd\n");
    }

    #[test]
    fn join_lines_at_the_last_line_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"only\n").unwrap();
        let sel = cursor_at(&buffer, 1);
        let (edits, _) = join_lines(&mut buffer, sel);
        assert_eq!(buffer.text(), "only\n");
        assert!(edits.is_empty());
    }

    #[test]
    fn sort_lines_ascending() {
        let mut buffer = Buffer::from_bytes(b"banana\napple\ncherry\n").unwrap();
        let sel = range_at(&buffer, 0, 20);
        sort_lines(&mut buffer, sel, true);
        assert_eq!(buffer.text(), "apple\nbanana\ncherry\n");
    }

    #[test]
    fn sort_lines_descending() {
        let mut buffer = Buffer::from_bytes(b"banana\napple\ncherry\n").unwrap();
        let sel = range_at(&buffer, 0, 20);
        sort_lines(&mut buffer, sel, false);
        assert_eq!(buffer.text(), "cherry\nbanana\napple\n");
    }

    #[test]
    fn sort_lines_preserves_a_missing_final_terminator() {
        let mut buffer = Buffer::from_bytes(b"b\na").unwrap();
        let sel = range_at(&buffer, 0, 3);
        sort_lines(&mut buffer, sel, true);
        assert_eq!(buffer.text(), "a\nb");
    }

    #[test]
    fn sort_lines_single_line_is_a_noop() {
        let mut buffer = Buffer::from_bytes(b"only one line\n").unwrap();
        let sel = cursor_at(&buffer, 3);
        let (edits, _) = sort_lines(&mut buffer, sel, true);
        assert!(edits.is_empty());
    }
}
