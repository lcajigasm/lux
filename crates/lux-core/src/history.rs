//! Undo/redo built on invertible `Edit`s, not whole-document snapshots — the
//! specific mistake `lux-edit` v0 made cloning the whole rope per undo step
//! (ADR 0001). A `Transaction` is one undo step: the edits it applied, plus
//! the cursor selections immediately before and after, so undo restores
//! cursor positions, not just text.
//!
//! Consecutive transactions are coalesced when they're structurally a
//! continuation of each other (typing right after the last insert,
//! backspacing/deleting right where the last delete left off) — see
//! `should_coalesce`. This is deliberately *not* wall-clock-based: grouping
//! by elapsed time would mean sampling a clock to decide something about the
//! world, which is the polling pattern this project's rules rule out (see
//! `CLAUDE.md` rule 5). Structural adjacency is decidable from the edits
//! alone.

use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::selection::Selection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub edits: Vec<Edit>,
    pub selections_before: Vec<Selection>,
    pub selections_after: Vec<Selection>,
    /// Whether this transaction is eligible to coalesce with an adjacent one
    /// (see `should_coalesce`). Only single-cursor, single-edit transactions
    /// built via `Transaction::single` are chainable — a deliberate N-cursor
    /// edit built via `Transaction::new` always gets its own undo step,
    /// regardless of whether its edits happen to sit next to a later one.
    chainable: bool,
}

impl Transaction {
    /// A general transaction: one or more edits applied atomically (e.g. the
    /// same edit applied to N cursors in one go). Never coalesces.
    pub fn new(
        edits: Vec<Edit>,
        selections_before: Vec<Selection>,
        selections_after: Vec<Selection>,
    ) -> Transaction {
        Transaction {
            edits,
            selections_before,
            selections_after,
            chainable: false,
        }
    }

    /// A single-cursor, single-edit transaction — the common case of typing
    /// or backspacing one character (or pasting one run of text at one
    /// cursor). Eligible to coalesce with an adjacent transaction of the
    /// same kind.
    pub fn single(
        edit: Edit,
        selection_before: Selection,
        selection_after: Selection,
    ) -> Transaction {
        Transaction {
            edits: vec![edit],
            selections_before: vec![selection_before],
            selections_after: vec![selection_after],
            chainable: true,
        }
    }

    pub fn apply(&self, buffer: &mut Buffer) {
        for edit in &self.edits {
            edit.apply(buffer);
        }
    }

    /// Reverts every edit in this transaction, in reverse order — undoing a
    /// sequence of edits one at a time, latest first, is what makes each
    /// edit's recorded char offsets valid again.
    pub fn undo(&self, buffer: &mut Buffer) {
        for edit in self.edits.iter().rev() {
            edit.invert().apply(buffer);
        }
    }
}

/// True when `next` should collapse into `prev` as one undo step, rather
/// than becoming its own. Both must be `chainable`, and `next`'s single edit
/// must sit exactly where `prev`'s most recent edit left off: consecutive
/// inserts (plain typing) or consecutive deletes at the same position
/// (holding Delete) or immediately before it (holding Backspace).
fn should_coalesce(prev: &Transaction, next: &Transaction) -> bool {
    if !prev.chainable || !next.chainable {
        return false;
    }
    let (Some(prev_edit), Some(next_edit)) = (prev.edits.last(), next.edits.first()) else {
        return false;
    };
    match (prev_edit, next_edit) {
        (
            Edit::Insert {
                char_idx: p_idx,
                text: p_text,
            },
            Edit::Insert {
                char_idx: n_idx,
                text: n_text,
            },
        ) => *n_idx == p_idx + p_text.chars().count() && !n_text.is_empty(),
        (
            Edit::Delete {
                char_idx: p_idx,
                text: _,
            },
            Edit::Delete {
                char_idx: n_idx,
                text: n_text,
            },
        ) => {
            // Forward-delete chain (cursor fixed, e.g. holding Delete) or
            // backspace chain (cursor moves left, e.g. holding Backspace).
            (*n_idx == *p_idx || n_idx + n_text.chars().count() == *p_idx) && !n_text.is_empty()
        }
        _ => false,
    }
}

