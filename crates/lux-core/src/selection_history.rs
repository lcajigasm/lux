//! Selection history for Soft Undo / Soft Redo (Sublime's `Cmd+U`).
//!
//! Soft undo steps back through where the carets and selections were
//! without touching the text. The owner reports the current state with
//! [`SelectionHistory::settle`] before handling each input; when the
//! selections differ from the last settled state, that state becomes one
//! undo step. Text edits do not create steps: [`SelectionHistory::after_edit`]
//! moves every remembered selection through the edits instead, so a step
//! still points at the same text after typing elsewhere.

use std::collections::VecDeque;

use crate::edit::Edit;

/// How many soft-undo steps are kept per document.
pub const CAPACITY: usize = 128;

/// Where the carets were, plus the view's scroll offset at that moment.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionSnapshot {
    /// `(anchor, head)` char offsets in document order, except that the
    /// primary selection is last.
    pub selections: Vec<(usize, usize)>,
    pub scroll: (f32, f32),
}

impl SelectionSnapshot {
    fn same_selections(&self, other: &SelectionSnapshot) -> bool {
        self.selections == other.selections
    }

    fn map(&mut self, edit: &Edit) {
        for (anchor, head) in &mut self.selections {
            *anchor = map_offset(*anchor, edit);
            *head = map_offset(*head, edit);
        }
    }
}

/// Where `offset` lands after `edit`. Text inserted at the offset pushes
/// it forward; an offset inside deleted text moves to the deletion point.
fn map_offset(offset: usize, edit: &Edit) -> usize {
    match edit {
        Edit::Insert { char_idx, text } if offset >= *char_idx => offset + text.chars().count(),
        Edit::Delete { char_idx, text } if offset > *char_idx => {
            let end = char_idx + text.chars().count();
            if offset >= end {
                offset - (end - char_idx)
            } else {
                *char_idx
            }
        }
        _ => offset,
    }
}

#[derive(Debug, Default)]
pub struct SelectionHistory {
    undo: VecDeque<SelectionSnapshot>,
    redo: Vec<SelectionSnapshot>,
    settled: Option<SelectionSnapshot>,
    /// The owner's text version `settled` belongs to.
    version: u64,
}

impl SelectionHistory {
    pub fn new() -> SelectionHistory {
        SelectionHistory::default()
    }

    /// Records `current` as the latest state of a document at text
    /// `version`. A change of selections since the last call becomes an
    /// undo step and clears the redo steps. A different `version` means
    /// the text changed without [`SelectionHistory::after_edit`], so every
    /// remembered position is unreliable and the history starts over.
    pub fn settle(&mut self, current: SelectionSnapshot, version: u64) {
        if version != self.version {
            self.reset(current, version);
            return;
        }
        match self.settled.take() {
            Some(previous) if !previous.same_selections(&current) => {
                if self.undo.len() == CAPACITY {
                    self.undo.pop_front();
                }
                self.undo.push_back(previous);
                self.redo.clear();
            }
            _ => {}
        }
        self.settled = Some(current);
    }

    /// Moves every remembered selection through `edits` (in the order they
    /// were applied) and takes `current` as the settled state at `version`
    /// without adding a step.
    pub fn after_edit(&mut self, edits: &[Edit], current: SelectionSnapshot, version: u64) {
        for edit in edits {
            for snapshot in self.undo.iter_mut().chain(self.redo.iter_mut()) {
                snapshot.map(edit);
            }
        }
        self.settled = Some(current);
        self.version = version;
    }

    /// Steps back: returns the previous state and remembers `current` for
    /// soft redo. `current` must already be settled.
    pub fn soft_undo(&mut self, current: SelectionSnapshot) -> Option<SelectionSnapshot> {
        let previous = self.undo.pop_back()?;
        self.redo.push(current);
        self.settled = Some(previous.clone());
        Some(previous)
    }

    /// Steps forward again after [`SelectionHistory::soft_undo`].
    pub fn soft_redo(&mut self, current: SelectionSnapshot) -> Option<SelectionSnapshot> {
        let next = self.redo.pop()?;
        self.undo.push_back(current);
        self.settled = Some(next.clone());
        Some(next)
    }

    pub fn can_soft_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_soft_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn reset(&mut self, current: SelectionSnapshot, version: u64) {
        self.undo.clear();
        self.redo.clear();
        self.settled = Some(current);
        self.version = version;
    }
}

#[cfg(test)]
#[path = "selection_history_tests.rs"]
mod tests;
