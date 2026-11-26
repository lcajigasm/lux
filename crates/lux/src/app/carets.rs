//! Memory the editor keeps about its carets beyond their current positions.

use crate::input::clipboard::ClipboardMemory;

pub(crate) mod soft_undo;

/// What the carets remember between commands: the column each caret aims
/// for on vertical motion, the per-caret pieces of the last copy, and
/// where they have been, for soft undo.
pub(crate) struct CaretMemory {
    pub(crate) goal_columns: Vec<usize>,
    pub(crate) clipboard: ClipboardMemory,
    pub(crate) soft_undo: soft_undo::SoftUndo,
}

impl Default for CaretMemory {
    fn default() -> Self {
        Self {
            goal_columns: vec![0],
            clipboard: ClipboardMemory::default(),
            soft_undo: soft_undo::SoftUndo::default(),
        }
    }
}