#[derive(Debug, Default)]
pub struct History {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    state: u64,
    next_state: u64,
}

#[derive(Debug)]
struct HistoryEntry {
    transaction: Transaction,
    before: u64,
    after: u64,
}

impl History {
    pub fn new() -> History {
        History::default()
    }

    /// Records `transaction`, which the caller has already applied to the
    /// buffer. Coalesces into the previous undo step when it's a structural
    /// continuation (see `should_coalesce`); otherwise pushes a new step.
    /// Recording anything clears the redo stack, since redoing past it would
    /// no longer replay onto the buffer state it was computed against.
    pub fn record(&mut self, transaction: Transaction) {
        if transaction.edits.is_empty() {
            return;
        }
        self.next_state = self
            .next_state
            .checked_add(1)
            .expect("history state exhausted");
        let before = self.state;
        self.state = self.next_state;
        self.redo_stack.clear();
        if let Some(last) = self.undo_stack.last_mut()
            && should_coalesce(&last.transaction, &transaction)
        {
            last.transaction.edits.extend(transaction.edits);
            last.transaction.selections_after = transaction.selections_after;
            last.after = self.state;
            return;
        }
        self.undo_stack.push(HistoryEntry {
            transaction,
            before,
            after: self.state,
        });
    }

    /// Undoes the last recorded step, mutating `buffer` in place. Returns
    /// the cursor selections to restore, or `None` if there's nothing to
    /// undo.
    pub fn undo(&mut self, buffer: &mut Buffer) -> Option<Vec<Selection>> {
        let entry = self.undo_stack.pop()?;
        entry.transaction.undo(buffer);
        let selections = entry.transaction.selections_before.clone();
        self.state = entry.before;
        self.redo_stack.push(entry);
        Some(selections)
    }

    /// Re-applies the last undone step. Returns the cursor selections to
    /// restore, or `None` if there's nothing to redo.
    pub fn redo(&mut self, buffer: &mut Buffer) -> Option<Vec<Selection>> {
        let entry = self.redo_stack.pop()?;
        entry.transaction.apply(buffer);
        let selections = entry.transaction.selections_after.clone();
        self.state = entry.after;
        self.undo_stack.push(entry);
        Some(selections)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Identifies the current content state within this History's lifetime.
    /// Undo/redo restores IDs; a new branch never reuses a discarded ID.
    pub fn state_id(&self) -> u64 {
        self.state
    }

    /// Makes the current state reachable by undo even if later typing would
    /// otherwise coalesce across a saved snapshot or explicit command boundary.
    pub fn checkpoint(&mut self) -> u64 {
        if let Some(last) = self.undo_stack.last_mut() {
            last.transaction.chainable = false;
        }
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;

    #[test]
    fn saved_state_survives_undo_redo_coalescing_and_branching() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let mut history = History::new();
        record_insert(&mut history, &mut buffer, 0, "a");
        record_insert(&mut history, &mut buffer, 1, "b");
        let saved = history.checkpoint();
        record_insert(&mut history, &mut buffer, 2, "c");
        let later = history.state_id();
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "ab");
        assert_eq!(history.state_id(), saved);
        history.redo(&mut buffer);
        assert_eq!(history.state_id(), later);
        history.undo(&mut buffer);
        record_insert(&mut history, &mut buffer, 2, "d");
        assert_ne!(history.state_id(), later);
        assert_ne!(history.state_id(), saved);
        assert!(!history.can_redo());
        history.undo(&mut buffer);
        assert_eq!(history.state_id(), saved);
        history.undo(&mut buffer);
        assert_eq!(history.state_id(), 0);
        assert_eq!(buffer.text(), "");
    }

    fn cursor_at(buffer: &Buffer, char_idx: usize) -> Selection {
        Selection::cursor(Position::from_char(buffer, char_idx))
    }

