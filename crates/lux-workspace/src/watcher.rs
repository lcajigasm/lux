//! Native file watchers (Phase 4 deliverable 3): `notify` 8.2.0 backed by a
//! real debouncer (`notify-debouncer-full` 0.7.0, the version that pairs
//! with `notify` 8.2.0 — `0.8.0-rc.*` tracks `notify` 9's still-unstable
//! RC, see `docs/DEPENDENCIES.md`). Event-driven end to end: the debouncer
//! thread blocks on the OS's own notification primitive (FSEvents,
//! inotify, `ReadDirectoryChangesW`) and only wakes on a real change —
//! there is no timer anywhere in this module re-checking the filesystem
//! (golden rule #5).
//!
//! The debouncer is also how this handles each backend's own
//! particularities, rather than by hand-rolling separate per-OS branches:
//! inotify emits a `rename`+`write`+`chmod` burst for one atomic editor
//! save, FSEvents coalesces bursts of its own with different latency
//! characteristics, and Windows' `ReadDirectoryChangesW` has yet another
//! shape — `notify-debouncer-full` exists specifically to normalize that
//! raw per-backend noise into one clean event per path within a window,
//! which is a real, load-bearing dependency choice, not a placeholder.
//! Linux's `inotify` watch-count limit is a separate, real constraint:
//! `Watcher::watch` surfaces it as `notify::ErrorKind::MaxFilesWatch`
//! rather than a generic error, so a caller can tell "add this folder to
//! your project's excludes" apart from "something else went wrong" —
//! see [`is_watch_limit_reached`].
//!
//! The other half of the deliverable — reload vs. conflict when a watched
//! file changes on disk — needs to know whether that file has unsaved
//! local edits. No buffer/document manager exists yet to ask (that's a
//! future editor session's territory, not this crate's), so this module
//! tracks it itself via [`FileWatcher::mark_dirty`]/[`FileWatcher::mark_clean`],
//! the two calls whichever layer eventually owns open buffers will drive.
//!
//! One more FSEvents particularity, found while testing this against the
//! real backend rather than assuming the docs: on macOS, editing a file
//! that already existed before `watch()` was called is reported as another
//! `Create`, not a `Modify` — confirmed to persist well past any single
//! debounce window, not just a first-edit timing fluke (`notify`'s own
//! "known problems" apply, per `notify-debouncer-full`'s module docs).
//! Trusting the raw event kind alone would misfile a real external edit to
//! a dirty file as a plain `Created` instead of a `Conflict`. `watch()`
//! fixes this by snapshotting which files already exist under the root
//! into `known` before returning; `classify` then treats a `Create` for an
//! already-known path the same as a `Modify`.
//!
//! A second, sharper FSEvents particularity, also found by testing against
//! the real backend: deleting a file shortly after it's created (even
//! across separate, real syscalls — this isn't just a same-instant race)
//! makes FSEvents coalesce that whole lifecycle into one raw event batch
//! with every flag it ever saw for that path set at once. `notify` itself
//! still reports a correct `Remove` in that batch, but
//! `notify-debouncer-full` has its own "a `Create` immediately followed by
//! a `Remove` cancels out to nothing" dedup rule, and applies it here even
//! though the create and remove were genuinely separate operations — so
//! the real `Remove` gets silently swallowed, leaving only the batch's
//! straggling `Modify` events to reach this module. Trusting `event.kind`
//! there would report a deleted file as merely modified. `classify` closes
//! this by treating any event whose path no longer exists on disk as
//! `Removed`, regardless of what kind the debouncer attached to it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{
    DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache, new_debouncer,
};

/// `true` if `err` is `notify` refusing to add another watch because the
/// OS-level limit was reached (Linux's `fs.inotify.max_user_watches`, most
/// commonly — see the module docs).
pub fn is_watch_limit_reached(err: &notify::Error) -> bool {
    matches!(err.kind, notify::ErrorKind::MaxFilesWatch)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Created,
    Removed,
    /// Modified on disk with no locally tracked unsaved edits — safe to
    /// silently reload.
    Reload,
    /// Modified on disk while [`FileWatcher::mark_dirty`] considers it
    /// unsaved locally — must not be silently clobbered.
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: PathBuf,
    pub kind: ChangeKind,
}

/// One drained batch: classified file changes, plus any watcher-level
/// errors from that same batch (kept separate from `changes` since they
/// aren't necessarily about one path).
#[derive(Debug, Default)]
pub struct WatchBatch {
    pub changes: Vec<FileChange>,
    pub errors: Vec<String>,
}

