//! Background project symbol indexing (ST-NAV-03).
//!
//! One thread owns the [`SymbolIndex`]. It walks the workspace once, then
//! sleeps on a channel until the file watcher reports changes. After each
//! batch it publishes an immutable [`SymbolSnapshot`] and calls the
//! registered waker. The UI thread only swaps in the newest snapshot and
//! filters it: no disk access and no parsing happen there.
//!
//! Dropping the [`SymbolIndexer`] cancels the thread: the first scan checks
//! a flag between files, and no snapshot is published or waker called
//! after the drop returns.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use ignore::WalkBuilder;

use crate::symbols::{SymbolIndex, SymbolKind};
use crate::watcher::{ChangeKind, FileChange};

type Wake = Arc<dyn Fn() + Send + Sync>;

/// Files indexed before the first progress snapshot of the first scan.
/// Each later interval doubles, so rebuilding the snapshot costs about
/// twice the final snapshot in total instead of growing quadratically.
const FIRST_PROGRESS_FILES: usize = 400;

/// One symbol, with its lowercase forms computed once off the UI thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    /// Relative to the workspace root when the file lies inside it.
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
    name_lower: String,
    path_lower: String,
}

/// An immutable, name-sorted view of the index at one moment.
#[derive(Debug, Default)]
pub struct SymbolSnapshot {
    entries: Vec<SymbolEntry>,
    indexing: bool,
}

impl SymbolSnapshot {
    /// Builds a snapshot. `indexing` is true while the first scan runs.
    pub fn from_index(index: &SymbolIndex, indexing: bool) -> Self {
        let mut entries: Vec<SymbolEntry> = index
            .iter()
            .map(|s| SymbolEntry {
                name_lower: s.name.to_lowercase(),
                path_lower: s.path.to_string_lossy().to_lowercase(),
                name: s.name.clone(),
                kind: s.kind,
                path: s.path.clone(),
                line: s.line,
                column: s.column,
            })
            .collect();
        entries.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.line.cmp(&b.line))
        });
        Self { entries, indexing }
    }

    /// The first workspace scan has not finished yet.
    pub fn is_indexing(&self) -> bool {
        self.indexing
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Ranks symbols against `query` and returns at most `limit` of them.
    /// `file@name` restricts to paths containing `file`. Ranking: exact
    /// name, then prefix, then substring; shorter names first within a
    /// tier; ties in name and path order.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&SymbolEntry> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.entries.iter().take(limit).collect();
        }
        let (file_part, sym_part) = match trimmed.split_once('@') {
            Some((f, s)) => (f.trim().to_lowercase(), s.trim().to_lowercase()),
            None => (String::new(), trimmed.to_lowercase()),
        };

        let mut scored: Vec<(i64, usize)> = Vec::new();
        for (i, entry) in self.entries.iter().enumerate() {
            if !file_part.is_empty() && !entry.path_lower.contains(&file_part) {
                continue;
            }
            let Some(score) = score(&entry.name_lower, &sym_part) else {
                continue;
            };
            scored.push((score, i));
        }
        let by_rank = |a: &(i64, usize), b: &(i64, usize)| b.0.cmp(&a.0).then(a.1.cmp(&b.1));
        if scored.len() > limit && limit > 0 {
            scored.select_nth_unstable_by(limit - 1, by_rank);
            scored.truncate(limit);
        }
        scored.sort_unstable_by(by_rank);
        scored
            .into_iter()
            .take(limit)
            .map(|(_, i)| &self.entries[i])
            .collect()
    }
}

fn score(name_lower: &str, sym_part: &str) -> Option<i64> {
    let extra = name_lower.len() as i64 - sym_part.len() as i64;
    if sym_part.is_empty() {
        Some(100)
    } else if name_lower == sym_part {
        Some(1000)
    } else if name_lower.starts_with(sym_part) {
        Some(500 - extra)
    } else if name_lower.contains(sym_part) {
        Some(250 - extra)
    } else {
        None
    }
}