    fn record_insert(history: &mut History, buffer: &mut Buffer, char_idx: usize, text: &str) {
        let before = cursor_at(buffer, char_idx);
        let edit = Edit::insert(char_idx, text);
        edit.apply(buffer);
        let after = cursor_at(buffer, char_idx + text.chars().count());
        history.record(Transaction::single(edit, before, after));
    }

    #[test]
    fn undo_reverts_a_single_insert() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let mut history = History::new();
        record_insert(&mut history, &mut buffer, 0, "hello");

        assert_eq!(buffer.text(), "hello");
        let restored = history.undo(&mut buffer).unwrap();
        assert_eq!(buffer.text(), "");
        assert_eq!(restored, vec![cursor_at(&buffer, 0)]);
    }

    #[test]
    fn redo_reapplies_an_undone_transaction() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let mut history = History::new();
        record_insert(&mut history, &mut buffer, 0, "hi");

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "");
        let restored = history.redo(&mut buffer).unwrap();
        assert_eq!(buffer.text(), "hi");
        assert_eq!(restored, vec![cursor_at(&buffer, 2)]);
    }

    #[test]
    fn recording_a_new_transaction_clears_redo() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let mut history = History::new();
        record_insert(&mut history, &mut buffer, 0, "a");
        history.undo(&mut buffer);
        assert!(history.can_redo());

        record_insert(&mut history, &mut buffer, 0, "b");
        assert!(!history.can_redo());
    }

    #[test]
    fn consecutive_typing_coalesces_into_one_undo_step() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let mut history = History::new();
        record_insert(&mut history, &mut buffer, 0, "h");
        record_insert(&mut history, &mut buffer, 1, "i");
        record_insert(&mut history, &mut buffer, 2, "!");

        assert_eq!(buffer.text(), "hi!");
        let restored = history.undo(&mut buffer).unwrap();
        assert_eq!(buffer.text(), "");
        assert_eq!(restored, vec![cursor_at(&buffer, 0)]);
        assert!(!history.can_undo());
    }

    #[test]
    fn non_adjacent_inserts_do_not_coalesce() {
        let mut buffer = Buffer::from_bytes(b"0123456789").unwrap();
        let mut history = History::new();
        record_insert(&mut history, &mut buffer, 0, "a");
        record_insert(&mut history, &mut buffer, 9, "b"); // not adjacent to the first

        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "a0123456789");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "0123456789");
    }

    #[test]
    fn multi_edit_transactions_never_coalesce_with_anything() {
        let mut buffer = Buffer::from_bytes(b"ab").unwrap();
        let mut history = History::new();
        // A single N-cursor transaction: two inserts applied atomically,
        // back-to-front (rightmost cursor's edit first) so the leftmost
        // edit's char index isn't invalidated by the other.
        let edits = vec![Edit::insert(2, "Y"), Edit::insert(0, "X")];
        for edit in &edits {
            edit.apply(&mut buffer);
        }
        history.record(Transaction::new(edits, vec![], vec![]));
        assert_eq!(buffer.text(), "XabY");

        record_insert(&mut history, &mut buffer, 4, "Z");
        assert_eq!(buffer.text(), "XabYZ");

        history.undo(&mut buffer); // undoes only "Z"
        assert_eq!(buffer.text(), "XabY");
        history.undo(&mut buffer); // undoes the N-cursor transaction as one step
        assert_eq!(buffer.text(), "ab");
    }

    #[test]
    fn backspace_chain_coalesces_and_undoes_as_one_step() {
        let mut buffer = Buffer::from_bytes(b"hello").unwrap();
        let mut history = History::new();
        for char_idx in (0..5).rev() {
            let before = cursor_at(&buffer, char_idx + 1);
            let edit = Edit::delete(&buffer, char_idx, 1);
            edit.apply(&mut buffer);
            let after = cursor_at(&buffer, char_idx);
            history.record(Transaction::single(edit, before, after));
        }

        assert_eq!(buffer.text(), "");
        history.undo(&mut buffer);
        assert_eq!(buffer.text(), "hello");
        assert!(!history.can_undo());
    }
}
