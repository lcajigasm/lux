//! Project folders, file watchers, and session persistence.

pub mod index;
pub mod placement;
pub mod project;
pub mod session;
pub mod session_store;
pub mod symbol_worker;
pub mod symbols;
pub mod watcher;

pub use index::{IndexDelta, WorkspaceIndex};
pub use placement::{MonitorRect, PlacedWindow, WindowFrame, restore_window};
pub use project::{FolderEntry, PROJECT_FILE_NAME, ProjectFile, Root, RootId, Workspace};
pub use session::{
    CursorState, RecoverySnapshot, RecoveryStore, SESSION_FILE_NAME, Session, TabState,
};
pub use session_store::{LayoutState, PaneState};
pub use symbol_worker::{SymbolEntry, SymbolIndexer, SymbolSnapshot};
pub use symbols::{SymbolIndex, WorkspaceSymbol};
pub use watcher::{ChangeKind, FileChange, FileWatcher, WatchBatch, is_watch_limit_reached};
