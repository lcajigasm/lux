//! Project folders, file watchers, and session persistence.

pub mod project;

pub use project::{FolderEntry, PROJECT_FILE_NAME, ProjectFile, Root, RootId, Workspace};
