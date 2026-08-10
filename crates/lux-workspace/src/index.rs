//! Incremental workspace index (Phase 4 deliverable 6): the current set of
//! files under one root, established by exactly one real recursive scan
//! and kept in sync afterward purely by applying [`FileChange`]s a
//! [`FileWatcher`](crate::watcher::FileWatcher) observed — never a
//! periodic rescan (golden rule #5). [`WorkspaceIndex`] doesn't own or
//! poll a watcher itself; [`WorkspaceIndex::apply_changes`] is a plain
//! function over whatever batch of changes the caller drained, the same
//! "consume another module's already-produced output" shape
//! `lux_search::replace_preview::build_preview` already uses for
//! `find_in_files`'s output.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::watcher::{ChangeKind, FileChange};

/// How many files an [`WorkspaceIndex::apply_changes`] call actually added
/// or removed — `0`/`0` means nothing in the batch changed which files
/// exist (e.g. it was all [`ChangeKind::Conflict`]/[`ChangeKind::Reload`]
/// on already-indexed files).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexDelta {
    pub added: usize,
    pub removed: usize,
}

/// The live file set for one workspace root.
#[derive(Debug, Clone)]
pub struct WorkspaceIndex {
    root: PathBuf,
    files: HashSet<PathBuf>,
}

impl WorkspaceIndex {
    /// Builds the index with one real, `.gitignore`-aware recursive scan
    /// of `root` — the only full walk this type ever does. `require_git
    /// (false)` for the same reason `lux_search::find_in_files` sets it:
    /// a project folder opened in an editor should get its `.gitignore`
    /// honored even if it isn't (yet, or ever) an actual git repository.
    /// Every update after this call goes through `apply_changes`.
    pub fn build(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let mut files = HashSet::new();
        for entry in WalkBuilder::new(&root).require_git(false).build() {
            let entry = entry.map_err(io::Error::other)?;
            if entry.file_type().is_some_and(|t| t.is_file()) {
                files.insert(entry.path().to_path_buf());
            }
        }
        Ok(WorkspaceIndex { root, files })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.files.iter().map(PathBuf::as_path)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.files.contains(path)
    }

    /// Applies a batch of watcher-observed changes in place. This is the
    /// entire update path after `build`: no re-walk, no re-stat of the
    /// whole tree, just the paths this specific batch names.
    ///
    /// `Created`/`Reload` insert `path` if it's currently a regular file
    /// (a directory create/reload — e.g. `mkdir` — isn't indexed; nor is a
    /// path that no longer exists by the time this runs, a real possible
    /// race between the watcher observing the event and this call). A
    /// `Removed` change drops `path` unconditionally — removing a path
    /// that was never indexed (it was a directory, or already gone) is a
    /// harmless no-op on the underlying set, so there's no need to know in
    /// advance whether it used to be a file. `Conflict` never changes
    /// membership: the file still exists, only its content did.
    pub fn apply_changes(&mut self, changes: &[FileChange]) -> IndexDelta {
        let mut delta = IndexDelta::default();
        for change in changes {
            match change.kind {
                ChangeKind::Created | ChangeKind::Reload => {
                    if change.path.is_file() && self.files.insert(change.path.clone()) {
                        delta.added += 1;
                    }
                }
                ChangeKind::Removed => {
                    if self.files.remove(&change.path) {
                        delta.removed += 1;
                    }
                }
                ChangeKind::Conflict => {}
            }
        }
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::FileWatcher;
    use std::fs;
    use std::time::Duration;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-workspace-index-test-{:?}-{}",
            std::thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(&dir).unwrap()
    }

    fn change(path: &Path, kind: ChangeKind) -> FileChange {
        FileChange {
            path: path.to_path_buf(),
            kind,
        }
    }

