//! Project folders, file watchers, and session persistence.

pub mod index;
pub mod project;
pub mod session;
pub mod watcher;

pub use index::{IndexDelta, WorkspaceIndex};
pub use project::{FolderEntry, PROJECT_FILE_NAME, ProjectFile, Root, RootId, Workspace};
pub use session::{
    CursorState, RecoverySnapshot, RecoveryStore, SESSION_FILE_NAME, Session, TabState,
};
pub use watcher::{ChangeKind, FileChange, FileWatcher, WatchBatch, is_watch_limit_reached};
