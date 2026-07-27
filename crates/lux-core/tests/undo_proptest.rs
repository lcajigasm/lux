//! Property tests for the undo/redo invariants from the phase plan: undoing
//! any sequence of edits restores the exact original buffer text and cursor
//! position, redoing after that replays back to the exact pre-undo state,
//! and the cursor never lands outside the document at any point along the
//! way.
//!
//! Runs as its own integration test binary (not `#[cfg(test)]` inside the
//! lib) because that's the unit the phase gate names directly.

use lux_core::buffer::Buffer;
use lux_core::edit::Edit;
use lux_core::history::{History, Transaction};
use lux_core::position::Position;
use lux_core::selection::Selection;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum RawOp {
    Insert(usize, String),
    Delete(usize, usize),
}

fn raw_op_strategy() -> impl Strategy<Value = RawOp> {
    prop_oneof![
        (any::<usize>(), "[a-zA-Z0-9 \n]{1,5}").prop_map(|(idx, text)| RawOp::Insert(idx, text)),
        (any::<usize>(), any::<usize>()).prop_map(|(idx, len)| RawOp::Delete(idx, len)),
    ]
}

fn cursor_at(buffer: &Buffer, char_idx: usize) -> Selection {
    Selection::cursor(Position::from_char(buffer, char_idx))
}

/// Applies one `RawOp` to `buffer`, clamping its raw (effectively
/// unbounded) offsets into the buffer's current valid range so every
/// generated case is a legal edit. `cursor` is wherever the cursor actually
/// was before this op — recorded as the transaction's `selections_before`
/// as-is, since an edit can happen anywhere regardless of where the cursor
/// last was (the user can click elsewhere, or this can be a second cursor).
/// Records the resulting transaction in `history` and returns the cursor
/// after the edit, or `None` if this op was a no-op (e.g. a delete with
/// nothing to delete).
fn apply_clamped(
    buffer: &mut Buffer,
    history: &mut History,
    cursor: Selection,
    op: &RawOp,
) -> Option<Selection> {
    let len_chars = buffer.len_chars();
    match op {
        RawOp::Insert(raw_idx, text) => {
            let char_idx = raw_idx % (len_chars + 1);
            let edit = Edit::insert(char_idx, text.clone());
            edit.apply(buffer);
            let after = cursor_at(buffer, char_idx + text.chars().count());
            history.record(Transaction::single(edit, cursor, after));
            Some(after)
        }
        RawOp::Delete(raw_idx, raw_len) => {
            if len_chars == 0 {
                return None;
            }
            let char_idx = raw_idx % len_chars;
            let max_len = len_chars - char_idx;
            let len = raw_len % max_len + 1;
            let edit = Edit::delete(buffer, char_idx, len);
            edit.apply(buffer);
            let after = cursor_at(buffer, char_idx);
            history.record(Transaction::single(edit, cursor, after));
            Some(after)
        }
    }
}

proptest! {
    #[test]
    fn undo_then_redo_round_trips_exactly(
        initial in "[a-zA-Z0-9 \n]{0,50}",
        ops in prop::collection::vec(raw_op_strategy(), 0..30),
    ) {
        let mut buffer = Buffer::from_bytes(initial.as_bytes()).unwrap();
        let mut history = History::new();

        let original_text = buffer.text();
        let mut cursor = cursor_at(&buffer, 0);
        let original_cursor = cursor;

        let mut edit_count = 0usize;
        for op in &ops {
            if let Some(new_cursor) = apply_clamped(&mut buffer, &mut history, cursor, op) {
                cursor = new_cursor;
                edit_count += 1;
            }
            // Invariant: the cursor is always at a valid char offset.
            prop_assert!(cursor.head.char <= buffer.len_chars());
        }

        let final_text = buffer.text();
        let final_cursor = cursor;

        // Undo everything.
        let mut last_restored = None;
        let mut undo_steps = 0usize;
        while let Some(restored) = history.undo(&mut buffer) {
            last_restored = Some(restored);
            undo_steps += 1;
            prop_assert!(!history.can_undo() || undo_steps <= edit_count);
        }

        prop_assert_eq!(&buffer.text(), &original_text);
        if edit_count > 0 {
            let restored = last_restored.expect("at least one undo happened");
            prop_assert_eq!(restored, vec![original_cursor]);
        } else {
            prop_assert!(last_restored.is_none());
        }

        // Redo everything back to the pre-undo state.
        let mut last_redo_restored = None;
        while let Some(restored) = history.redo(&mut buffer) {
            last_redo_restored = Some(restored);
        }
        prop_assert_eq!(buffer.text(), final_text);
        if edit_count > 0 {
            // The last redo applied is the most recent transaction, so its
            // restored selection is exactly the final cursor after all ops.
            prop_assert_eq!(last_redo_restored, Some(vec![final_cursor]));
        } else {
            prop_assert!(last_redo_restored.is_none());
        }
    }
}
