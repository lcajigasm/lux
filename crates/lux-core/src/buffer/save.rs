//! Atomic replacement with unique temporary files and recoverable backups.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use super::{Buffer, encoding};

impl Buffer {
    /// Writes an independent rope snapshot on the caller's thread. Call on
    /// an I/O worker: encoding, backup creation and synchronization block.
    /// Existing symlinks retain their identity and their target is updated.
    /// Existing file permissions are retained. Temporary files are created
    /// exclusively beside the target and removed on failure.
    ///
    /// The old content is atomically installed at `path + ".bak"` before
    /// replacing the document. Unix also synchronizes the parent directory.
    /// Concurrent external writers are not locked by this primitive; callers
    /// must handle external-change conflicts before initiating a save.
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        let resolved;
        let path = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                resolved = fs::canonicalize(path)?;
                resolved.as_path()
            }
            Ok(_) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => path,
            Err(error) => return Err(error),
        };
        let original = match fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "save target is not a regular file",
                    ));
                }
                if metadata.permissions().readonly() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "save target is read-only",
                    ));
                }
                Some(metadata)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::Builder::new()
            .prefix(".lux-save-")
            .tempfile_in(dir)?;
        let text = self.text();
        if !super::encoding::can_encode(&text, self.encoding) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "document contains characters that cannot be saved in its encoding",
            ));
        }
        let bytes = encoding::encode(&text, self.encoding);
        temporary.write_all(&bytes)?;
        if let Some(metadata) = &original {
            temporary
                .as_file()
                .set_permissions(metadata.permissions())?;
        }
        temporary.as_file().sync_all()?;

        if let Some(metadata) = original {
            let backup_path = backup_path(path)?;
            let mut backup = tempfile::Builder::new()
                .prefix(".lux-backup-")
                .tempfile_in(dir)?;
            io::copy(&mut File::open(path)?, backup.as_file_mut())?;
            backup.as_file().set_permissions(metadata.permissions())?;
            backup.as_file().sync_all()?;
            backup.persist(&backup_path).map_err(|error| error.error)?;
        }
        temporary.persist(path).map_err(|error| error.error)?;
        #[cfg(unix)]
        File::open(dir)?.sync_all()?;
        Ok(())
    }
}

fn backup_path(path: &Path) -> io::Result<std::path::PathBuf> {
    let mut name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "save target has no file name"))?
        .to_os_string();
    name.push(".bak");
    Ok(path.with_file_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrepresentable_latin1_edit_does_not_silently_replace_characters() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latin1");
        fs::write(&path, [0xe9]).unwrap();
        let mut buffer = Buffer::open(&path).unwrap();
        buffer.insert(1, "猫");
        assert_eq!(
            buffer.save(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&path).unwrap(), [0xe9]);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn save_does_not_truncate_an_existing_legacy_temporary_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        let unrelated = dir.path().join(".note.txt.lux-tmp");
        fs::write(&unrelated, b"unrelated data").unwrap();
        Buffer::from_bytes(b"new").unwrap().save(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read(unrelated).unwrap(), b"unrelated data");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn failed_backup_preserves_original_and_cleans_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        fs::write(&path, b"old").unwrap();
        fs::create_dir(dir.path().join("note.txt.bak")).unwrap();
        assert!(Buffer::from_bytes(b"new").unwrap().save(&path).is_err());
        assert_eq!(fs::read(path).unwrap(), b"old");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn a_snapshot_can_be_saved_while_the_live_buffer_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        let mut live = Buffer::from_bytes(b"snapshot").unwrap();
        let snapshot = live.clone();
        live.insert(8, " + edit");
        std::thread::scope(|scope| {
            scope.spawn(|| snapshot.save(&path).unwrap());
        });
        assert_eq!(fs::read(path).unwrap(), b"snapshot");
        assert_eq!(live.text(), "snapshot + edit");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_and_executable_permissions_survive_save() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("script");
        let link = dir.path().join("link");
        fs::write(&target, b"old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o750)).unwrap();
        symlink(&target, &link).unwrap();
        Buffer::from_bytes(b"new").unwrap().save(&link).unwrap();
        assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert_eq!(
            fs::metadata(target).unwrap().permissions().mode() & 0o777,
            0o750
        );
        assert_eq!(fs::read(dir.path().join("script.bak")).unwrap(), b"old");
    }

    #[cfg(unix)]
    #[test]
    fn backup_does_not_follow_a_symlink() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note");
        let backup = backup_path(&path).unwrap();
        let unrelated = dir.path().join("unrelated");
        fs::write(&path, b"old").unwrap();
        fs::write(&unrelated, b"keep").unwrap();
        symlink(&unrelated, &backup).unwrap();
        Buffer::from_bytes(b"new").unwrap().save(&path).unwrap();
        assert_eq!(fs::read(backup).unwrap(), b"old");
        assert_eq!(fs::read(unrelated).unwrap(), b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn backup_path_preserves_non_utf8_bytes_without_requiring_filesystem_support() {
        use std::ffi::OsString;
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let path = std::path::PathBuf::from(OsString::from_vec(b"note-\xff".to_vec()));
        assert_eq!(
            backup_path(&path).unwrap().as_os_str().as_bytes(),
            b"note-\xff.bak"
        );
    }
}