/// Owns the background indexing thread for one workspace root.
pub struct SymbolIndexer {
    changes: Sender<Vec<FileChange>>,
    published: Arc<Mutex<Option<Arc<SymbolSnapshot>>>>,
    waker: Arc<Mutex<Option<Wake>>>,
    current: Arc<SymbolSnapshot>,
    cancelled: Arc<AtomicBool>,
    /// Never joined outside tests: dropping must not block the UI thread.
    #[cfg_attr(not(test), allow(dead_code))]
    thread: Option<JoinHandle<()>>,
}

impl Drop for SymbolIndexer {
    /// Cancels the thread without waiting for it. The flag is set and the
    /// waker cleared under the waker lock, which `publish` holds while it
    /// checks the flag and wakes, so no wake can follow this call.
    fn drop(&mut self) {
        let mut slot = self.waker.lock().unwrap_or_else(|e| e.into_inner());
        self.cancelled.store(true, Ordering::Relaxed);
        *slot = None;
    }
}

impl SymbolIndexer {
    /// Starts the thread. With a `root` it walks it (respecting
    /// `.gitignore`) and indexes every file; without one it indexes only
    /// the files later passed to [`SymbolIndexer::index_files`]. It then
    /// waits for changes. The thread ends when this handle is dropped.
    pub fn start(root: Option<PathBuf>, waker: Option<Wake>) -> Self {
        let (tx, rx) = mpsc::channel();
        let published = Arc::new(Mutex::new(None));
        let waker = Arc::new(Mutex::new(waker));
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            published: Arc::clone(&published),
            waker: Arc::clone(&waker),
            cancelled: Arc::clone(&cancelled),
        };
        let thread = thread::Builder::new()
            .name("lux-symbol-indexer".to_string())
            .spawn(move || worker.run(root.as_deref(), &rx))
            .expect("symbol indexing thread must spawn");
        Self {
            cancelled,
            thread: Some(thread),
            changes: tx,
            published,
            waker,
            current: Arc::new(SymbolSnapshot {
                entries: Vec::new(),
                indexing: true,
            }),
        }
    }

    /// Sets the callback run after each published snapshot.
    pub fn set_waker(&self, wake: Wake) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(wake);
        }
    }

    /// Asks the thread to (re)index these files, e.g. the open tabs. A file
    /// whose mtime is unchanged is not read again. Never touches the disk.
    pub fn index_files(&self, paths: impl IntoIterator<Item = PathBuf>) {
        let batch = paths
            .into_iter()
            .map(|path| FileChange {
                path,
                kind: ChangeKind::Reload,
                is_dir: false,
            })
            .collect();
        let _ = self.changes.send(batch);
    }

    /// Hands watcher changes to the thread. Never touches the disk.
    pub fn apply_changes(&self, changes: &[FileChange]) {
        if !changes.is_empty() {
            let _ = self.changes.send(changes.to_vec());
        }
    }

    /// Takes the newest published snapshot, if any. Returns whether the
    /// current snapshot changed.
    pub fn refresh(&mut self) -> bool {
        let next = self.published.lock().ok().and_then(|mut slot| slot.take());
        match next {
            Some(snapshot) => {
                self.current = snapshot;
                true
            }
            None => false,
        }
    }

    pub fn snapshot(&self) -> &Arc<SymbolSnapshot> {
        &self.current
    }
}

