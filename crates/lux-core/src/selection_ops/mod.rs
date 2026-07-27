//! Sublime-style selection commands: expand selection to word, select next
//! occurrence (`Cmd+D`), select all occurrences, skip the current
//! occurrence, column/box selection, and split selection into lines. All
//! of these read a `Buffer` and a `Selection`/`MultiCursor` and produce a
//! new `MultiCursor` — none of them mutate the buffer (that's
//! `edit_ops`'s job).

mod box_select;
mod occurrence;
mod split;
mod word;

pub use box_select::box_select;
pub use occurrence::{select_all_occurrences, select_next_occurrence, skip_current_occurrence};
pub use split::split_into_lines;
pub use word::expand_to_word;
