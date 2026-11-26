//! Memory the editor keeps about its carets beyond their current positions.

use crate::input::clipboard::ClipboardMemory;

/// What the carets remember between commands: the column each caret aims
/// for on vertical motion, and the per-caret pieces of the last copy.
pub(crate) struct CaretMemory {
    pub(crate) goal_columns: Vec<usize>,
    pub(crate) clipboard: ClipboardMemory,
}

impl Default for CaretMemory {
    fn default() -> Self {
        Self {
            goal_columns: vec![0],
            clipboard: ClipboardMemory::default(),
        }
    }
}