/// A path the watcher did not report (an open tab, say) may spell the
/// root differently, e.g. `/tmp` for `/private/tmp`. Resolve it only when
/// that places it under the root; otherwise keep it as the editor knows it.
fn keyed_path(index: &SymbolIndex, path: &Path) -> PathBuf {
    match index.root() {
        Some(root) if !path.starts_with(root) => std::fs::canonicalize(path)
            .ok()
            .filter(|resolved| resolved.starts_with(root))
            .unwrap_or_else(|| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

struct Worker {
    published: Arc<Mutex<Option<Arc<SymbolSnapshot>>>>,
    waker: Arc<Mutex<Option<Wake>>>,
    cancelled: Arc<AtomicBool>,
}

impl Worker {
    fn run(&self, root: Option<&Path>, rx: &Receiver<Vec<FileChange>>) {
        let mut ready = false;
        let mut index = SymbolIndex::new();
        if let Some(root) = root {
            // The watcher reports canonical paths; key the index the same way.
            let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
            index.set_root(&root);
            if !self.first_scan(&root, &mut index) {
                return;
            }
            self.publish(&index, false);
            ready = true;
        }

        while let Ok(mut batch) = rx.recv() {
            while let Ok(more) = rx.try_recv() {
                batch.extend(more);
            }
            let mut changed = false;
            for change in &batch {
                if change.kind == ChangeKind::Removed {
                    index.remove_file(&change.path);
                    changed = true;
                } else {
                    let path = keyed_path(&index, &change.path);
                    changed |= index.index_file_path(&path);
                }
            }
            if self.is_cancelled() {
                return;
            }
            if changed || !ready {
                self.publish(&index, false);
                ready = true;
            }
        }
    }

    /// Walks `root` (`.gitignore`-aware, like `WorkspaceIndex::build`) and
    /// indexes each file as it is found, publishing progress at doubling
    /// intervals. Returns false if the indexer was dropped meanwhile.
    fn first_scan(&self, root: &Path, index: &mut SymbolIndex) -> bool {
        let mut indexed = 0usize;
        let mut interval = FIRST_PROGRESS_FILES;
        let mut next_publish = interval;
        for entry in WalkBuilder::new(root).require_git(false).build() {
            if self.is_cancelled() {
                return false;
            }
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            index.index_file_path(entry.path());
            indexed += 1;
            if indexed == next_publish {
                self.publish(index, true);
                interval *= 2;
                next_publish += interval;
            }
        }
        !self.is_cancelled()
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn publish(&self, index: &SymbolIndex, indexing: bool) {
        if self.is_cancelled() {
            return;
        }
        let snapshot = Arc::new(SymbolSnapshot::from_index(index, indexing));
        if let Ok(mut slot) = self.published.lock() {
            *slot = Some(snapshot);
        }
        // Held while waking so `Drop` cannot return between the check and
        // the wake. The waker only posts to the platform loop.
        let slot = self.waker.lock().unwrap_or_else(|e| e.into_inner());
        if self.is_cancelled() {
            return;
        }
        if let Some(wake) = slot.as_ref() {
            wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

    fn wait_for(
        indexer: &mut SymbolIndexer,
        woke: &Receiver<()>,
        done: impl Fn(&SymbolSnapshot) -> bool,
    ) {
        loop {
            match woke.recv_timeout(Duration::from_secs(10)) {
                Ok(()) => {}
                Err(RecvTimeoutError::Timeout) => panic!("the indexer never woke the UI"),
                Err(RecvTimeoutError::Disconnected) => panic!("waker dropped"),
            }
            indexer.refresh();
            if done(indexer.snapshot()) {
                return;
            }
        }
    }

    #[test]
    fn the_thread_scans_then_follows_watcher_changes() {
        let dir = std::env::temp_dir().join(format!("lux-symbol-worker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.rs"), "fn alpha_one() {}\n").unwrap();
        std::fs::write(dir.join("b.py"), "def beta_two():\n    pass\n").unwrap();

        let (tx, woke) = mpsc::channel();
        let mut indexer = SymbolIndexer::start(
            Some(dir.clone()),
            Some(Arc::new(move || {
                let _ = tx.send(());
            })),
        );
        assert!(
            indexer.snapshot().is_indexing(),
            "nothing is claimed before the scan"
        );
        wait_for(&mut indexer, &woke, |s| !s.is_indexing());
        let snap = Arc::clone(indexer.snapshot());
        assert_eq!(snap.len(), 2);
        let hit = snap.search("alpha", 10);
        assert_eq!(hit[0].path, Path::new("src/a.rs"));

        let root = std::fs::canonicalize(&dir).unwrap();
        std::fs::write(root.join("c.rs"), "struct GammaThree;\n").unwrap();
        std::fs::remove_file(root.join("b.py")).unwrap();
        indexer.apply_changes(&[
            FileChange {
                path: root.join("c.rs"),
                kind: ChangeKind::Created,
                is_dir: false,
            },
            FileChange {
                path: root.join("b.py"),
                kind: ChangeKind::Removed,
                is_dir: false,
            },
        ]);
        wait_for(&mut indexer, &woke, |s| s.search("gamma", 10).len() == 1);
        assert!(indexer.snapshot().search("beta", 10).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dropping_mid_scan_stops_the_thread_and_its_wakes() {
        let dir = std::env::temp_dir().join(format!("lux-symbol-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in 0..10 {
            let sub = dir.join(format!("d{d}"));
            std::fs::create_dir_all(&sub).unwrap();
            for f in 0..500 {
                std::fs::write(
                    sub.join(format!("f{f}.txt")),
                    format!("fn s{d}_{f}() {{}}\n"),
                )
                .unwrap();
            }
        }
        // A full scan of 5,000 files publishes at 400, 1,200 and 2,800
        // files and once at the end: four wakes.
        let wakes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (tx, woke) = mpsc::channel();
        let counter = Arc::clone(&wakes);
        let mut indexer = SymbolIndexer::start(
            Some(dir.clone()),
            Some(Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                let _ = tx.send(());
            })),
        );
        woke.recv_timeout(Duration::from_secs(10))
            .expect("the first progress snapshot");
        indexer.refresh();
        assert!(indexer.snapshot().is_indexing(), "still mid-scan");

        let thread = indexer.thread.take().unwrap();
        drop(indexer);
        let at_drop = wakes.load(Ordering::SeqCst);
        thread.join().unwrap();
        assert_eq!(
            wakes.load(Ordering::SeqCst),
            at_drop,
            "no wake after the indexer is dropped"
        );
        assert!(at_drop < 4, "the scan stopped before finishing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_root_only_the_requested_files_are_indexed() {
        let dir = std::env::temp_dir().join(format!("lux-symbol-loose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("open.rs"), "fn open_tab_fn() {}\n").unwrap();
        std::fs::write(dir.join("closed.rs"), "fn closed_fn() {}\n").unwrap();

        let (tx, woke) = mpsc::channel();
        let mut indexer = SymbolIndexer::start(
            None,
            Some(Arc::new(move || {
                let _ = tx.send(());
            })),
        );
        indexer.index_files([dir.join("open.rs")]);
        wait_for(&mut indexer, &woke, |s| !s.is_indexing());
        assert_eq!(indexer.snapshot().search("open_tab", 10).len(), 1);
        assert!(indexer.snapshot().search("closed", 10).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_ranks_exact_then_prefix_then_substring_and_limits() {
        let mut index = SymbolIndex::new();
        index.index_file_content(
            Path::new("src/lib.rs"),
            "fn parse() {}\nfn parse_args() {}\nfn reparse() {}\nfn other() {}\n",
            None,
        );
        let snap = SymbolSnapshot::from_index(&index, false);
        let names: Vec<&str> = snap
            .search("parse", 10)
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, ["parse", "parse_args", "reparse"]);
        assert_eq!(snap.search("parse", 2).len(), 2);
        assert_eq!(snap.search("lib@other", 10)[0].name, "other");
        assert!(snap.search("main@other", 10).is_empty());
        assert_eq!(snap.search("", 3).len(), 3);
    }
}
