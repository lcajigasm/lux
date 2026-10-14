//! Project symbol indexing and search (ST08).
//!
//! Indexes semantic symbols across workspace files using `lux-syntax` Tree-sitter
//! and line-based extractors, updating incrementally when files change.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::SystemTime;

pub use lux_syntax::SymbolKind;

use crate::watcher::{ChangeKind, FileChange};

/// Maximum file size to index for symbols (2 MB) to prevent out-of-memory or parsing hangs.
pub const MAX_INDEX_FILE_SIZE_BYTES: u64 = 2 * 1024 * 1024;

/// A symbol defined within a workspace file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
    pub container: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedFileSymbols {
    mtime: Option<SystemTime>,
    symbols: Vec<WorkspaceSymbol>,
}

/// Project-wide symbol index for rapid lookup and fuzzy searching across the workspace.
#[derive(Debug, Clone)]
pub struct SymbolIndex {
    root: Option<PathBuf>,
    file_symbols: HashMap<PathBuf, CachedFileSymbols>,
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolIndex {
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: None,
            file_symbols: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
            file_symbols: HashMap::new(),
        }
    }

    pub fn set_root(&mut self, root: impl Into<PathBuf>) {
        self.root = Some(root.into());
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Indexes or updates symbols for a single file from its in-memory source.
    pub fn index_file_content(
        &mut self,
        rel_or_abs_path: &Path,
        content: &str,
        mtime: Option<SystemTime>,
    ) {
        let file_name = rel_or_abs_path.file_name().and_then(|n| n.to_str());
        let raw_symbols = lux_syntax::extract_symbols_from_source(content, file_name);

        let symbols = raw_symbols
            .into_iter()
            .map(|s| WorkspaceSymbol {
                name: s.name,
                kind: s.kind,
                path: rel_or_abs_path.to_path_buf(),
                line: s.line,
                column: s.column,
                container: None,
            })
            .collect();

        self.file_symbols.insert(
            rel_or_abs_path.to_path_buf(),
            CachedFileSymbols { mtime, symbols },
        );
    }

    /// Indexes a file from disk if it exists and falls under the size cap.
    pub fn index_file_path(&mut self, path: &Path) {
        let Ok(meta) = fs::metadata(path) else {
            self.file_symbols.remove(path);
            return;
        };

        if !meta.is_file() || meta.len() > MAX_INDEX_FILE_SIZE_BYTES {
            return;
        }

        let mtime = meta.modified().ok();
        if let Some(cached) = self.file_symbols.get(path)
            && cached.mtime.is_some()
            && cached.mtime == mtime
        {
            return;
        }

        if let Ok(content) = fs::read_to_string(path) {
            let rel_path = if let Some(root) = &self.root {
                path.strip_prefix(root).unwrap_or(path)
            } else {
                path
            };
            self.index_file_content(rel_path, &content, mtime);
        }
    }

    /// Removes a file from the index.
    pub fn remove_file(&mut self, path: &Path) {
        let rel_path = if let Some(root) = &self.root {
            path.strip_prefix(root).unwrap_or(path)
        } else {
            path
        };
        self.file_symbols.remove(rel_path);
        self.file_symbols.remove(path);
    }

    /// Updates the index according to file changes observed by a watcher.
    pub fn apply_changes(&mut self, changes: &[FileChange]) {
        for change in changes {
            match change.kind {
                ChangeKind::Created | ChangeKind::Reload | ChangeKind::Conflict => {
                    self.index_file_path(&change.path);
                }
                ChangeKind::Removed => {
                    self.remove_file(&change.path);
                }
            }
        }
    }

    /// Total number of indexed symbols.
    pub fn len(&self) -> usize {
        self.file_symbols.values().map(|c| c.symbols.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns all indexed symbols.
    pub fn all_symbols(&self) -> Vec<WorkspaceSymbol> {
        let mut list = Vec::new();
        for cached in self.file_symbols.values() {
            list.extend(cached.symbols.clone());
        }
        list
    }

    /// Performs a ranked search for `query` across all indexed symbols.
    pub fn search(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let trimmed = query.trim();
        let mut all = self.all_symbols();

        if trimmed.is_empty() {
            all.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
            return all;
        }

        let query_lower = trimmed.to_lowercase();
        let (file_part, sym_part) = if let Some((f, s)) = trimmed.split_once('@') {
            (Some(f.trim().to_lowercase()), s.trim().to_lowercase())
        } else {
            (None, query_lower)
        };

        let mut scored: Vec<(i32, WorkspaceSymbol)> = Vec::new();
        for sym in all {
            let name_lower = sym.name.to_lowercase();
            let path_str = sym.path.to_string_lossy().to_lowercase();

            if let Some(f) = &file_part
                && !f.is_empty()
                && !path_str.contains(f)
            {
                continue;
            }

            let score = if sym_part.is_empty() {
                100
            } else if name_lower == sym_part {
                1000
            } else if name_lower.starts_with(&sym_part) {
                500 - (name_lower.len() as i32 - sym_part.len() as i32)
            } else if name_lower.contains(&sym_part) {
                250 - (name_lower.len() as i32 - sym_part.len() as i32)
            } else {
                continue;
            };

            scored.push((score, sym));
        }

        scored.sort_by(|(s1, a), (s2, b)| {
            s2.cmp(s1)
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.path.cmp(&b.path))
        });

        scored.into_iter().map(|(_, sym)| sym).collect()
    }
}

/// Handle to a background symbol indexing job.
pub struct SymbolIndexWorker {
    shared_index: Arc<RwLock<SymbolIndex>>,
    is_done: Arc<AtomicBool>,
    rx: Receiver<WorkspaceSymbol>,
}

impl SymbolIndexWorker {
    /// Starts indexing the given files in a background worker thread.
    pub fn start(root: Option<PathBuf>, files: Vec<PathBuf>) -> Self {
        let shared_index = Arc::new(RwLock::new(SymbolIndex {
            root: root.clone(),
            file_symbols: HashMap::new(),
        }));
        let is_done = Arc::new(AtomicBool::new(false));
        let (tx, rx): (Sender<WorkspaceSymbol>, Receiver<WorkspaceSymbol>) = mpsc::channel();

        let shared_clone = Arc::clone(&shared_index);
        let done_clone = Arc::clone(&is_done);

        thread::Builder::new()
            .name("lux-symbol-indexer".to_string())
            .spawn(move || {
                for path in files {
                    if let Ok(meta) = fs::metadata(&path)
                        && meta.is_file()
                        && meta.len() <= MAX_INDEX_FILE_SIZE_BYTES
                        && let Ok(content) = fs::read_to_string(&path)
                    {
                        let rel_path = if let Some(r) = &root {
                            path.strip_prefix(r).unwrap_or(&path).to_path_buf()
                        } else {
                            path.clone()
                        };
                        let mtime = meta.modified().ok();
                        let mut local_index = SymbolIndex::new();
                        local_index.index_file_content(&rel_path, &content, mtime);

                        for sym in local_index.all_symbols() {
                            let _ = tx.send(sym);
                        }

                        if let Ok(mut idx) = shared_clone.write() {
                            idx.index_file_content(&rel_path, &content, mtime);
                        }
                    }
                }
                done_clone.store(true, Ordering::SeqCst);
            })
            .expect("symbol indexing thread must spawn");

        Self {
            shared_index,
            is_done,
            rx,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.is_done.load(Ordering::SeqCst)
    }

    /// Polls newly indexed symbols without blocking.
    pub fn poll_symbols(&self) -> Vec<WorkspaceSymbol> {
        let mut new_symbols = Vec::new();
        while let Ok(sym) = self.rx.try_recv() {
            new_symbols.push(sym);
        }
        new_symbols
    }

    pub fn snapshot(&self) -> SymbolIndex {
        self.shared_index
            .read()
            .map(|i| i.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-workspace-symbols-test-{:?}-{}",
            std::thread::current().id(),
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        fs::canonicalize(&dir).unwrap_or(dir)
    }

    #[test]
    fn index_and_search_symbols_in_memory() {
        let mut index = SymbolIndex::new();
        let rs_content = r#"
pub fn run_server() {}
pub struct ConfigManager;
pub trait EventHandler {}
"#;
        index.index_file_content(Path::new("src/server.rs"), rs_content, None);

        let py_content = r#"
def calculate_tax():
    pass
class AccountController:
    pass
"#;
        index.index_file_content(Path::new("scripts/tax.py"), py_content, None);

        assert_eq!(index.len(), 5);

        let results = index.search("server");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "run_server");
        assert_eq!(results[0].kind, SymbolKind::Function);
        assert_eq!(results[0].path, Path::new("src/server.rs"));

        let results = index.search("Config");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "ConfigManager");

        let results = index.search("Account");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "AccountController");

        let all = index.search("");
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn background_worker_indexes_files() {
        let dir = temp_dir();
        let f1 = dir.join("a.rs");
        let f2 = dir.join("b.py");

        fs::write(&f1, "fn helper_a() {}\n").expect("write a");
        fs::write(&f2, "def helper_b():\n    pass\n").expect("write b");

        let worker = SymbolIndexWorker::start(Some(dir.clone()), vec![f1, f2]);
        while !worker.is_finished() {
            thread::sleep(std::time::Duration::from_millis(10));
        }

        let snap = worker.snapshot();
        assert_eq!(snap.len(), 2);
        let found = snap.search("helper");
        assert_eq!(found.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }
}
