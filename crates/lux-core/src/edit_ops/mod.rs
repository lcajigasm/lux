//! Higher-level editing commands, each composing the `Buffer::insert`/
//! `Buffer::delete` primitives (and the invertible `Edit` wrapper around
//! them) into one user-facing operation: insert, delete, delete word/line,
//! duplicate line, move line up/down, indent/dedent, comment/uncomment line
//! and block, join lines, transpose, sort lines, change case.
//!
//! Every operation has the same shape: `fn(&mut Buffer, Selection, ...) ->
//! (Vec<Edit>, Selection)`. It mutates `buffer` directly and returns the
//! edits it made (in the order it made them) plus the resulting cursor —
//! the caller wraps that in a `Transaction` and hands it to `History` to
//! make it undoable. Operations never touch `History` themselves; they're
//! pure text-editing logic.

mod comment;
mod indent;
mod lines;
mod transform;
mod whitespace;
mod word;

pub use comment::{toggle_block_comment, toggle_line_comment};
pub use indent::{dedent, indent};
pub use lines::{
    delete_line, duplicate_line, join_lines, move_line_down, move_line_up, sort_lines,
};
pub use transform::{CaseChange, change_case, transpose};
pub use whitespace::{ensure_final_newline, trim_trailing_whitespace};
pub use word::{delete_word_backward, delete_word_forward};

use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::history::Transaction;
use crate::multicursor::MultiCursor;
use crate::position::Position;
use crate::selection::Selection;
use ropey::Rope;

/// The char range `[line_to_char(line), end)` covering a whole line
/// including its trailing terminator, where `end` is the start of the next
/// line, or the document's end if `line` is the last one (which may have no
/// terminator at all).
pub(crate) fn line_range(rope: &Rope, line: usize) -> std::ops::Range<usize> {
    let start = rope.line_to_char(line);
    let end = if line + 1 < rope.len_lines() {
        rope.line_to_char(line + 1)
    } else {
        rope.len_chars()
    };
    start..end
}

/// The inclusive line range `selection` touches. `ropey` counts a phantom
/// empty final line when the document ends in a terminator (so "a\n" is 2
/// lines: "a\n" and ""); a multi-line selection whose end sits exactly at
/// the start of that phantom line hasn't actually selected any of it, so
/// it's excluded here — otherwise line-oriented ops (indent, delete-line,
/// sort, ...) would silently act on a line that doesn't exist. A collapsed
/// cursor deliberately placed on that empty line still counts as touching
/// it.
pub(crate) fn touched_lines(selection: Selection, rope: &Rope) -> std::ops::RangeInclusive<usize> {
    let (start, end) = selection.ordered();
    let mut last_line = end.line;
    if last_line > start.line && end.char == rope.line_to_char(end.line) {
        last_line -= 1;
    }
    start.line..=last_line
}

/// Splits text representing one whole line (as returned by `line_range`)
/// into its content and trailing terminator — `""` for the terminator if
/// this is the document's last line and it has none.
pub(crate) fn split_terminator(text: &str) -> (&str, &str) {
    for term in ["\r\n", "\n", "\r"] {
        if let Some(content) = text.strip_suffix(term) {
            return (content, term);
        }
    }
    (text, "")
}

/// Splits multi-line `text` into `(content, terminator)` pairs, one per
/// line, preserving each line's own terminator exactly (the last one is
/// `""` if `text` doesn't end in a line break).
pub(crate) fn split_lines_keep_terminators(text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let (content, term, consumed) = match rest.find(['\n', '\r']) {
            Some(idx)
                if rest.as_bytes()[idx] == b'\r'
                    && rest.as_bytes().get(idx + 1) == Some(&b'\n') =>
            {
                (&rest[..idx], "\r\n", idx + 2)
            }
            Some(idx) => (&rest[..idx], &rest[idx..idx + 1], idx + 1),
            None => (rest, "", rest.len()),
        };
        result.push((content.to_string(), term.to_string()));
        rest = &rest[consumed..];
    }
    result
}

/// Replaces `text` in `selection`'s empty cursor with... no — inserts
/// `text` at the cursor, first deleting the selection if it isn't empty
/// (typing over a selection replaces it). The common case behind plain
/// "insert" and "delete" (an empty `text` deletes the selection and inserts
/// nothing).
pub fn insert_text(
    buffer: &mut Buffer,
    selection: Selection,
    text: &str,
) -> (Vec<Edit>, Selection) {
    let mut edits = Vec::new();
    let insert_at = if !selection.is_empty() {
        let range = selection.char_range();
        let edit = Edit::delete(buffer, range.start, range.len());
        edit.apply(buffer);
        edits.push(edit);
        range.start
    } else {
        selection.head.char
    };
    if !text.is_empty() {
        let edit = Edit::insert(insert_at, text);
        edit.apply(buffer);
        edits.push(edit);
    }
    let new_pos = Position::from_char(buffer, insert_at + text.chars().count());
    (edits, Selection::cursor(new_pos))
}

