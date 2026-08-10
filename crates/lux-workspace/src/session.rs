//! Sessions and recovery (Phase 4 deliverable 7): persisting open
//! tabs/cursors/scroll/folders across restarts, and recovering unsaved
//! buffer content after an unexpected close (a crash, a `kill -9`, a
//! power loss) — not just a clean quit. Two related but distinct kinds of
//! state: [`Session`] (where you left off — cheap, rewritten on every real
//! change) and [`RecoveryStore`] (the actual unsaved text, written only
//! for buffers with real unsaved edits, read back only when the previous
//! session didn't end cleanly).
//!
//! Both are caller-driven, the same shape
//! `watcher::FileWatcher::mark_dirty` already established: no buffer or
//! document manager exists yet in this codebase to call `Session::save`
//! or `RecoveryStore::save` automatically on a timer (which would violate
//! golden rule #5 besides), so whichever layer eventually owns open
//! buffers is the one that calls these, on real events (a tab
//! opens/closes, an edit happens, a save completes) — not periodically.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::Workspace;

pub const SESSION_FILE_NAME: &str = ".lux-session";
const RECOVERY_DIR_NAME: &str = ".lux-recovery";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    pub char_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabState {
    pub path: PathBuf,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub cursors: Vec<CursorState>,
    #[serde(default)]
    pub scroll_line: usize,
}

/// Where you left off: open folders, open tabs (each with its cursors and
/// scroll position), and which tab was active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub folders: Vec<PathBuf>,
    #[serde(default)]
    pub tabs: Vec<TabState>,
    #[serde(default)]
    pub active_tab: Option<usize>,
    /// `false` from the moment a session starts until `end_cleanly` writes
    /// it `true` as the very last step of a normal quit. A `Session`
    /// loaded with this still `false` means whatever process wrote it
    /// never reached a clean exit — see `needs_recovery`.
    #[serde(default)]
    clean_shutdown: bool,
}

impl Session {
    pub fn new() -> Self {
        Session {
            folders: Vec::new(),
            tabs: Vec::new(),
            active_tab: None,
            clean_shutdown: false,
        }
    }

    /// Loads the session persisted at `path`, or a fresh empty one if
    /// there's nothing there yet (the common case: opening a project for
    /// the first time). A malformed file is a real `io::Error`, not
    /// silently discarded — losing a corrupt-but-present session's
    /// folders/tabs without telling anyone would be a worse failure mode
    /// than surfacing it.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Session::new());
        }
        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// `true` if the session loaded from disk was left with
    /// `clean_shutdown == false` — the signal to check
    /// `RecoveryStore::recoverable` before trusting on-disk content as the
    /// whole story, since something ended without a normal quit.
    pub fn needs_recovery(&self) -> bool {
        !self.clean_shutdown
    }

    /// Persists the session with `clean_shutdown = false`. Call this right
    /// after `load` (a session is now active) and again after any real
    /// change to tabs/cursors/scroll/folders — on the actual event (tab
    /// open/close/switch, cursor move, scroll, folder add/remove), never
    /// on a timer.
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.write(path, false)
    }

    /// Persists the session with `clean_shutdown = true` — the last call a
    /// normal quit makes, so the *next* `load` sees a clean exit and
    /// `needs_recovery` returns `false`.
    pub fn end_cleanly(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.write(path, true)
    }

    fn write(&self, path: impl AsRef<Path>, clean_shutdown: bool) -> io::Result<()> {
        let mut session = self.clone();
        session.clean_shutdown = clean_shutdown;
        let text = serde_json::to_string_pretty(&session)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        atomic_write(path.as_ref(), text.as_bytes())
    }

    /// Rebuilds a [`Workspace`] from `folders` — the "restore folders on
    /// open" half of this deliverable, composed with `project::Workspace`
    /// (deliverable 1's multi-root model) instead of duplicating it.
    pub fn to_workspace(&self) -> Workspace {
        let mut workspace = Workspace::new();
        for folder in &self.folders {
            workspace.add_root(folder.clone(), None);
        }
        workspace
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// One buffer's unsaved content, as of whenever it was last written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub path: PathBuf,
    pub content: String,
}

