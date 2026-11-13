//! Persisted Open Recent list. The file is read on first use and written
//! from the `lux-recent` thread.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, Once};
#[cfg(test)]
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Menu tags 200..210. `MenuAction` starts again at 210.
pub(crate) const TAG_BASE: isize = 200;
pub(crate) const SLOTS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RecentFileList {
    paths: Vec<PathBuf>,
}

struct Recent {
    paths: Vec<PathBuf>,
    loaded: bool,
    queued: u64,
    written: u64,
}

static STATE: Mutex<Recent> = Mutex::new(Recent {
    paths: Vec::new(),
    loaded: false,
    queued: 0,
    written: 0,
});
static WAKE: Condvar = Condvar::new();
static DONE: Condvar = Condvar::new();

thread_local! {
    static RECORD_HERE: Cell<bool> = const { Cell::new(false) };
}

fn lock() -> std::sync::MutexGuard<'static, Recent> {
    STATE.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn store_path() -> PathBuf {
    if let Some(path) = std::env::var_os("LUX_RECENT_PATH") {
        return PathBuf::from(path);
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("lux")
        .join("recent_files.json")
}

fn load_disk() -> Vec<PathBuf> {
    let path = store_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<RecentFileList>(&text)
        .map(|list| list.paths)
        .unwrap_or_default()
}

fn write_disk(path: &Path, paths: &[PathBuf]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(text) = serde_json::to_string_pretty(&RecentFileList {
        paths: paths.to_vec(),
    }) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn ensure_loaded(recent: &mut Recent) {
    if recent.loaded {
        return;
    }
    recent.paths = load_disk();
    recent.loaded = true;
}

fn ensure_writer() {
    static START: Once = Once::new();
    START.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("lux-recent".into())
            .spawn(|| {
                loop {
                    let (paths, generation, file) = {
                        let mut recent = lock();
                        while recent.written == recent.queued {
                            recent = WAKE
                                .wait(recent)
                                .unwrap_or_else(|poison| poison.into_inner());
                        }
                        (recent.paths.clone(), recent.queued, store_path())
                    };
                    write_disk(&file, &paths);
                    let mut recent = lock();
                    if recent.queued == generation {
                        recent.written = generation;
                        DONE.notify_all();
                    }
                }
            });
    });
}

fn recording_enabled() -> bool {
    #[cfg(not(test))]
    {
        true
    }
    #[cfg(test)]
    {
        RECORD_HERE.with(Cell::get)
    }
}

fn publish() {
    #[cfg(target_os = "macos")]
    super::macos::refresh_open_recent();
}

/// Remember `path` after a real open or save. Tests ignore this unless
/// [`begin_test`](begin_test) enabled recording on this thread.
pub(crate) fn note(path: Option<&Path>) {
    if !recording_enabled() {
        return;
    }
    let Some(path) = path else {
        return;
    };
    if path.as_os_str().is_empty() {
        return;
    }
    ensure_writer();
    {
        let mut recent = lock();
        ensure_loaded(&mut recent);
        recent.paths.retain(|existing| existing != path);
        recent.paths.insert(0, path.to_path_buf());
        recent.paths.truncate(SLOTS);
        recent.queued += 1;
    }
    WAKE.notify_one();
    publish();
}

pub(crate) fn clear() {
    if !recording_enabled() {
        return;
    }
    ensure_writer();
    {
        let mut recent = lock();
        ensure_loaded(&mut recent);
        recent.paths.clear();
        recent.queued += 1;
    }
    WAKE.notify_one();
    publish();
}

pub(crate) fn entries() -> Vec<PathBuf> {
    let mut recent = lock();
    ensure_loaded(&mut recent);
    recent.paths.clone()
}

pub(crate) fn path_at(index: usize) -> Option<PathBuf> {
    entries().into_iter().nth(index)
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
pub(crate) fn index_from_tag(tag: isize) -> Option<usize> {
    let offset = tag.checked_sub(TAG_BASE)?;
    if (0..SLOTS as isize).contains(&offset) {
        Some(offset as usize)
    } else {
        None
    }
}

#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
pub(crate) fn label(path: &Path, all: &[PathBuf]) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let duplicates = all
        .iter()
        .filter(|other| other.file_name() == path.file_name())
        .count()
        > 1;
    if duplicates {
        format!("{name} — {}", path.display())
    } else {
        name
    }
}

#[cfg(test)]
pub(crate) fn sync() {
    let mut recent = lock();
    let deadline = Instant::now() + Duration::from_secs(2);
    while recent.written != recent.queued {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            panic!("recent file was not written");
        }
        let (next, timeout) = DONE
            .wait_timeout(recent, remaining)
            .unwrap_or_else(|poison| poison.into_inner());
        recent = next;
        if timeout.timed_out() && recent.written != recent.queued {
            panic!("recent file was not written");
        }
    }
}

#[cfg(test)]
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) struct TestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for TestGuard {
    fn drop(&mut self) {
        RECORD_HERE.with(|flag| flag.set(false));
        unsafe { std::env::remove_var("LUX_RECENT_PATH") };
    }
}

#[cfg(test)]
pub(crate) fn begin_test(path: &Path) -> TestGuard {
    let guard = TestGuard {
        _lock: TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()),
    };
    unsafe { std::env::set_var("LUX_RECENT_PATH", path) };
    RECORD_HERE.with(|flag| flag.set(true));
    {
        let mut recent = lock();
        recent.loaded = false;
        recent.paths.clear();
    }
    guard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_list_is_persisted_deduped_capped_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("recent.json");
        let _guard = begin_test(&store);
        let older = dir.path().join("older.txt");
        let newer = dir.path().join("newer.txt");
        note(Some(&older));
        note(Some(&newer));
        note(Some(&older));
        sync();
        assert_eq!(entries(), vec![older.clone(), newer.clone()]);

        {
            let mut recent = lock();
            recent.loaded = false;
            recent.paths.clear();
        }
        assert_eq!(entries(), vec![older.clone(), newer.clone()]);

        for index in 0..12 {
            note(Some(&dir.path().join(format!("f{index}.txt"))));
        }
        sync();
        let listed = entries();
        assert_eq!(listed.len(), SLOTS);
        assert_eq!(listed[0], dir.path().join("f11.txt"));
        assert!(!listed.iter().any(|path| path == &older));

        clear();
        sync();
        assert!(entries().is_empty());
        let text = std::fs::read_to_string(&store).unwrap();
        assert!(text.contains("\"paths\""));
        assert!(!text.contains("f11.txt"));
    }

    #[test]
    fn recent_tags_are_ten_slots_and_not_menu_actions() {
        assert_eq!(index_from_tag(199), None);
        assert_eq!(index_from_tag(TAG_BASE), Some(0));
        assert_eq!(index_from_tag(TAG_BASE + 9), Some(9));
        assert_eq!(index_from_tag(TAG_BASE + SLOTS as isize), None);
        let (alone, shared) = dir_label();
        assert_eq!(alone, "b.txt");
        assert!(shared.contains("a.txt —"), "{shared}");
    }

    fn dir_label() -> (String, String) {
        let a = PathBuf::from("/tmp/one/a.txt");
        let b = PathBuf::from("/tmp/two/a.txt");
        let alone = PathBuf::from("/tmp/one/b.txt");
        let all = vec![a.clone(), b.clone(), alone.clone()];
        (label(&alone, &all), label(&a, &all))
    }
}
