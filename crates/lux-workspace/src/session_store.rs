//! Where sessions live and what sits beside them: the split layout and
//! one encoded undo history per tab (ADR 0019).
//!
//! A session is per project, stored under the user's data directory and
//! keyed by a hash of the project path, so nothing is written into the
//! project tree. Each session file `<key>.json` owns a sibling directory
//! `<key>.history/` holding the histories its tabs reference by name.
//! The JSON is written last and atomically, so it only ever names history
//! files that already exist; [`prune_histories`] then removes the rest.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::session::{CursorState, SESSION_FILE_NAME, atomic_write};

/// Session file name for a window with no folder open.
const GLOBAL_SESSION: &str = "global.json";

/// One split pane: which tab it shows (an index into `Session::tabs`) and
/// that pane's own carets and scroll.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneState {
    pub tab: usize,
    #[serde(default)]
    pub cursors: Vec<CursorState>,
    #[serde(default)]
    pub scroll_line: usize,
}

/// The split editor layout. `preset` is the layout's name as the editor
/// spells it (`Columns2`, `Grid2x2`, …); ratios are stored in thousandths
/// so the session stays `Eq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutState {
    pub preset: String,
    #[serde(default)]
    pub ratio_millis: u32,
    #[serde(default)]
    pub ratio_y_millis: u32,
    #[serde(default)]
    pub active_pane: usize,
    #[serde(default)]
    pub panes: Vec<PaneState>,
}

/// The session file for `root` (or the folderless window) under
/// `data_dir`, e.g. `~/.local/share/lux/sessions/3f2a…9c.json`.
pub fn session_file(data_dir: &Path, root: Option<&Path>) -> PathBuf {
    let dir = data_dir.join("sessions");
    match root {
        Some(root) => dir.join(format!("{:016x}.json", path_key(root))),
        None => dir.join(GLOBAL_SESSION),
    }
}

/// Where versions before ADR 0019 wrote a project's session. Read only
/// while the new file does not exist yet; it is left in place, since it
/// sits in the user's project tree and may be tracked there.
pub fn legacy_session_file(root: &Path) -> PathBuf {
    root.join(SESSION_FILE_NAME)
}

/// The directory holding the histories a session file references.
pub fn history_dir(session_file: &Path) -> PathBuf {
    session_file.with_extension("history")
}

/// Writes one encoded history atomically. A file already present under
/// `name` is left alone: names are unique per history revision, so the
/// bytes would be identical.
pub fn write_history(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let path = dir.join(name);
    if path.exists() {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    atomic_write(&path, bytes)
}

pub fn history_exists(dir: &Path, name: &str) -> bool {
    dir.join(name).exists()
}

/// Reads one history, or `None` when it is missing or unreadable — the
/// tab then restores without undo, which is the honest fallback.
pub fn read_history(dir: &Path, name: &str) -> Option<Vec<u8>> {
    if name.contains(['/', '\\']) || name.starts_with('.') {
        return None;
    }
    fs::read(dir.join(name)).ok()
}

/// Deletes every history file in `dir` not named in `keep`.
pub fn prune_histories(dir: &Path, keep: &HashSet<String>) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !keep.contains(name) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// FNV-1a of the path's bytes: stable across runs, unlike `DefaultHasher`.
fn path_key(root: &Path) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in root.as_os_str().as_encoded_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_are_keyed_by_project_and_live_outside_it() {
        let data = Path::new("/data/lux");
        let a = session_file(data, Some(Path::new("/work/a")));
        let b = session_file(data, Some(Path::new("/work/b")));
        assert_ne!(a, b);
        assert!(a.starts_with("/data/lux/sessions"));
        assert_eq!(a, session_file(data, Some(Path::new("/work/a"))));
        assert_eq!(
            session_file(data, None),
            Path::new("/data/lux/sessions/global.json")
        );
        assert_eq!(history_dir(&a), a.with_extension("history"));
        assert_eq!(
            legacy_session_file(Path::new("/work/a")),
            Path::new("/work/a/.lux-session")
        );
    }

    #[test]
    fn histories_are_written_once_read_back_and_pruned() {
        let tmp = std::env::temp_dir().join(format!("lux-store-test-{}", std::process::id()));
        let dir = tmp.join("s.history");
        write_history(&dir, "a.luxh", b"one").unwrap();
        write_history(&dir, "a.luxh", b"two").unwrap();
        write_history(&dir, "b.luxh", b"bee").unwrap();
        assert_eq!(read_history(&dir, "a.luxh").unwrap(), b"one");
        assert!(read_history(&dir, "../a.luxh").is_none());
        assert!(read_history(&dir, "missing").is_none());
        prune_histories(&dir, &HashSet::from(["b.luxh".to_string()])).unwrap();
        assert!(!history_exists(&dir, "a.luxh"));
        assert!(history_exists(&dir, "b.luxh"));
        prune_histories(&tmp.join("none"), &HashSet::new()).unwrap();
        fs::remove_dir_all(&tmp).ok();
    }
}