    #[test]
    fn build_indexes_every_file_under_the_root() {
        let dir = temp_dir();
        fs::write(dir.join("a.rs"), "").unwrap();
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/b.rs"), "").unwrap();

        let index = WorkspaceIndex::build(&dir).unwrap();
        assert_eq!(index.len(), 2);
        assert!(index.contains(&dir.join("a.rs")));
        assert!(index.contains(&dir.join("sub/b.rs")));
        assert_eq!(index.root(), dir);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_does_not_index_directories_themselves() {
        let dir = temp_dir();
        fs::create_dir(dir.join("sub")).unwrap();

        let index = WorkspaceIndex::build(&dir).unwrap();
        assert!(index.is_empty());
        assert!(!index.contains(&dir.join("sub")));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_respects_gitignore() {
        let dir = temp_dir();
        fs::write(dir.join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(dir.join("ignored.rs"), "").unwrap();
        fs::write(dir.join("kept.rs"), "").unwrap();

        let index = WorkspaceIndex::build(&dir).unwrap();
        assert_eq!(index.len(), 1);
        assert!(index.contains(&dir.join("kept.rs")));
        assert!(!index.contains(&dir.join("ignored.rs")));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_changes_created_adds_a_real_file() {
        let dir = temp_dir();
        let mut index = WorkspaceIndex::build(&dir).unwrap();
        assert!(index.is_empty());

        let new_file = dir.join("new.rs");
        fs::write(&new_file, "").unwrap();
        let delta = index.apply_changes(&[change(&new_file, ChangeKind::Created)]);

        assert_eq!(
            delta,
            IndexDelta {
                added: 1,
                removed: 0
            }
        );
        assert!(index.contains(&new_file));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_changes_created_for_a_path_that_is_not_a_file_is_a_no_op() {
        let dir = temp_dir();
        let mut index = WorkspaceIndex::build(&dir).unwrap();

        let sub = dir.join("sub");
        fs::create_dir(&sub).unwrap();
        let delta = index.apply_changes(&[change(&sub, ChangeKind::Created)]);

        assert_eq!(delta, IndexDelta::default());
        assert!(!index.contains(&sub));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_changes_removed_drops_an_indexed_file() {
        let dir = temp_dir();
        let file = dir.join("gone.rs");
        fs::write(&file, "").unwrap();
        let mut index = WorkspaceIndex::build(&dir).unwrap();
        assert!(index.contains(&file));

        fs::remove_file(&file).unwrap();
        let delta = index.apply_changes(&[change(&file, ChangeKind::Removed)]);

        assert_eq!(
            delta,
            IndexDelta {
                added: 0,
                removed: 1
            }
        );
        assert!(!index.contains(&file));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_changes_removed_for_an_untracked_path_is_a_no_op() {
        let dir = temp_dir();
        let mut index = WorkspaceIndex::build(&dir).unwrap();

        let delta =
            index.apply_changes(&[change(&dir.join("never-indexed.rs"), ChangeKind::Removed)]);
        assert_eq!(delta, IndexDelta::default());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_changes_reload_and_conflict_never_change_membership() {
        let dir = temp_dir();
        let file = dir.join("a.rs");
        fs::write(&file, "one").unwrap();
        let mut index = WorkspaceIndex::build(&dir).unwrap();
        assert_eq!(index.len(), 1);

        let delta = index.apply_changes(&[
            change(&file, ChangeKind::Reload),
            change(&file, ChangeKind::Conflict),
        ]);
        // `Reload` on an already-indexed file re-inserts the same path —
        // `HashSet::insert` returns `false` for an existing member, so
        // this correctly reports zero *new* additions.
        assert_eq!(delta, IndexDelta::default());
        assert_eq!(index.len(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_changes_processes_a_batch_in_one_call() {
        let dir = temp_dir();
        let keep = dir.join("keep.rs");
        let dropped = dir.join("dropped.rs");
        fs::write(&dropped, "").unwrap();
        let mut index = WorkspaceIndex::build(&dir).unwrap();
        assert!(index.contains(&dropped));

        fs::write(&keep, "").unwrap();
        fs::remove_file(&dropped).unwrap();
        let delta = index.apply_changes(&[
            change(&keep, ChangeKind::Created),
            change(&dropped, ChangeKind::Removed),
        ]);

        assert_eq!(
            delta,
            IndexDelta {
                added: 1,
                removed: 1
            }
        );
        assert!(index.contains(&keep));
        assert!(!index.contains(&dropped));

        fs::remove_dir_all(&dir).ok();
    }

    /// End-to-end: a real `FileWatcher` observes real filesystem changes,
    /// and its actual output — not a synthetic `FileChange` built by a
    /// test — keeps the index correct. This is what "fed by watchers"
    /// means for real, not just "the types line up."
    #[test]
    fn index_stays_correct_when_fed_by_a_real_watcher() {
        let dir = temp_dir();
        let mut index = WorkspaceIndex::build(&dir).unwrap();
        assert!(index.is_empty());

        let mut watcher = FileWatcher::new(Duration::from_millis(150)).unwrap();
        watcher.watch(&dir).unwrap();
        // Let the native watcher backend arm before generating changes —
        // same real-backend caveat `watcher.rs`'s own tests document.
        std::thread::sleep(Duration::from_millis(300));

        let added = dir.join("added_by_watcher.rs");
        fs::write(&added, "hello").unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for the watcher to observe the new file"
            );
            let batch = watcher.recv(Duration::from_millis(500));
            index.apply_changes(&batch.changes);
            if index.contains(&added) {
                break;
            }
        }

        fs::remove_dir_all(&dir).ok();
    }
}
