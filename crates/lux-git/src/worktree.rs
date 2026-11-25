//! One walk of the working tree that sorts every path git does not track
//! into untracked or ignored, plus the index facts `git status` needs to
//! tell a staged new file and a merge conflict apart from an untracked one.
//!
//! Ignore rules come from `gix`'s own exclude stack (`.gitignore` files,
//! `.git/info/exclude`, `core.excludesFile`), so this agrees with
//! `git status --ignored` rather than with an approximation (ADR 0010). An
//! ignored directory is reported once and never entered: `target/` costs
//! one lookup, not a walk of every build artifact.

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
            } else if is_dir {
                pending.push(rel);
            } else {
                out.files.push(rel);
            }
        }
    }
    out.files.sort();
    out.ignored.sort();
    Ok(out)
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
