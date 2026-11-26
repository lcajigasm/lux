//! Where a session lives (ADR 0019): one file per project under the
//! platform data directory, keyed by a hash of the project path, plus one
//! for the folderless window. Sessions written by earlier versions are
//! read from their old location while the new file does not exist yet.

use std::path::{Path, PathBuf};

use lux_workspace::session_store;

/// The platform data directory for Lux. `XDG_DATA_HOME` wins everywhere
/// so a test or a portable setup can redirect it.
pub(crate) fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(dir).join("lux");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Application Support"))
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        home.map(|h| h.join(".local/share"))
    };
    base.unwrap_or_else(std::env::temp_dir).join("lux")
}

/// The session file for `root`, or for the folderless window.
pub(crate) fn session_file(root: Option<&Path>) -> PathBuf {
    session_store::session_file(&data_dir(), root)
}

/// Where versions before ADR 0019 kept the same session.
pub(crate) fn legacy_session_file(root: Option<&Path>) -> PathBuf {
    match root {
        Some(root) => session_store::legacy_session_file(root),
        None => std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(std::env::temp_dir)
            .join("lux")
            .join("last-session.json"),
    }
}

/// The file to read a session from: the new location, or the legacy one
/// once, while nothing has been written to the new location yet.
pub(crate) fn readable_session_file(root: Option<&Path>) -> Option<PathBuf> {
    readable_session_file_in(&data_dir(), root)
}

/// [`readable_session_file`] with sessions kept under `data_dir`.
pub(crate) fn readable_session_file_in(data_dir: &Path, root: Option<&Path>) -> Option<PathBuf> {
    let current = session_store::session_file(data_dir, root);
    if current.exists() {
        return Some(current);
    }
    let legacy = legacy_session_file(root);
    legacy.exists().then_some(legacy)
}
