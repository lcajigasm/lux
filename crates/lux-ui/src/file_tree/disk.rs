//! The disk half of each tree operation, as free functions a worker
//! thread can call without the tree. The tree methods with the same names
//! call these, then update their nodes. The desktop sidebar runs these on
//! a worker and applies the result with the `note_*` methods.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::NodeKind;

/// Creates `parent/name` as a file or directory. Never truncates or
/// replaces: an existing entry is `AlreadyExists`.
pub fn create_entry(parent: &Path, name: &str, kind: NodeKind) -> io::Result<PathBuf> {
    check_name(name)?;
    let path = parent.join(name);
    match kind {
        NodeKind::Directory => fs::create_dir(&path)?,
        NodeKind::File => {
            // `create_new`, not `File::create`: the latter truncates a
            // pre-existing file silently, which in a file explorer
            // means data loss. This fails with `AlreadyExists` instead.
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
        }
    }
    Ok(path)
}

/// Renames `from` to `new_name` in the same directory. An existing
/// destination is `AlreadyExists`, not replaced.
pub fn rename_entry(from: &Path, new_name: &str) -> io::Result<PathBuf> {
    check_name(new_name)?;
    let to = from.with_file_name(new_name);
    if to == from {
        return Ok(to);
    }
    // A case-only rename on a case-insensitive volume sees the source as
    // the destination. That is the same file, not a collision.
    let same_file = match (fs::canonicalize(from), fs::canonicalize(&to)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same_file {
        reject_if_exists(&to)?;
    }
    fs::rename(from, &to)?;
    Ok(to)
}

/// Copies the file `src` to `"<stem> copy.<ext>"` beside it, then
/// `"<stem> copy 2.<ext>"` and so on. Never overwrites. Directories are
/// refused.
pub fn duplicate_file(src: &Path) -> io::Result<PathBuf> {
    if src.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "duplicating a folder is not supported",
        ));
    }
    let parent = src.parent().unwrap_or(Path::new("."));
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let ext = src.extension().map(|s| s.to_string_lossy().into_owned());
    for n in 1..1000 {
        let suffix = if n == 1 {
            " copy".to_string()
        } else {
            format!(" copy {n}")
        };
        let name = match &ext {
            Some(ext) => format!("{stem}{suffix}.{ext}"),
            None => format!("{stem}{suffix}"),
        };
        let dst = parent.join(name);
        // `create_new` reserves the name atomically, so a file created
        // between the check and the copy is never replaced.
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dst)
        {
            Ok(_) => {
                if let Err(error) = fs::copy(src, &dst) {
                    let _ = fs::remove_file(&dst);
                    return Err(error);
                }
                return Ok(dst);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many copies already exist",
    ))
}

/// A name the user typed must stay in its folder.
fn check_name(name: &str) -> io::Result<()> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || (cfg!(windows) && name.contains('\\'));
    if bad {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("\"{name}\" is not a valid name"),
        ));
    }
    Ok(())
}

/// Guards `rename`/`move_node` against `fs::rename`'s own POSIX semantics:
/// unlike `File::create`'s `create_new` option, `fs::rename` silently
/// replaces an existing destination rather than erroring — real data loss
/// for a rename/move landing on top of something already there. Checked
/// explicitly rather than relying on `fs::rename` itself, which has no
/// "fail if destination exists" mode. `try_exists` (not `exists`) so a
/// stat error (e.g. permission denied) surfaces as a real `io::Error`
/// instead of silently reading as "doesn't exist".
pub(super) fn reject_if_exists(path: &Path) -> io::Result<()> {
    if path.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        ));
    }
    Ok(())
}

/// Moves `path` into the system trash. On macOS the default `trash` path
/// asks Finder, via AppleScript, and that call fails when the process has
/// no Automation permission (a sandboxed `cargo test`, or a terminal that
/// has never been allowed to control Finder).
/// `NSFileManager.trashItemAtURL` is the same Trash and does not ask.
pub fn move_to_trash(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(path).map_err(io::Error::other)
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(path).map_err(io::Error::other)
    }
}
