//! Huge file handling (Phase 4 deliverable 8, the "huge" half of "huge and
//! binary files"): above a configurable size threshold, a file is opened
//! in a read-only mode with syntax highlighting explicitly signaled off —
//! never silently rendered (or edited) at full cost. Classification here
//! is cheap and separate from actually opening a file: [`classify_path`]
//! answers "how big is this" with one `fs::metadata` call, no content
//! read, so a caller can decide *how* to open something before paying for
//! opening it. [`crate::buffer::Buffer::open_with_policy`] is where this
//! and [`crate::binary_detect`] (the sibling half of this deliverable)
//! actually combine into one real open path.

use std::fs;
use std::io;
use std::path::Path;

/// Default huge-file threshold: 10 MB. Comfortably below the 100 MB size
/// `docs/PERFORMANCE.md`'s "open a 100 MB file" budget already exercises
/// (`Buffer::open` handles that size, just not RSS-optimally — a known,
/// separately tracked gap, not something this deliverable changes), and a
/// well-understood "this single text file is unusually large" size in
/// most editors.
pub const DEFAULT_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;

/// A configurable byte threshold above which a file counts as "huge".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HugeFilePolicy {
    pub threshold_bytes: u64,
}

impl HugeFilePolicy {
    pub fn new(threshold_bytes: u64) -> Self {
        HugeFilePolicy { threshold_bytes }
    }
}

impl Default for HugeFilePolicy {
    fn default() -> Self {
        HugeFilePolicy {
            threshold_bytes: DEFAULT_THRESHOLD_BYTES,
        }
    }
}

/// A file's size, classified against a [`HugeFilePolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSizeClass {
    Normal { size_bytes: u64 },
    Huge { size_bytes: u64 },
}

impl FileSizeClass {
    pub fn size_bytes(&self) -> u64 {
        match self {
            FileSizeClass::Normal { size_bytes } | FileSizeClass::Huge { size_bytes } => {
                *size_bytes
            }
        }
    }

    pub fn is_huge(&self) -> bool {
        matches!(self, FileSizeClass::Huge { .. })
    }
}

/// Classifies `size_bytes` against `policy` — the pure comparison
/// `classify_path` (and, for a size already known some other way, any
/// other caller) builds on.
pub fn classify(size_bytes: u64, policy: HugeFilePolicy) -> FileSizeClass {
    if size_bytes >= policy.threshold_bytes {
        FileSizeClass::Huge { size_bytes }
    } else {
        FileSizeClass::Normal { size_bytes }
    }
}

/// Cheaply classifies the file at `path` via `fs::metadata` alone — no
/// content read at all, so this is safe to call even on a file so huge
/// that reading it would itself be the expensive operation this
/// deliverable exists to avoid doing unconditionally.
pub fn classify_path(path: impl AsRef<Path>, policy: HugeFilePolicy) -> io::Result<FileSizeClass> {
    let size_bytes = fs::metadata(path)?.len();
    Ok(classify(size_bytes, policy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-core-huge-file-test-{:?}-{}",
            std::thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_policy_uses_the_documented_ten_megabyte_threshold() {
        assert_eq!(HugeFilePolicy::default().threshold_bytes, 10 * 1024 * 1024);
    }

    #[test]
    fn classify_below_threshold_is_normal() {
        let policy = HugeFilePolicy::new(1000);
        let class = classify(999, policy);
        assert!(!class.is_huge());
        assert_eq!(class.size_bytes(), 999);
    }

    #[test]
    fn classify_at_threshold_is_huge() {
        let policy = HugeFilePolicy::new(1000);
        let class = classify(1000, policy);
        assert!(class.is_huge());
    }

    #[test]
    fn classify_above_threshold_is_huge() {
        let policy = HugeFilePolicy::new(1000);
        assert!(classify(1_000_000, policy).is_huge());
    }

    #[test]
    fn threshold_is_configurable_not_fixed() {
        // The same file size classifies differently under two different
        // policies — proving the threshold is a real parameter, not a
        // hardcoded constant with a settable-but-ignored field.
        let lenient = HugeFilePolicy::new(1_000_000);
        let strict = HugeFilePolicy::new(10);
        assert!(!classify(500, lenient).is_huge());
        assert!(classify(500, strict).is_huge());
    }

    #[test]
    fn classify_path_reads_the_real_file_size() {
        let dir = temp_dir();
        let path = dir.join("file.txt");
        fs::write(&path, vec![b'a'; 2048]).unwrap();

        let class = classify_path(&path, HugeFilePolicy::new(1000)).unwrap();
        assert!(class.is_huge());
        assert_eq!(class.size_bytes(), 2048);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn classify_path_normal_for_a_small_file() {
        let dir = temp_dir();
        let path = dir.join("small.txt");
        fs::write(&path, "hi").unwrap();

        let class = classify_path(&path, HugeFilePolicy::default()).unwrap();
        assert!(!class.is_huge());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn classify_path_surfaces_a_missing_file_as_io_error() {
        let err = classify_path(
            "/nonexistent/definitely-not-here.txt",
            HugeFilePolicy::default(),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
