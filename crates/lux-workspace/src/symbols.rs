//! Project symbol indexing and search (ST08).
//!
//! Indexes semantic symbols across workspace files using `lux-syntax` Tree-sitter
//! and line-based extractors, updating incrementally when files change.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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

    /// The key a file is stored under: relative to the root when it lies
    /// inside it, otherwise the path as given.
    fn key_for(&self, path: &Path) -> PathBuf {
        match &self.root {
            Some(root) => path.strip_prefix(root).unwrap_or(path).to_path_buf(),
            None => path.to_path_buf(),
        }
    }

    /// Indexes a file from disk if it exists and falls under the size cap.
    /// A file whose modification time matches the cached entry is not
    /// read again. Returns `true` when the file was read and parsed.
    pub fn index_file_path(&mut self, path: &Path) -> bool {
        let key = self.key_for(path);
        let Ok(meta) = fs::metadata(path) else {
            self.file_symbols.remove(&key);
            return false;
        };

        if !meta.is_file() || meta.len() > MAX_INDEX_FILE_SIZE_BYTES {
            self.file_symbols.remove(&key);
            return false;
        }

        let mtime = meta.modified().ok();
        if let Some(cached) = self.file_symbols.get(&key)
            && cached.mtime.is_some()
            && cached.mtime == mtime
        {
            return false;
        }

        match fs::read_to_string(path) {
            Ok(content) => {
                self.index_file_content(&key, &content, mtime);
                true
            }
            Err(_) => {
                self.file_symbols.remove(&key);
                false
            }
        }
    }

    /// Removes a file from the index.
    pub fn remove_file(&mut self, path: &Path) {
        let key = self.key_for(path);
        self.file_symbols.remove(&key);
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

    /// Iterates every indexed symbol without copying.
    pub fn iter(&self) -> impl Iterator<Item = &WorkspaceSymbol> {
        self.file_symbols.values().flat_map(|c| c.symbols.iter())
    }

    /// Number of files with a cached entry.
    pub fn file_count(&self) -> usize {
        self.file_symbols.len()
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
    fn an_unchanged_file_is_served_from_the_cache() {
        let dir = temp_dir().join("cache-key");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("lib.rs");
        fs::write(&file, "fn first_name() {}\n").expect("write");
        let mtime = fs::metadata(&file)
            .and_then(|m| m.modified())
            .expect("mtime");

        let mut index = SymbolIndex::with_root(&dir);
        assert!(
            index.index_file_path(&file),
            "the first pass reads the file"
        );
        assert_eq!(index.search("first_name").len(), 1);
        assert_eq!(index.search("")[0].path, Path::new("lib.rs"));

        // Same mtime, different bytes: a cache hit must not read the file.
        fs::write(&file, "fn second_name() {}\n").expect("rewrite");
        fs::File::options()
            .write(true)
            .open(&file)
            .and_then(|f| f.set_modified(mtime))
            .expect("restore mtime");
        assert!(!index.index_file_path(&file), "an unchanged mtime is a hit");
        assert_eq!(index.search("first_name").len(), 1);
        assert_eq!(index.file_count(), 1);

        index.remove_file(&file);
        assert!(index.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
