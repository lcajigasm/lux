//! Compares every path `HEAD` tracks against the working tree, the way
//! `git status` does: the index's cached stat data (mtime, ctime, size,
//! inode, ...) answers for every file whose stat still matches, and only a
//! stat-dirty file is read and compared byte for byte.
//!
//! A file whose stat matches its stage-0 index entry holds exactly that
//! entry's blob, so it differs from `HEAD` only if the index entry's id
//! differs from the `HEAD` blob id — no read needed. A file written in the
//! same second the index was (racy git) does not count as matching.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

use crate::diff_panel::{ChangeStatus, ChangedFile, DiffPanelError};

#[cfg(test)]
thread_local! {
    /// Working-tree files read for a content comparison on this thread.
    pub(crate) static CONTENT_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Pushes one [`ChangedFile`] per path in `head_blobs` whose working-tree
/// state differs from its `HEAD` blob.
pub(crate) fn compare(
    repo: &gix::Repository,
    work_dir: &Path,
    head_blobs: &BTreeMap<PathBuf, gix::ObjectId>,
    index: &gix::index::State,
    changed: &mut Vec<ChangedFile>,
) -> Result<(), DiffPanelError> {
    let options = repo
        .stat_options()
        .map_err(|e| DiffPanelError::Index(Box::new(e)))?;
    let index_time = index.timestamp();
    for (rel, oid) in head_blobs {
        let absolute = work_dir.join(rel);
        let metadata = match gix::index::fs::Metadata::from_path_no_follow(&absolute) {
            Ok(metadata) => metadata,
            Err(e) if is_gone(&e) => {
                push(changed, rel, ChangeStatus::Deleted);
                continue;
            }
            Err(e) => return Err(DiffPanelError::Io(e)),
        };
        // A directory standing where a tracked file was is the same fact as
        // a missing file: the blob is gone. `git status` reports that path
        // deleted and lists the files inside.
        if metadata.is_dir() {
            push(changed, rel, ChangeStatus::Deleted);
            continue;
        }
        let cached = rel
            .to_str()
            .and_then(|path| {
                index.entry_by_path_and_stage(
                    path.as_bytes().as_bstr(),
                    gix::index::entry::Stage::Unconflicted,
                )
            })
            .filter(|entry| {
                gix::index::entry::Stat::from_fs(&metadata).is_ok_and(|disk| {
                    entry.stat.matches(&disk, options) && !entry.stat.is_racy(index_time, options)
                })
            });
        if let Some(entry) = cached {
            if entry.id != *oid {
                push(changed, rel, ChangeStatus::Modified);
            }
            continue;
        }
        if differs_on_disk(repo, &absolute, *oid)? {
            push(changed, rel, ChangeStatus::Modified);
        }
    }
    Ok(())
}

/// Whether the file's bytes differ from the blob. A file that vanished
/// between the stat and the read counts as changed; the next run reports it
/// deleted.
fn differs_on_disk(
    repo: &gix::Repository,
    absolute: &Path,
    oid: gix::ObjectId,
) -> Result<bool, DiffPanelError> {
    #[cfg(test)]
    CONTENT_READS.with(|reads| reads.set(reads.get() + 1));
    let disk_bytes = match std::fs::read(absolute) {
        Ok(bytes) => bytes,
        Err(e) if is_gone(&e) => return Ok(true),
        Err(e) => return Err(DiffPanelError::Io(e)),
    };
    let blob = repo
        .find_blob(oid)
        .map_err(|e| DiffPanelError::ReadBlob(Box::new(e)))?;
    Ok(blob.data != disk_bytes)
}

fn is_gone(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound
            | std::io::ErrorKind::IsADirectory
            | std::io::ErrorKind::NotADirectory
    )
}

fn push(changed: &mut Vec<ChangedFile>, rel: &Path, status: ChangeStatus) {
    changed.push(ChangedFile {
        path: rel.to_path_buf(),
        status,
    });
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use super::CONTENT_READS;
    use crate::diff_panel::{ChangeStatus, ChangedFile, worktree_status};

    /// Backdates `path` so the index written next is newer than it: a stat
    /// taken in the same second as the index write is racy and never trusted.
    fn backdate(path: &Path) {
        let file = fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000))
            .unwrap();
    }

    #[test]
    fn only_stat_dirty_tracked_files_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let repo = gix::init(dir.path()).unwrap();
        for name in ["same.rs", "edited.rs", "staged.rs"] {
            fs::write(dir.path().join(name), format!("{name}\n")).unwrap();
            backdate(&dir.path().join(name));
            crate::staging::stage(&repo, Path::new(name)).unwrap();
        }
        crate::commit::commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();

        // Stat-dirty: the only file whose content has to be compared.
        fs::write(dir.path().join("edited.rs"), "edited, longer\n").unwrap();
        // Stat-clean against an index entry that differs from `HEAD`.
        fs::write(dir.path().join("staged.rs"), "staged, longer\n").unwrap();
        backdate(&dir.path().join("staged.rs"));
        crate::staging::stage(&repo, Path::new("staged.rs")).unwrap();

        CONTENT_READS.with(|reads| reads.set(0));
        let status = worktree_status(&repo).unwrap();
        assert_eq!(
            status.changes,
            vec![
                ChangedFile {
                    path: "edited.rs".into(),
                    status: ChangeStatus::Modified
                },
                ChangedFile {
                    path: "staged.rs".into(),
                    status: ChangeStatus::Modified
                },
            ]
        );
        assert_eq!(
            CONTENT_READS.with(std::cell::Cell::get),
            1,
            "same.rs and staged.rs match their index stat and must not be read"
        );
    }

    #[test]
    fn a_file_without_a_trusted_stat_is_still_compared() {
        let dir = tempfile::tempdir().unwrap();
        let repo = gix::init(dir.path()).unwrap();
        fs::write(dir.path().join("a.rs"), "one\n").unwrap();
        crate::staging::stage(&repo, Path::new("a.rs")).unwrap();
        crate::commit::commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();
        // Same size, possibly the same second: only the bytes tell.
        fs::write(dir.path().join("a.rs"), "two\n").unwrap();

        let status = worktree_status(&repo).unwrap();
        assert_eq!(
            status.changes,
            vec![ChangedFile {
                path: "a.rs".into(),
                status: ChangeStatus::Modified
            }]
        );
    }
}
