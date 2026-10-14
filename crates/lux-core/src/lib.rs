//! Headless text editing engine. No UI dependencies, ever.

pub mod binary_detect;
pub mod bookmarks;
pub mod buffer;
pub mod cursor;
pub mod edit;
pub mod edit_ops;
pub mod find_replace;
pub mod folding;
pub mod history;
pub mod huge_file;
pub mod movement;
pub mod multicursor;
pub mod position;
pub mod selection;
pub mod selection_ops;
pub mod snippets;