/// Deletes the selection. A no-op (returns no edits) if it's already empty
/// — plain Delete/Backspace with nothing selected is handled by
/// `delete_word_*` or the caller extending the selection by one grapheme
/// first, not by this function guessing.
pub fn delete_selection(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    if selection.is_empty() {
        return (Vec::new(), selection);
    }
    let range = selection.char_range();
    let edit = Edit::delete(buffer, range.start, range.len());
    edit.apply(buffer);
    let new_pos = Position::from_char(buffer, range.start);
    (vec![edit], Selection::cursor(new_pos))
}

fn edit_len_delta(edit: &Edit) -> isize {
    match edit {
        Edit::Insert { text, .. } => text.chars().count() as isize,
        Edit::Delete { text, .. } => -(text.chars().count() as isize),
    }
}

/// Applies `op` to every cursor in `multicursor`, back-to-front (highest
/// document position first) so that one cursor's edit never invalidates the
/// char offsets a not-yet-processed cursor was computed against — the
/// N-cursors-in-one-transaction guarantee from the phase plan. Returns one
/// `Transaction` covering every edit and the `MultiCursor` reflecting each
/// cursor's own resulting position (re-normalized and overlap-merged, same
/// as any other `MultiCursor` mutation).
///
/// Each `op` call computes its own resulting selection relative to the
/// buffer state *at that moment* — but later iterations (further left,
/// lower offsets) still have edits to apply, which shift everything at or
/// after their position, including positions already handed back by an
/// earlier (further right) iteration. So this doesn't trust `op`'s returned
/// selection as final: it records the raw char offsets plus how much
/// *more* shifting happened after that point, and only resolves the final
/// `Position`s once every edit has landed.
pub fn apply_to_each(
    buffer: &mut Buffer,
    multicursor: &MultiCursor,
    mut op: impl FnMut(&mut Buffer, Selection) -> (Vec<Edit>, Selection),
) -> (Transaction, MultiCursor) {
    let selections_before = multicursor.cursors().to_vec();
    let mut all_edits = Vec::new();
    let mut raw_after = Vec::with_capacity(selections_before.len());
    let mut shift_at_capture = Vec::with_capacity(selections_before.len());

    let mut cumulative_shift: isize = 0;
    for &sel in selections_before.iter().rev() {
        let (edits, new_sel) = op(buffer, sel);
        for edit in &edits {
            cumulative_shift += edit_len_delta(edit);
        }
        raw_after.push((new_sel.anchor.char, new_sel.head.char));
        shift_at_capture.push(cumulative_shift);
        all_edits.extend(edits);
    }
    let total_shift = cumulative_shift;

    let mut selections_after: Vec<Selection> = raw_after
        .into_iter()
        .zip(shift_at_capture)
        .map(|((anchor, head), captured)| {
            let remaining = total_shift - captured;
            let new_anchor = (anchor as isize + remaining) as usize;
            let new_head = (head as isize + remaining) as usize;
            Selection::new(
                Position::from_char(buffer, new_anchor),
                Position::from_char(buffer, new_head),
            )
        })
        .collect();
    selections_after.reverse();

    // No reverse on `all_edits`: it was built via `.extend()` in the same
    // order `op` actually applied each cursor's edits to `buffer`
    // (highest-offset cursor first) — that's already the exact
    // chronological order `Transaction::apply`/`undo` need. Reversing it
    // here was a real bug (found and fixed while building the find/replace
    // deliverable, which hit the identical mistake): it silently broke
    // undo for every multi-cursor edit built through this function.
    let transaction = Transaction::new(all_edits, selections_before, selections_after.clone());
    let new_multicursor = MultiCursor::from_cursors(selections_after);
    (transaction, new_multicursor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    fn cursor_at(buffer: &Buffer, char_idx: usize) -> Selection {
        Selection::cursor(pos_at(buffer, char_idx))
    }

    fn range_at(buffer: &Buffer, from: usize, to: usize) -> Selection {
        Selection::new(pos_at(buffer, from), pos_at(buffer, to))
    }

    #[test]
    fn insert_text_at_empty_cursor() {
        let mut buffer = Buffer::from_bytes(b"helloworld").unwrap();
        let sel = cursor_at(&buffer, 5);
        let (edits, sel) = insert_text(&mut buffer, sel, " ");
        assert_eq!(buffer.text(), "hello world");
        assert_eq!(edits.len(), 1);
        assert_eq!(sel.head.char, 6);
    }

    #[test]
    fn insert_text_replaces_a_selection() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = range_at(&buffer, 6, 11);
        let (edits, sel) = insert_text(&mut buffer, sel, "Rust");
        assert_eq!(buffer.text(), "hello Rust");
        assert_eq!(edits.len(), 2); // delete the selection, then insert
        assert_eq!(sel.head.char, 10);
    }

    #[test]
    fn delete_selection_removes_the_range() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let sel = range_at(&buffer, 5, 11);
        let (edits, sel) = delete_selection(&mut buffer, sel);
        assert_eq!(buffer.text(), "hello");
        assert_eq!(edits.len(), 1);
        assert!(sel.is_empty());
        assert_eq!(sel.head.char, 5);
    }

    #[test]
    fn delete_selection_is_a_noop_when_empty() {
        let mut buffer = Buffer::from_bytes(b"hello").unwrap();
        let sel = cursor_at(&buffer, 2);
        let (edits, sel) = delete_selection(&mut buffer, sel);
        assert_eq!(buffer.text(), "hello");
        assert!(edits.is_empty());
        assert_eq!(sel.head.char, 2);
    }

    #[test]
    fn apply_to_each_inserts_at_every_cursor_in_one_transaction() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let mc = MultiCursor::from_cursors(vec![cursor_at(&buffer, 0), cursor_at(&buffer, 2)]);
        let (transaction, new_mc) = apply_to_each(&mut buffer, &mc, |buffer, sel| {
            insert_text(buffer, sel, "X")
        });
        assert_eq!(buffer.text(), "XabX");
        assert_eq!(transaction.edits.len(), 2);
        assert_eq!(new_mc.count(), 2);
        let positions: Vec<usize> = new_mc.cursors().iter().map(|c| c.head.char).collect();
        assert_eq!(positions, vec![1, 4]);
    }

    #[test]
    fn apply_to_each_records_selections_before_and_after() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let mc = MultiCursor::new(cursor_at(&buffer, 1));
        let (transaction, _) = apply_to_each(&mut buffer, &mc, |buffer, sel| {
            insert_text(buffer, sel, "!")
        });
        assert_eq!(transaction.selections_before, vec![cursor_at(&buffer, 1)]);
        assert_eq!(transaction.selections_after[0].head.char, 2);
    }

    /// Regression test: an earlier version of `apply_to_each` reversed
    /// `all_edits` one time too many, which happened to leave `buffer.text()`
    /// and the returned cursor positions correct (this function's other two
    /// tests didn't catch it) while silently putting `Transaction.edits` in
    /// the wrong order for `undo`/`redo` to replay. Multi-edit transactions
    /// only prove themselves correct by actually being undone and redone.
    #[test]
    fn apply_to_each_transaction_is_undoable_and_redoable() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let original = buffer.text();
        let mc = MultiCursor::from_cursors(vec![cursor_at(&buffer, 0), cursor_at(&buffer, 2)]);
        let (transaction, _) = apply_to_each(&mut buffer, &mc, |buffer, sel| {
            insert_text(buffer, sel, "X")
        });
        let after_apply = buffer.text();
        assert_eq!(after_apply, "XabX");

        transaction.undo(&mut buffer);
        assert_eq!(buffer.text(), original);

        transaction.apply(&mut buffer);
        assert_eq!(buffer.text(), after_apply);
    }

    /// Same regression, exercised through a real multi-edit `History` flow
    /// (not just calling `Transaction::undo` directly) — `History::undo`
    /// returning the correct `selections_before` depends on the edits list
    /// being in the right order too, since `undo` replays it in reverse.
    #[test]
    fn apply_to_each_transaction_undoes_correctly_through_history() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let original = buffer.text();
        let mc = MultiCursor::from_cursors(vec![cursor_at(&buffer, 0), cursor_at(&buffer, 2)]);
        let (transaction, _) = apply_to_each(&mut buffer, &mc, |buffer, sel| {
            insert_text(buffer, sel, "X")
        });
        assert_eq!(buffer.text(), "XabX");

        let mut history = crate::history::History::new();
        history.record(transaction);
        let restored = history.undo(&mut buffer);
        assert_eq!(buffer.text(), original);
        assert_eq!(
            restored,
            Some(vec![cursor_at(&buffer, 0), cursor_at(&buffer, 2)])
        );
    }

    #[test]
    fn split_lines_keep_terminators_handles_mixed_and_missing_terminator() {
        let lines = split_lines_keep_terminators("a\nb\r\nc");
        assert_eq!(
            lines,
            vec![
                ("a".to_string(), "\n".to_string()),
                ("b".to_string(), "\r\n".to_string()),
                ("c".to_string(), "".to_string()),
            ]
        );
    }
}
