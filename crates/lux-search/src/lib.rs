//! Find-in-files and fuzzy matching (`docs/ARCHITECTURE.md`). No UI, no
//! rendering.

pub mod find_in_files;
pub mod fuzzy;

pub use find_in_files::{SearchError, SearchMatch, SearchOptions, SearchStream, find_in_files};
