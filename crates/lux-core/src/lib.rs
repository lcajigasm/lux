//! Headless text editing engine. No UI dependencies, ever.

pub mod buffer;
pub mod cursor;
pub mod edit;
pub mod edit_ops;
pub mod find_replace;
pub mod history;
pub mod multicursor;
pub mod position;
pub mod selection;
pub mod selection_ops;
pub mod snippets;