pub struct FileWatcher {
    debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    events: Receiver<DebounceEventResult>,
    dirty: HashSet<PathBuf>,
    /// Files known to exist as of the last `watch()` call (or since
    /// discovered via a `Create`/`Modify` event) — how `classify`
    /// distinguishes a genuinely new file from macOS's spurious
    /// re-`Create` on an existing file's first edit (see module docs).
    known: HashSet<PathBuf>,
}

impl FileWatcher {
    /// Starts a debounced watcher. `debounce` is the coalescing window —
    /// how long `notify-debouncer-full` waits for a burst of raw events on
    /// the same path to settle before emitting one.
    pub fn new(debounce: Duration) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let debouncer = new_debouncer(debounce, None, tx)?;
        Ok(FileWatcher {
            debouncer,
            events: rx,
            dirty: HashSet::new(),
            known: HashSet::new(),
        })
    }

    /// Watches `root` and everything below it. See [`is_watch_limit_reached`]
    /// for the one error worth telling apart from the rest.
    ///
    /// `root` is canonicalized first: macOS's FSEvents backend always
    /// reports paths resolved through symlinks (e.g. `/tmp` -> `/private/
    /// tmp`), so watching the given, possibly-unresolved path would mean
    /// every reported event's path fails to string-match anything this
    /// module (or a caller's dirty-tracking) compares it against.
    /// `mark_dirty`/`mark_clean`/`is_dirty` normalize the same way so both
    /// sides agree. Also does one synchronous recursive read of `root` to
    /// seed `known` (see module docs) — real I/O, so whichever layer wires
    /// this into the running app must call it off the UI thread (golden
    /// rule #6), same caveat already noted for `lux-ui::file_tree`.
    pub fn watch(&mut self, root: impl AsRef<Path>) -> notify::Result<()> {
        let root = normalize(root.as_ref());
        self.debouncer.watch(&root, RecursiveMode::Recursive)?;
        scan_known(&root, &mut self.known);
        Ok(())
    }

    pub fn unwatch(&mut self, root: impl AsRef<Path>) -> notify::Result<()> {
        self.debouncer.unwatch(normalize(root.as_ref()))
    }

    pub fn mark_dirty(&mut self, path: impl AsRef<Path>) {
        self.dirty.insert(normalize(path.as_ref()));
    }

    pub fn mark_clean(&mut self, path: impl AsRef<Path>) {
        self.dirty.remove(&normalize(path.as_ref()));
    }

    pub fn is_dirty(&self, path: impl AsRef<Path>) -> bool {
        self.dirty.contains(&normalize(path.as_ref()))
    }

    /// Blocks up to `timeout` for the next debounced batch of real
    /// filesystem changes, classified against tracked dirty state. This
    /// call itself never re-checks the filesystem on a timer — it's a
    /// bounded wait on a channel a background OS-notification thread
    /// pushes into; `timeout` only bounds how long the *caller* waits, the
    /// same way any blocking-with-timeout channel read works. Empty if
    /// nothing arrives in time.
    pub fn recv(&mut self, timeout: Duration) -> WatchBatch {
        let mut batch = WatchBatch::default();
        match self.events.recv_timeout(timeout) {
            Ok(result) => self.absorb(result, &mut batch),
            Err(_) => return batch,
        }
        self.drain_into(&mut batch);
        batch
    }

    /// Non-blocking: whatever debounced batches have already arrived.
    pub fn try_recv(&mut self) -> WatchBatch {
        let mut batch = WatchBatch::default();
        self.drain_into(&mut batch);
        batch
    }

    fn drain_into(&mut self, batch: &mut WatchBatch) {
        while let Ok(result) = self.events.try_recv() {
            self.absorb(result, batch);
        }
    }

    fn absorb(&mut self, result: DebounceEventResult, batch: &mut WatchBatch) {
        match result {
            Ok(events) => {
                for event in &events {
                    self.classify_into(event, &mut batch.changes);
                }
            }
            Err(errors) => {
                batch
                    .errors
                    .extend(errors.into_iter().map(|e| e.to_string()));
            }
        }
    }

    fn classify_into(&mut self, event: &DebouncedEvent, changes: &mut Vec<FileChange>) {
        let is_create = event.kind.is_create();
        let is_modify = event.kind.is_modify();
        let is_remove = event.kind.is_remove();
        if !is_create && !is_modify && !is_remove {
            // Access/Other/Any: not a content change worth surfacing.
            return;
        }
        for path in &event.paths {
            // A real `Remove`, or a path that no longer exists no matter
            // what kind the debouncer reported — macOS's other real
            // particularity (see module docs): a fast create-then-remove
            // makes FSEvents coalesce the whole lifecycle into one raw
            // batch, which trips `notify-debouncer-full`'s own "cancel out
            // a create immediately followed by a remove" logic and
            // silently drops the real `Remove`, leaving only the stray
            // `Modify` events that were queued after it. Trusting
            // `event.kind` alone would then misreport a deleted file as
            // still there.
            if is_remove || !path.exists() {
                self.known.remove(path);
                changes.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Removed,
                });
                continue;
            }
            if is_create && !self.known.contains(path) {
                self.known.insert(path.clone());
                changes.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Created,
                });
                continue;
            }
            // Either a real `Modify`, or a `Create` for a path already in
            // `known` — macOS's own particularity described in the module
            // docs. Both mean the same thing from here: the file already
            // existed, so classify by dirty state instead.
            self.known.insert(path.clone());
            let kind = if self.dirty.contains(path) {
                ChangeKind::Conflict
            } else {
                ChangeKind::Reload
            };
            changes.push(FileChange {
                path: path.clone(),
                kind,
            });
        }
    }
}