/// Stores unsaved-buffer snapshots under `<root>/.lux-recovery/`, one file
/// per path (named by a hash of the path, so arbitrary paths — with
/// slashes, unicode, whatever — never need filename-escaping logic; the
/// real path lives inside the file's own JSON).
pub struct RecoveryStore {
    dir: PathBuf,
}

impl RecoveryStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        RecoveryStore {
            dir: root.into().join(RECOVERY_DIR_NAME),
        }
    }

    /// Writes (or overwrites) `path`'s unsaved content. Call this whenever
    /// a buffer with real unsaved edits changes.
    pub fn save(&self, path: &Path, content: &str) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let snapshot = RecoverySnapshot {
            path: path.to_path_buf(),
            content: content.to_owned(),
        };
        let text = serde_json::to_string(&snapshot)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        atomic_write(&self.snapshot_path(path), text.as_bytes())
    }

    /// Deletes `path`'s recovery snapshot — call after a real save to disk
    /// succeeds (or the user discards the unsaved edits): the snapshot
    /// would otherwise outlive its purpose and falsely offer to "recover"
    /// content that's already safely on disk. Not an error if there was
    /// nothing to clear.
    pub fn clear(&self, path: &Path) -> io::Result<()> {
        match fs::remove_file(self.snapshot_path(path)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Every recovery snapshot currently stored — what a "recover unsaved
    /// changes?" prompt would list after `Session::needs_recovery`
    /// returns `true`. Empty (not an error) if nothing has ever been
    /// saved here.
    pub fn recoverable(&self) -> io::Result<Vec<RecoverySnapshot>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut snapshots = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let text = fs::read_to_string(entry.path())?;
            let snapshot: RecoverySnapshot = serde_json::from_str(&text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            snapshots.push(snapshot);
        }
        Ok(snapshots)
    }

    fn snapshot_path(&self, path: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        self.dir.join(format!("{:016x}.json", hasher.finish()))
    }
}

