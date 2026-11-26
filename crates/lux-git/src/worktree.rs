//! One walk of the working tree that sorts every path git does not track
//! into untracked or ignored, plus the index facts `git status` needs to
//! tell a staged new file and a merge conflict apart from an untracked one.
//!
//! Ignore rules come from `gix`'s own exclude stack (`.gitignore` files,
//! `.git/info/exclude`, `core.excludesFile`), so this agrees with
//! `git status --ignored` rather than with an approximation (ADR 0010). An
//! ignored directory is reported once and never entered: `target/` costs
//! one lookup, not a walk of every build artifact. An untracked directory
//! is reported once too, as `git status` does by default: the walk enters
//! it only until it finds the first file that is not ignored.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

use crate::diff_panel::DiffPanelError;

/// What the index says about paths, independent of the working tree.
#[derive(Debug, Default)]
pub(crate) struct IndexFacts {
    /// Paths with a stage-0 entry: added to the index.
    pub(crate) staged: BTreeSet<PathBuf>,
    /// Paths with a stage 1–3 entry: an unresolved merge conflict.
    pub(crate) conflicted: BTreeSet<PathBuf>,
}

/// Paths the walk found that `HEAD` does not track.
#[derive(Debug, Default)]
pub(crate) struct Untracked {
    /// Files that are neither tracked nor ignored. Staged new files are
    /// included here too; the caller tells them apart with [`IndexFacts`].
    pub(crate) files: Vec<PathBuf>,
    /// Untracked directories holding no tracked or staged path and at least
    /// one file that is not ignored, collapsed to the outermost such
    /// directory. Their contents are not listed in `files`.
    pub(crate) dirs: Vec<PathBuf>,
    /// Ignored files and directories, collapsed to the outermost ignored
    /// path, relative to the working directory.
    pub(crate) ignored: Vec<PathBuf>,
}

fn index_error(e: impl std::error::Error + Send + Sync + 'static) -> DiffPanelError {
    DiffPanelError::Index(Box::new(e))
}

pub(crate) fn index_facts(
    repo: &gix::Repository,
) -> Result<(gix::worktree::Index, IndexFacts), DiffPanelError> {
    let index = repo.index_or_empty().map_err(index_error)?;
    let mut facts = IndexFacts::default();
    for entry in index.entries() {
        let path = PathBuf::from(entry.path(&index).to_str_lossy().into_owned());
        if entry.stage_raw() == 0 {
            facts.staged.insert(path);
        } else {
            facts.conflicted.insert(path);
        }
    }
    Ok((index, facts))
}

/// Walks `work_dir`, skipping every path in `tracked` (the caller compares
/// those against `HEAD` itself). `keep` holds paths that may never be
/// reported ignored because git tracks them or something below them.
pub(crate) fn walk(
    repo: &gix::Repository,
    index: &gix::index::State,
    tracked: &BTreeSet<PathBuf>,
    keep: &BTreeSet<PathBuf>,
) -> Result<Untracked, DiffPanelError> {
    let work_dir = repo.workdir().ok_or(DiffPanelError::BareRepository)?;
    let mut excludes = repo
        .excludes(
            index,
            None,
            gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
        )
        .map_err(index_error)?;
    let mut out = Untracked::default();
    let mut pending = vec![PathBuf::new()];
    while let Some(rel_dir) = pending.pop() {
        let entries = match std::fs::read_dir(work_dir.join(&rel_dir)) {
            Ok(entries) => entries,
            // A directory removed mid-walk is no longer part of the tree.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(DiffPanelError::Io(e)),
        };
        for entry in entries {
            let entry = entry.map_err(DiffPanelError::Io)?;
            if entry.file_name() == ".git" {
                continue;
            }
            let rel = rel_dir.join(entry.file_name());
            let is_dir = entry.file_type().map_err(DiffPanelError::Io)?.is_dir();
            if !is_dir && tracked.contains(&rel) {
                continue;
            }
            let mode = if is_dir {
                gix::index::entry::Mode::DIR
            } else {
                gix::index::entry::Mode::FILE
            };
            let excluded = excludes
                .at_path(&rel, Some(mode))
                .map_err(DiffPanelError::Io)?
                .is_excluded();
            if excluded && !keep.contains(&rel) {
                out.ignored.push(rel);
            } else if is_dir && keep.contains(&rel) {
                pending.push(rel);
            } else if is_dir {
                match classify_untracked_dir(work_dir, &rel, &mut excludes)? {
                    DirContents::Untracked => out.dirs.push(rel),
                    DirContents::OnlyIgnored => out.ignored.push(rel),
                    DirContents::Empty => {}
                }
            } else {
                out.files.push(rel);
            }
        }
    }
    out.files.sort();
    out.dirs.sort();
    out.ignored.sort();
    Ok(out)
}

