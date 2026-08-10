//! Find-in-files and fuzzy matching (`docs/ARCHITECTURE.md`). No UI, no
//! rendering.

pub mod find_in_files;
pub mod fuzzy;
pub mod replace_preview;

pub use find_in_files::{SearchError, SearchMatch, SearchOptions, SearchStream, find_in_files};
pub use replace_preview::{
    FileApplyOutcome, FileApplyResult, HunkId, ReplacementHunk, ReplacementPreview, build_preview,
};