/// Resolves symlinks so paths compare consistently across `watch`,
/// `mark_dirty`/`mark_clean`/`is_dirty`, and the paths backends actually
/// report (see `watch`'s doc comment). Best-effort: a path that no longer
/// exists (e.g. classifying a `Removed` event, or a caller marking a file
/// dirty right as it's deleted out from under it) falls back to the
/// original, unresolved path rather than failing.
fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Recursively records every regular file under `dir` into `known` — see
/// `watch`'s doc comment for why. Permissive like `lux_ui::file_tree`'s own
/// directory reads: an unreadable entry or subdirectory is skipped, not a
/// hard failure for the whole scan.
fn scan_known(dir: &Path, known: &mut HashSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        // `entry.file_type()` (unlike `Path::is_dir()`) doesn't follow
        // symlinks, so a symlink is always treated as a leaf here and
        // never recursed into — a symlink cycle inside the watched root
        // would otherwise send this into unbounded recursion.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            scan_known(&path, known);
        } else {
            known.insert(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;

    const DEBOUNCE: Duration = Duration::from_millis(150);
    const RECV_TIMEOUT: Duration = Duration::from_secs(5);
    // How long a settle wait blocks for before deciding nothing more is
    // coming — see `drain_until_quiet`/`settle`.
    const SETTLE_TIMEOUT: Duration = Duration::from_millis(2000);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-workspace-watcher-test-{:?}-{}",
            thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        // Canonicalized so paths built from this (`dir.join(...)`) compare
        // equal to what `FileWatcher` reports — see `normalize`'s doc
        // comment for why that's not automatic on macOS.
        fs::canonicalize(&dir).unwrap()
    }

    /// Drains whatever batches keep arriving until a full `recv` call times
    /// out with nothing new — i.e. the channel has gone quiet. `watch()`'s
    /// historical-replay burst (FSEvents (macOS) and other native backends
    /// report at least "this directory exists" right after `watch()`
    /// returns) and `unwatch()`'s OS-level teardown both have latency that
    /// varies a lot by machine (a loaded CI runner is not a quiet laptop);
    /// waiting for actual quiescence instead of guessing a fixed sleep
    /// duration is what makes these tests reliable across both. Bounded by
    /// `SETTLE_TIMEOUT` per `recv` call, not a polling loop — each call
    /// blocks on the same real, event-driven channel every other test uses.
    fn settle(watcher: &mut FileWatcher) {
        loop {
            let batch = watcher.recv(SETTLE_TIMEOUT);
            if batch.changes.is_empty() && batch.errors.is_empty() {
                break;
            }
        }
    }

    fn watch_dir(dir: &Path) -> FileWatcher {
        let mut watcher = FileWatcher::new(DEBOUNCE).unwrap();
        watcher.watch(dir).unwrap();
        settle(&mut watcher);
        watcher
    }

    #[test]
    #[cfg(unix)]
    fn scan_known_does_not_recurse_into_a_symlink_cycle() {
        let dir = temp_dir();
        let sub = dir.join("sub");
        fs::create_dir(&sub).unwrap();
        // A symlink inside `sub` pointing back at `dir` — a real cycle a
        // naive `Path::is_dir()`-following recursion would loop on forever.
        std::os::unix::fs::symlink(&dir, sub.join("back_to_root")).unwrap();

        let mut known = HashSet::new();
        scan_known(&dir, &mut known);
        // Terminates at all is the actual assertion; the symlink itself is
        // recorded as a leaf, not descended into.
        assert!(known.contains(&sub.join("back_to_root")));

        fs::remove_dir_all(&dir).ok();
    }

    fn wait_for(
        watcher: &mut FileWatcher,
        mut matches: impl FnMut(&FileChange) -> bool,
    ) -> FileChange {
        let deadline = std::time::Instant::now() + RECV_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out waiting for the expected change"
            );
            let batch = watcher.recv(remaining);
            if let Some(change) = batch.changes.into_iter().find(|c| matches(c)) {
                return change;
            }
        }
    }

    #[test]
    fn detects_a_created_file() {
        let dir = temp_dir();
        let mut watcher = watch_dir(&dir);

        let file = dir.join("new.txt");
        fs::write(&file, "hi").unwrap();

        let change = wait_for(&mut watcher, |c| c.path == file);
        assert_eq!(change.kind, ChangeKind::Created);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detects_a_removed_file() {
        let dir = temp_dir();
        let file = dir.join("gone.txt");
        fs::write(&file, "hi").unwrap();
        let mut watcher = watch_dir(&dir);

        fs::remove_file(&file).unwrap();

        let change = wait_for(&mut watcher, |c| {
            c.path == file && c.kind == ChangeKind::Removed
        });
        assert_eq!(change.kind, ChangeKind::Removed);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn modifying_a_clean_file_reports_reload() {
        let dir = temp_dir();
        let file = dir.join("clean.txt");
        fs::write(&file, "before").unwrap();
        let mut watcher = watch_dir(&dir);

        fs::write(&file, "after").unwrap();

        let change = wait_for(&mut watcher, |c| {
            c.path == file && c.kind == ChangeKind::Reload
        });
        assert_eq!(change.kind, ChangeKind::Reload);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn modifying_a_dirty_file_reports_conflict() {
        let dir = temp_dir();
        let file = dir.join("dirty.txt");
        fs::write(&file, "before").unwrap();
        let mut watcher = watch_dir(&dir);
        watcher.mark_dirty(file.clone());

        fs::write(&file, "changed externally").unwrap();

        let change = wait_for(&mut watcher, |c| {
            c.path == file && c.kind == ChangeKind::Conflict
        });
        assert_eq!(change.kind, ChangeKind::Conflict);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mark_clean_reverts_to_reload_classification() {
        let dir = temp_dir();
        let file = dir.join("was_dirty.txt");
        fs::write(&file, "before").unwrap();
        let mut watcher = watch_dir(&dir);
        watcher.mark_dirty(file.clone());
        assert!(watcher.is_dirty(&file));

        watcher.mark_clean(&file);
        assert!(!watcher.is_dirty(&file));

        fs::write(&file, "after").unwrap();
        let change = wait_for(&mut watcher, |c| {
            c.path == file && c.kind == ChangeKind::Reload
        });
        assert_eq!(change.kind, ChangeKind::Reload);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn try_recv_is_non_blocking_and_empty_with_no_changes() {
        let dir = temp_dir();
        let mut watcher = watch_dir(&dir);
        let batch = watcher.try_recv();
        assert!(batch.changes.is_empty());
        assert!(batch.errors.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recv_returns_empty_batch_on_timeout_with_no_changes() {
        let dir = temp_dir();
        let mut watcher = watch_dir(&dir);
        let batch = watcher.recv(Duration::from_millis(200));
        assert!(batch.changes.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unwatch_stops_reporting_changes_from_that_root() {
        let dir = temp_dir();
        let mut watcher = watch_dir(&dir);
        watcher.unwatch(&dir).unwrap();
        // `unwatch` returning doesn't guarantee the OS has actually torn
        // down the underlying watch stream yet — settle the same way
        // `watch_dir` does, so a write made right after this doesn't race
        // a not-yet-finished teardown on a slower machine.
        settle(&mut watcher);

        fs::write(dir.join("after_unwatch.txt"), "hi").unwrap();

        let batch = watcher.recv(Duration::from_millis(500));
        assert!(batch.changes.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn is_watch_limit_reached_identifies_the_max_files_watch_error() {
        let limit_err = notify::Error::new(notify::ErrorKind::MaxFilesWatch);
        assert!(is_watch_limit_reached(&limit_err));

        let other_err = notify::Error::new(notify::ErrorKind::WatchNotFound);
        assert!(!is_watch_limit_reached(&other_err));
    }

    #[test]
    fn dirty_tracking_is_independent_per_path() {
        let dir = temp_dir();
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let mut watcher = watch_dir(&dir);

        watcher.mark_dirty(a.clone());
        assert!(watcher.is_dirty(&a));
        assert!(!watcher.is_dirty(&b));

        fs::remove_dir_all(&dir).ok();
    }
}