enum DirContents {
    /// At least one file below is neither tracked nor ignored.
    Untracked,
    /// Files below, every one of them ignored.
    OnlyIgnored,
    Empty,
}

/// Looks inside an untracked directory just far enough to tell how
/// `git status --ignored` shows it, stopping at the first file that is not
/// ignored. Ignored subdirectories are not entered.
fn classify_untracked_dir(
    work_dir: &Path,
    rel: &Path,
    excludes: &mut gix::AttributeStack<'_>,
) -> Result<DirContents, DiffPanelError> {
    let mut seen_ignored = false;
    let mut pending = vec![rel.to_path_buf()];
    while let Some(rel_dir) = pending.pop() {
        let entries = match std::fs::read_dir(work_dir.join(&rel_dir)) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(DiffPanelError::Io(e)),
        };
        for entry in entries {
            let entry = entry.map_err(DiffPanelError::Io)?;
            if entry.file_name() == ".git" {
                continue;
            }
            let rel = rel_dir.join(entry.file_name());
            let is_dir = entry.file_type().map_err(DiffPanelError::Io)?.is_dir();
            let mode = if is_dir {
                gix::index::entry::Mode::DIR
            } else {
                gix::index::entry::Mode::FILE
            };
            if excludes
                .at_path(&rel, Some(mode))
                .map_err(DiffPanelError::Io)?
                .is_excluded()
            {
                seen_ignored = true;
            } else if is_dir {
                pending.push(rel);
            } else {
                return Ok(DirContents::Untracked);
            }
        }
    }
    Ok(if seen_ignored {
        DirContents::OnlyIgnored
    } else {
        DirContents::Empty
    })
}

/// Every tracked path plus each of its parent directories.
pub(crate) fn with_parents<'a>(paths: impl Iterator<Item = &'a PathBuf>) -> BTreeSet<PathBuf> {
    let mut keep = BTreeSet::new();
    for path in paths {
        let mut current: Option<&Path> = Some(path.as_path());
        while let Some(p) = current {
            if p.as_os_str().is_empty() || !keep.insert(p.to_path_buf()) {
                break;
            }
            current = p.parent();
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::diff_panel::{ChangeStatus, ChangedFile, worktree_status};

    #[test]
    fn an_untracked_directory_is_reported_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let repo = gix::init(root).unwrap();
        fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "lib\n").unwrap();
        crate::staging::stage(&repo, Path::new(".gitignore")).unwrap();
        crate::staging::stage(&repo, Path::new("src/lib.rs")).unwrap();
        crate::commit::commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();

        fs::create_dir_all(root.join("scratch/deep")).unwrap();
        fs::write(root.join("scratch/deep/a.txt"), "a\n").unwrap();
        fs::write(root.join("scratch/b.txt"), "b\n").unwrap();
        fs::create_dir_all(root.join("src/notes")).unwrap();
        fs::write(root.join("src/notes/todo.txt"), "todo\n").unwrap();
        fs::create_dir_all(root.join("logs")).unwrap();
        fs::write(root.join("logs/run.log"), "noise\n").unwrap();
        fs::create_dir_all(root.join("empty")).unwrap();

        let status = worktree_status(&repo).unwrap();
        let untracked = |path: &str| ChangedFile {
            path: path.into(),
            status: ChangeStatus::Untracked,
        };
        assert_eq!(
            status.changes,
            vec![untracked("scratch"), untracked("src/notes")]
        );
        assert_eq!(
            status.untracked_dirs,
            vec![PathBuf::from("scratch"), PathBuf::from("src/notes")]
        );
        assert_eq!(status.ignored, vec![PathBuf::from("logs")]);
    }
}