/// Writes `bytes` to `path` atomically: content goes to a temp file in the
/// same directory, fsync'd, then renamed over the target — the same
/// pattern `lux_core::buffer::Buffer::save` uses, applied here because
/// both `Session` and `RecoveryStore` exist specifically to survive a
/// crash. A plain `fs::write` can be interrupted mid-write by the exact
/// kind of event (power loss, `kill -9`) this module is meant to recover
/// from, leaving a half-written, unparseable file behind — defeating the
/// recovery story `Session::needs_recovery`/`RecoveryStore::recoverable`
/// exist to serve.
fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let tmp_path = dir.join(format!(
        ".{}.lux-tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("session")
    ));

    {
        use std::io::Write;
        let mut tmp = fs::File::create(&tmp_path)?;
        tmp.write_all(bytes)?;
        tmp.sync_all()?;
    }

    fs::rename(&tmp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-workspace-session-test-{:?}-{}",
            std::thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(&dir).unwrap()
    }

    #[test]
    fn load_with_no_file_returns_a_fresh_empty_session() {
        let dir = temp_dir();
        let session = Session::load(dir.join(SESSION_FILE_NAME)).unwrap();
        assert!(session.folders.is_empty());
        assert!(session.tabs.is_empty());
        assert_eq!(session.active_tab, None);
        assert!(session.needs_recovery());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_then_load_round_trips_folders_tabs_and_active_tab() {
        let dir = temp_dir();
        let path = dir.join(SESSION_FILE_NAME);

        let mut session = Session::new();
        session.folders = vec![dir.join("backend"), dir.join("frontend")];
        session.tabs = vec![
            TabState {
                path: dir.join("a.rs"),
                pinned: true,
                cursors: vec![CursorState { char_offset: 42 }],
                scroll_line: 10,
            },
            TabState {
                path: dir.join("b.rs"),
                pinned: false,
                cursors: vec![],
                scroll_line: 0,
            },
        ];
        session.active_tab = Some(1);
        session.save(&path).unwrap();

        let loaded = Session::load(&path).unwrap();
        assert_eq!(loaded.folders, session.folders);
        assert_eq!(loaded.tabs, session.tabs);
        assert_eq!(loaded.active_tab, Some(1));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn needs_recovery_is_true_after_save_but_false_after_end_cleanly() {
        let dir = temp_dir();
        let path = dir.join(SESSION_FILE_NAME);

        Session::new().save(&path).unwrap();
        assert!(Session::load(&path).unwrap().needs_recovery());

        Session::new().end_cleanly(&path).unwrap();
        assert!(!Session::load(&path).unwrap().needs_recovery());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_of_malformed_file_is_an_invalid_data_error() {
        let dir = temp_dir();
        let path = dir.join(SESSION_FILE_NAME);
        fs::write(&path, "not json").unwrap();

        let err = Session::load(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_defaults_missing_fields_from_a_minimal_file() {
        let dir = temp_dir();
        let path = dir.join(SESSION_FILE_NAME);
        fs::write(&path, "{}").unwrap();

        let session = Session::load(&path).unwrap();
        assert!(session.folders.is_empty());
        assert!(session.tabs.is_empty());
        assert!(session.needs_recovery());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn to_workspace_adds_one_root_per_folder() {
        let dir = temp_dir();
        let mut session = Session::new();
        session.folders = vec![dir.join("backend"), dir.join("frontend")];

        let workspace = session.to_workspace();
        assert_eq!(workspace.roots().len(), 2);
        assert!(workspace.is_multi_root());
        assert_eq!(workspace.roots()[0].path, dir.join("backend"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recovery_store_save_then_recoverable_lists_the_snapshot() {
        let dir = temp_dir();
        let store = RecoveryStore::new(&dir);
        let file = dir.join("draft.rs");

        store.save(&file, "unsaved content").unwrap();
        let snapshots = store.recoverable().unwrap();

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].path, file);
        assert_eq!(snapshots[0].content, "unsaved content");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recovery_store_save_overwrites_previous_content_for_the_same_path() {
        let dir = temp_dir();
        let store = RecoveryStore::new(&dir);
        let file = dir.join("draft.rs");

        store.save(&file, "first").unwrap();
        store.save(&file, "second").unwrap();
        let snapshots = store.recoverable().unwrap();

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].content, "second");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recovery_store_handles_multiple_paths_independently() {
        let dir = temp_dir();
        let store = RecoveryStore::new(&dir);
        let a = dir.join("a.rs");
        let b = dir.join("b.rs");

        store.save(&a, "content a").unwrap();
        store.save(&b, "content b").unwrap();
        let mut snapshots = store.recoverable().unwrap();
        snapshots.sort_by(|x, y| x.path.cmp(&y.path));

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].content, "content a");
        assert_eq!(snapshots[1].content, "content b");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recovery_store_clear_removes_the_snapshot() {
        let dir = temp_dir();
        let store = RecoveryStore::new(&dir);
        let file = dir.join("draft.rs");

        store.save(&file, "unsaved").unwrap();
        store.clear(&file).unwrap();

        assert!(store.recoverable().unwrap().is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recovery_store_clear_of_nothing_is_not_an_error() {
        let dir = temp_dir();
        let store = RecoveryStore::new(&dir);
        assert!(store.clear(&dir.join("never-saved.rs")).is_ok());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recovery_store_recoverable_is_empty_when_directory_does_not_exist() {
        let dir = temp_dir();
        let store = RecoveryStore::new(&dir);
        assert!(store.recoverable().unwrap().is_empty());
        fs::remove_dir_all(&dir).ok();
    }
}
