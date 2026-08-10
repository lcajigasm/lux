//! Project folders, file watchers, and session persistence.

pub mod project;
pub mod watcher;

pub use project::{FolderEntry, PROJECT_FILE_NAME, ProjectFile, Root, RootId, Workspace};
pub use watcher::{ChangeKind, FileChange, FileWatcher, WatchBatch, is_watch_limit_reached};
