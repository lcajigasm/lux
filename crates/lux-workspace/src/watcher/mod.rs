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
//!
//! Every `stat` this module needs happens on the debouncer's own thread,
//! before the batch is sent: whether a reported path still exists and
//! whether it is a directory. Draining a batch on the UI thread
//! ([`FileWatcher::try_recv`]) is memory only. [`FileWatcher::watch_detached`]
//! adds a root and runs its seeding scan on a worker thread as well, and
//! [`FileWatcher::unwatch_detached`] drops one there. Both go through one
//! worker per watcher, in the order they were asked, so a folder closed
//! right after it was added ends up unwatched.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
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
    /// Whether `path` was a directory when the debouncer thread looked at
    /// it. Always `false` for `Removed`.
    pub is_dir: bool,
}

type Wake = Arc<dyn Fn() + Send + Sync>;

/// What the debouncer thread learned about one reported path.
#[derive(Debug, Clone, Copy)]
struct Seen {
    exists: bool,
    is_dir: bool,
}

/// A debounced batch with each event's paths already stat'ed.
type Probed = Result<Vec<(DebouncedEvent, Vec<Seen>)>, Vec<String>>;

/// One drained batch: classified file changes, plus any watcher-level
/// errors from that same batch (kept separate from `changes` since they
/// aren't necessarily about one path).
#[derive(Debug, Default)]
pub struct WatchBatch {
    pub changes: Vec<FileChange>,
    pub errors: Vec<String>,
}

type SharedDebouncer = Arc<Mutex<Debouncer<RecommendedWatcher, RecommendedCache>>>;

/// A root change for the watcher's worker thread.
enum RootOp {
    Watch(PathBuf),
    Unwatch(PathBuf),
}

pub struct FileWatcher {
    debouncer: SharedDebouncer,
    events: Receiver<Probed>,
    /// Lets a detached `watch` report its error through the same channel.
    sender: Sender<Probed>,
    dirty: HashSet<PathBuf>,
    /// Files known to exist as of the last `watch()` call (or since
    /// discovered via a `Create`/`Modify` event) — how `classify`
    /// distinguishes a genuinely new file from macOS's spurious
    /// re-`Create` on an existing file's first edit (see module docs).
    known: Arc<Mutex<HashSet<PathBuf>>>,
    /// Shared with the debouncer thread. Set after `new` so a real OS
    /// event can wake the editor loop without a timer.
    waker: Arc<Mutex<Option<Wake>>>,
    /// The worker that adds and drops roots, started by the first
    /// detached call. It ends when this watcher is dropped.
    roots: Option<Sender<RootOp>>,
}

fn probe(result: DebounceEventResult) -> Probed {
    match result {
        Ok(events) => Ok(events
            .into_iter()
            .map(|event| {
                let seen = event
                    .paths
                    .iter()
                    .map(|path| match std::fs::metadata(path) {
                        Ok(meta) => Seen {
                            exists: true,
                            is_dir: meta.is_dir(),
                        },
                        Err(_) => Seen {
                            exists: path.exists(),
                            is_dir: false,
                        },
                    })
                    .collect();
                (event, seen)
            })
            .collect()),
        Err(errors) => Err(errors.into_iter().map(|e| e.to_string()).collect()),
    }
}

fn wake(slot: &Mutex<Option<Wake>>) {
    if let Ok(guard) = slot.lock()
        && let Some(wake) = guard.as_ref()
    {
        wake();
    }
}

