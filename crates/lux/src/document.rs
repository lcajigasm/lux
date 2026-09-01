//! Open/save helpers for the desktop binary — thin wrappers around
//! `lux_core::buffer::Buffer`'s real atomic save and policy-aware open.
//! Kept separate from `app` so the I/O policy stays unit-testable without
//! spinning up a GPU window (and so `app.rs` stays under the 600-line cap).

use std::path::{Path, PathBuf};

use lux_core::buffer::Buffer;
use lux_core::huge_file::HugeFilePolicy;

/// Result of opening a path with the editor's huge/binary policy applied.
#[derive(Debug)]
pub struct OpenedDocument {
    pub buffer: Buffer,
    pub path: PathBuf,
    /// Huge files open read-only in the shell (see `Buffer::open_with_policy`).
    pub read_only: bool,
}

/// Opens `path` with [`HugeFilePolicy::default`]. Binary files are refused
/// (`io::ErrorKind::InvalidData`); huge files load fully but come back with
/// `read_only: true` so the shell can block edits without lying about a
/// virtualized view that does not exist.
pub fn open_path(path: impl AsRef<Path>) -> Result<OpenedDocument, String> {
    let path = path.as_ref();
    let buffer = Buffer::open_with_policy(path, HugeFilePolicy::default()).map_err(|err| {
        format!(
            "failed to open {}: {err}",
            path.display()
        )
    })?;
    let read_only = buffer.is_huge_file_mode();
    Ok(OpenedDocument {
        buffer,
        path: path.to_path_buf(),
        read_only,
    })
}

/// Atomic save + `.bak` via [`Buffer::save`].
pub fn save_buffer(buffer: &Buffer, path: impl AsRef<Path>) -> Result<(), String> {
    let path = path.as_ref();
    buffer
        .save(path)
        .map_err(|err| format!("failed to save {}: {err}", path.display()))
}

/// Window title reflecting path, dirty bit, and read-only huge-file mode.
pub fn window_title(path: Option<&Path>, dirty: bool, read_only: bool) -> String {
    let name = path
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");
    let mut title = String::new();
    if dirty {
        title.push('•');
        title.push(' ');
    }
    title.push_str(name);
    if read_only {
        title.push_str(" [read-only]");
    }
    title.push_str(" — Lux");
    title
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("lux-g02-{tag}-{nanos}.txt"))
    }

    #[test]
    fn open_save_round_trip_persists_utf8_bytes() {
        let path = temp_path("round");
        fs::write(&path, "alpha\nbeta\n").expect("seed");
        let opened = open_path(&path).expect("open");
        assert!(!opened.read_only);
        assert!(opened.buffer.text().contains("alpha"));

        let mut buffer = opened.buffer;
        buffer.insert(buffer.len_chars(), "gamma\n");
        save_buffer(&buffer, &path).expect("save");
        let disk = fs::read_to_string(&path).expect("reread");
        assert!(disk.ends_with("gamma\n"));
        // `Buffer::save` copies the previous bytes to `{filename}.bak`.
        let bak = {
            let mut name = path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            name.push_str(".bak");
            path.with_file_name(name)
        };
        assert!(bak.exists(), "atomic save must leave a .bak sibling");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(bak);
    }

    #[test]
    fn binary_file_is_refused_without_fake_preview() {
        let path = temp_path("bin");
        fs::write(&path, [0x00_u8, 0x01, 0x02, 0xff, 0xfe]).expect("seed binary");
        let err = open_path(&path).expect_err("binary must fail");
        assert!(
            err.contains("failed to open"),
            "error must name the path: {err}"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn title_marks_dirty_and_read_only() {
        let path = PathBuf::from("/tmp/demo.rs");
        assert_eq!(
            window_title(Some(&path), true, false),
            "• demo.rs — Lux"
        );
        assert_eq!(
            window_title(Some(&path), false, true),
            "demo.rs [read-only] — Lux"
        );
        assert_eq!(window_title(None, true, false), "• Untitled — Lux");
    }
}
