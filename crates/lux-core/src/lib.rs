//! Headless text editing engine. No UI dependencies, ever.

pub mod buffer;
pub mod cursor;
pub mod edit;
pub mod edit_ops;
pub mod history;
pub mod multicursor;
pub mod position;
pub mod selection;