impl FileWatcher {
    /// Starts a debounced watcher. `debounce` is the coalescing window —
    /// how long `notify-debouncer-full` waits for a burst of raw events on
    /// the same path to settle before emitting one.
    pub fn new(debounce: Duration) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let waker: Arc<Mutex<Option<Wake>>> = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&waker);
        let events = tx.clone();
        let debouncer = new_debouncer(debounce, None, move |event| {
            let _ = events.send(probe(event));
            wake(&slot);
        })?;
        Ok(FileWatcher {
            debouncer: Arc::new(Mutex::new(debouncer)),
            events: rx,
            sender: tx,
            dirty: HashSet::new(),
            known: Arc::new(Mutex::new(HashSet::new())),
            waker,
            roots: None,
        })
    }

    /// Called when the debouncer emits an event. The editor registers the
    /// platform loop's waker so an external save is applied without polling.
    pub fn set_waker(&mut self, wake: Wake) {
        if let Ok(mut guard) = self.waker.lock() {
            *guard = Some(wake);
        }
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
    /// seed `known` (see module docs). That is real I/O on the caller;
    /// the editor uses [`FileWatcher::watch_detached`] instead.
    pub fn watch(&mut self, root: impl AsRef<Path>) -> notify::Result<()> {
        watch_root(&self.debouncer, &self.known, root.as_ref())
    }

    /// `watch` on the watcher's worker thread. A failure arrives as an
    /// error in a later batch, and the registered waker runs once the
    /// watch is in place, so the caller never blocks on the recursive scan.
    pub fn watch_detached(&mut self, root: impl AsRef<Path>) {
        self.send_root_op(RootOp::Watch(root.as_ref().to_path_buf()));
    }

    /// Stops watching `root`. Canonicalizes it, which is one `stat`.
    pub fn unwatch(&mut self, root: impl AsRef<Path>) -> notify::Result<()> {
        let root = normalize(root.as_ref());
        self.debouncer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwatch(root)
    }

    /// `unwatch` on the watcher's worker thread, after every root change
    /// asked before it. Other roots and dirty marks are kept.
    pub fn unwatch_detached(&mut self, root: impl AsRef<Path>) {
        self.send_root_op(RootOp::Unwatch(root.as_ref().to_path_buf()));
    }

    fn send_root_op(&mut self, op: RootOp) {
        let op = match self.roots.as_ref() {
            Some(roots) => match roots.send(op) {
                Ok(()) => return,
                Err(mpsc::SendError(op)) => op,
            },
            None => op,
        };
        let (tx, rx) = mpsc::channel();
        let debouncer = Arc::clone(&self.debouncer);
        let known = Arc::clone(&self.known);
        let sender = self.sender.clone();
        let slot = Arc::clone(&self.waker);
        let spawned = std::thread::Builder::new()
            .name("lux-watch-roots".into())
            .spawn(move || {
                for op in rx {
                    apply_root_op(&debouncer, &known, &sender, &slot, op);
                }
            });
        match spawned {
            Ok(_) => {
                let _ = tx.send(op);
                self.roots = Some(tx);
            }
            Err(error) => {
                let _ = self
                    .sender
                    .send(Err(vec![format!("cannot watch: {error}")]));
            }
        }
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

    fn absorb(&mut self, result: Probed, batch: &mut WatchBatch) {
        match result {
            Ok(events) => {
                let known = Arc::clone(&self.known);
                let mut known = known.lock().unwrap_or_else(|e| e.into_inner());
                for (event, seen) in &events {
                    self.classify_into(event, seen, &mut known, &mut batch.changes);
                }
            }
            Err(errors) => batch.errors.extend(errors),
        }
    }

    fn classify_into(
        &self,
        event: &DebouncedEvent,
        seen: &[Seen],
        known: &mut HashSet<PathBuf>,
        changes: &mut Vec<FileChange>,
    ) {
        let is_create = event.kind.is_create();
        let is_modify = event.kind.is_modify();
        let is_remove = event.kind.is_remove();
        if !is_create && !is_modify && !is_remove {
            // Access/Other/Any: not a content change worth surfacing.
            return;
        }
        for (index, path) in event.paths.iter().enumerate() {
            let seen = seen.get(index).copied().unwrap_or(Seen {
                exists: false,
                is_dir: false,
            });
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
            if is_remove || !seen.exists {
                known.remove(path);
                changes.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Removed,
                    is_dir: false,
                });
                continue;
            }
            if is_create && !known.contains(path) {
                known.insert(path.clone());
                changes.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Created,
                    is_dir: seen.is_dir,
                });
                continue;
            }
            // Either a real `Modify`, or a `Create` for a path already in
            // `known` — macOS's own particularity described in the module
            // docs. Both mean the same thing from here: the file already
            // existed, so classify by dirty state instead.
            known.insert(path.clone());
            let kind = if self.dirty.contains(path) {
                ChangeKind::Conflict
            } else {
                ChangeKind::Reload
            };
            changes.push(FileChange {
                path: path.clone(),
                kind,
                is_dir: seen.is_dir,
            });
        }
    }
}

fn apply_root_op(
    debouncer: &SharedDebouncer,
    known: &Mutex<HashSet<PathBuf>>,
    sender: &Sender<Probed>,
    slot: &Mutex<Option<Wake>>,
    op: RootOp,
) {
    match op {
        RootOp::Watch(root) => {
            if let Err(error) = watch_root(debouncer, known, &root) {
                let message = format!("cannot watch {}: {error}", root.display());
                let _ = sender.send(Err(vec![message]));
            }
            wake(slot);
        }
        RootOp::Unwatch(root) => {
            let root = normalize(&root);
            let _ = debouncer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .unwatch(root);
        }
    }
}

fn watch_root(
    debouncer: &SharedDebouncer,
    known: &Mutex<HashSet<PathBuf>>,
    root: &Path,
) -> notify::Result<()> {
    let root = normalize(root);
    debouncer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .watch(&root, RecursiveMode::Recursive)?;
    let mut found = HashSet::new();
    scan_known(&root, &mut found);
    known
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(found);
    Ok(())
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
mod tests;
