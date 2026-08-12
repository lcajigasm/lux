//! Stage/unstage: mutate the repository's index (`.git/index`) to match
//! real files on disk, in-process via `gix` (no shell-out — ADR 0010).
//!
//! Per-hunk staging ([`unstaged_hunks`]/[`stage_hunk`]) reuses this
//! crate's own diff engine (`gutter::diff_hunks`) rather than a second
//! one, applying a single selected hunk onto whatever's currently staged
//! (or `HEAD`'s content, if nothing is), the same base `git add -p` uses.

use std::fmt;
use std::path::Path;

use gix::bstr::BStr;
use gix::index::entry;

use crate::gutter::{Hunk, diff_hunks};

/// Failure staging, unstaging, or reading a repository's index.
#[derive(Debug)]
pub enum StagingError {
    BareRepository,
    NonUtf8Path,
    OpenIndex(Box<dyn std::error::Error + Send + Sync>),
    ReadFile(std::io::Error),
    ReadBlob(Box<dyn std::error::Error + Send + Sync>),
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    Metadata(std::io::Error),
    Stat(std::time::SystemTimeError),
    WriteBlob(Box<dyn std::error::Error + Send + Sync>),
    WriteIndex(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for StagingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StagingError::BareRepository => write!(f, "repository has no working directory"),
            StagingError::NonUtf8Path => write!(f, "path is not valid UTF-8"),
            StagingError::OpenIndex(e) => write!(f, "failed to open index: {e}"),
            StagingError::ReadFile(e) => write!(f, "failed to read file: {e}"),
            StagingError::ReadBlob(e) => write!(f, "failed to read blob: {e}"),
            StagingError::ReadTree(e) => write!(f, "failed to read tree: {e}"),
            StagingError::Metadata(e) => write!(f, "failed to read file metadata: {e}"),
            StagingError::Stat(e) => write!(f, "failed to convert file metadata: {e}"),
            StagingError::WriteBlob(e) => write!(f, "failed to write blob: {e}"),
            StagingError::WriteIndex(e) => write!(f, "failed to write index: {e}"),
        }
    }
}

impl std::error::Error for StagingError {}

/// Opens `repo`'s index, or an empty one if `.git/index` doesn't exist yet
/// (a fresh repository nothing has ever been staged in).
pub(crate) fn open_or_create_index(
    repo: &gix::Repository,
) -> Result<gix::index::File, StagingError> {
    match repo.open_index() {
        Ok(index) => Ok(index),
        Err(gix::worktree::open_index::Error::IndexFile(gix::index::file::init::Error::Io(e)))
            if e.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(gix::index::File::from_state(
                gix::index::State::new(repo.object_hash()),
                repo.index_path(),
            ))
        }
        Err(e) => Err(StagingError::OpenIndex(Box::new(e))),
    }
}

fn path_to_bstr(path: &Path) -> Result<&BStr, StagingError> {
    path.to_str()
        .map(Into::into)
        .ok_or(StagingError::NonUtf8Path)
}

pub(crate) fn mode_for(metadata: &gix::index::fs::Metadata) -> entry::Mode {
    if metadata.is_executable() {
        entry::Mode::FILE_EXECUTABLE
    } else {
        entry::Mode::FILE
    }
}

/// Writes `content` as a blob and records it in the index at `rel`,
/// stage 0, replacing any existing entry for the same path. `absolute`
/// supplies the stat info (mtime, size, ...) cached alongside the entry.
fn write_staged_entry(
    repo: &gix::Repository,
    rel: &BStr,
    absolute: &Path,
    content: &[u8],
) -> Result<(), StagingError> {
    let blob_id = repo
        .write_blob(content)
        .map_err(|e| StagingError::WriteBlob(Box::new(e)))?;
    let metadata =
        gix::index::fs::Metadata::from_path_no_follow(absolute).map_err(StagingError::Metadata)?;
    let stat = entry::Stat::from_fs(&metadata).map_err(StagingError::Stat)?;
    let mode = mode_for(&metadata);

    let mut index = open_or_create_index(repo)?;
    index.remove_entries(|_, entry_path, _| entry_path == rel);
    index.dangerously_push_entry(stat, blob_id.detach(), entry::Flags::empty(), mode, rel);
    index.sort_entries();
    index
        .write(gix::index::write::Options::default())
        .map_err(|e| StagingError::WriteIndex(Box::new(e)))?;
    Ok(())
}

/// Stages `path` (relative to the repository's working directory): writes
/// its current on-disk content as a blob and records it in the index at
/// stage 0, replacing any existing entry for the same path. `path` must
/// name a real file in the working tree.
pub fn stage(repo: &gix::Repository, path: &Path) -> Result<(), StagingError> {
    let work_dir = repo.workdir().ok_or(StagingError::BareRepository)?;
    let absolute = work_dir.join(path);
    let rel = path_to_bstr(path)?;
    let content = std::fs::read(&absolute).map_err(StagingError::ReadFile)?;
    write_staged_entry(repo, rel, &absolute, &content)
}

/// Unstages `path`: removes its index entry entirely. The file itself is
/// left untouched on disk (matches `git restore --staged`, not `git
/// checkout`). Not an error if `path` wasn't staged.
pub fn unstage(repo: &gix::Repository, path: &Path) -> Result<(), StagingError> {
    let rel = path_to_bstr(path)?;
    let mut index = open_or_create_index(repo)?;
    index.remove_entries(|_, entry_path, _| entry_path == rel);
    index
        .write(gix::index::write::Options::default())
        .map_err(|e| StagingError::WriteIndex(Box::new(e)))?;
    Ok(())
}

/// Returns whether `path` currently has an index entry.
pub fn is_staged(repo: &gix::Repository, path: &Path) -> Result<bool, StagingError> {
    let rel = path_to_bstr(path)?;
    let index = open_or_create_index(repo)?;
    Ok(index.entry_by_path(rel).is_some())
}

/// `path`'s currently staged content, or its content at `HEAD` if nothing
/// is staged for it yet, or an empty string if it's absent from both (a
/// new, untracked file) — the base `git add -p` diffs the working copy
/// against to find hunks to offer.
fn staged_or_head_content(
    repo: &gix::Repository,
    path: &Path,
    rel: &BStr,
) -> Result<String, StagingError> {
    let index = open_or_create_index(repo)?;
    if let Some(entry) = index.entry_by_path(rel) {
        let blob = repo
            .find_blob(entry.id)
            .map_err(|e| StagingError::ReadBlob(Box::new(e)))?;
        return Ok(String::from_utf8_lossy(&blob.data).into_owned());
    }

    let tree_id = repo
        .head_tree_id_or_empty()
        .map_err(|e| StagingError::ReadTree(Box::new(e)))?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| StagingError::ReadTree(Box::new(e)))?;
    match tree
        .lookup_entry_by_path(path)
        .map_err(|e| StagingError::ReadTree(Box::new(e)))?
    {
        Some(entry) => {
            let blob = repo
                .find_blob(entry.object_id())
                .map_err(|e| StagingError::ReadBlob(Box::new(e)))?;
            Ok(String::from_utf8_lossy(&blob.data).into_owned())
        }
        None => Ok(String::new()),
    }
}

/// The hunks between `path`'s currently staged content (or `HEAD`'s, if
/// nothing is staged) and its working-copy content — what a "stage this
/// hunk" UI lists to choose from. Pass one of the returned hunks to
/// [`stage_hunk`] to stage just that change.
pub fn unstaged_hunks(repo: &gix::Repository, path: &Path) -> Result<Vec<Hunk>, StagingError> {
    let work_dir = repo.workdir().ok_or(StagingError::BareRepository)?;
    let base = staged_or_head_content(repo, path, path_to_bstr(path)?)?;
    let working = std::fs::read_to_string(work_dir.join(path)).map_err(StagingError::ReadFile)?;
    Ok(diff_hunks(&base, &working))
}

/// Stages a single hunk of `path`'s unstaged changes: applies just `hunk`
/// on top of `path`'s currently staged content (or `HEAD`'s, if nothing
/// is staged yet) and records the result in the index — matching `git add
/// -p`'s per-hunk staging, not a whole-file replace. `hunk` must be one
/// [`unstaged_hunks`] returned for the same `path`; its `before_*` fields
/// are read as positions in that same base content.
pub fn stage_hunk(repo: &gix::Repository, path: &Path, hunk: &Hunk) -> Result<(), StagingError> {
    let work_dir = repo.workdir().ok_or(StagingError::BareRepository)?;
    let absolute = work_dir.join(path);
    let rel = path_to_bstr(path)?;

    let base = staged_or_head_content(repo, path, rel)?;
    let working = std::fs::read_to_string(&absolute).map_err(StagingError::ReadFile)?;
    let new_content = apply_hunk(&base, &working, hunk);

    write_staged_entry(repo, rel, &absolute, new_content.as_bytes())
}

/// Replaces `hunk`'s `before` range in `base` with its `after` range from
/// `working`, leaving everything outside the hunk untouched.
fn apply_hunk(base: &str, working: &str, hunk: &Hunk) -> String {
    let base_lines: Vec<&str> = base.lines().collect();
    let working_lines: Vec<&str> = working.lines().collect();

    let before_start = (hunk.before_start - 1) as usize;
    let before_end = before_start + hunk.before_len as usize;
    let after_start = (hunk.after_start - 1) as usize;
    let after_end = after_start + hunk.after_len as usize;

    let mut result_lines = Vec::with_capacity(base_lines.len());
    result_lines.extend_from_slice(&base_lines[..before_start]);
    result_lines.extend_from_slice(&working_lines[after_start..after_end]);
    result_lines.extend_from_slice(&base_lines[before_end..]);

    let mut result = result_lines.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init temp repo")
    }

    #[test]
    fn staging_a_new_file_adds_it_to_the_index() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();

        assert!(!is_staged(&repo, Path::new("a.rs")).unwrap());
        stage(&repo, Path::new("a.rs")).unwrap();
        assert!(is_staged(&repo, Path::new("a.rs")).unwrap());
    }

    #[test]
    fn staging_twice_replaces_the_entry_rather_than_duplicating_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let path = dir.path().join("a.rs");

        fs::write(&path, "one\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        fs::write(&path, "two\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();

        let index = repo.open_index().expect("index exists");
        let matches = index
            .entries()
            .iter()
            .filter(|e| e.path(&index) == "a.rs")
            .count();
        assert_eq!(matches, 1, "no duplicate entries for the same path");

        let entry = index.entry_by_path("a.rs".into()).expect("staged");
        let blob = repo.find_blob(entry.id).expect("blob exists");
        assert_eq!(blob.data, b"two\n");
    }

    #[test]
    fn unstaging_removes_the_entry_but_not_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        assert!(is_staged(&repo, Path::new("a.rs")).unwrap());

        unstage(&repo, Path::new("a.rs")).unwrap();

        assert!(!is_staged(&repo, Path::new("a.rs")).unwrap());
        assert!(
            dir.path().join("a.rs").exists(),
            "unstage doesn't touch the working copy"
        );
    }

    #[test]
    fn unstaging_an_unstaged_path_is_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        unstage(&repo, Path::new("never-staged.rs")).unwrap();
    }

    #[test]
    fn staging_persists_across_a_fresh_index_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();

        // A brand new `Repository` handle, forcing a real read of `.git/index` from disk.
        let reopened = gix::open(dir.path()).expect("reopen repo");
        assert!(is_staged(&reopened, Path::new("a.rs")).unwrap());
    }

    #[test]
    fn unstaged_hunks_on_an_untracked_file_diffs_against_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\ntwo\n").unwrap();

        let hunks = unstaged_hunks(&repo, Path::new("a.rs")).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].after_len, 2);
    }

    #[test]
    fn stage_hunk_stages_only_the_selected_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\ntwo\nthree\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        crate::commit::commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();

        // Two independent, non-adjacent edits far enough apart to diff as separate hunks.
        fs::write(dir.path().join("a.rs"), "ONE\ntwo\nTHREE\n").unwrap();
        let hunks = unstaged_hunks(&repo, Path::new("a.rs")).unwrap();
        assert_eq!(hunks.len(), 2, "two separate edits");

        stage_hunk(&repo, Path::new("a.rs"), &hunks[0]).unwrap();

        let index = repo.open_index().unwrap();
        let entry = index.entry_by_path("a.rs".into()).unwrap();
        let blob = repo.find_blob(entry.id).unwrap();
        assert_eq!(
            blob.data, b"ONE\ntwo\nthree\n",
            "only the first hunk is staged, the second edit isn't"
        );
    }

    #[test]
    fn stage_hunk_twice_stages_the_whole_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\ntwo\nthree\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        crate::commit::commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "ONE\ntwo\nTHREE\n").unwrap();
        let hunks = unstaged_hunks(&repo, Path::new("a.rs")).unwrap();
        assert_eq!(hunks.len(), 2);
        stage_hunk(&repo, Path::new("a.rs"), &hunks[0]).unwrap();

        // Recompute hunks against the now-partially-staged content for the second hunk.
        let remaining = unstaged_hunks(&repo, Path::new("a.rs")).unwrap();
        assert_eq!(remaining.len(), 1, "one edit already staged, one left");
        stage_hunk(&repo, Path::new("a.rs"), &remaining[0]).unwrap();

        let index = repo.open_index().unwrap();
        let entry = index.entry_by_path("a.rs".into()).unwrap();
        let blob = repo.find_blob(entry.id).unwrap();
        assert_eq!(
            blob.data, b"ONE\ntwo\nTHREE\n",
            "both hunks staged one at a time equal the whole file"
        );
    }

    #[test]
    fn stage_hunk_on_an_untracked_file_stages_a_pure_insertion() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("new.rs"), "brand new content\n").unwrap();

        let hunks = unstaged_hunks(&repo, Path::new("new.rs")).unwrap();
        assert_eq!(hunks.len(), 1);
        stage_hunk(&repo, Path::new("new.rs"), &hunks[0]).unwrap();

        assert!(is_staged(&repo, Path::new("new.rs")).unwrap());
        let index = repo.open_index().unwrap();
        let entry = index.entry_by_path("new.rs".into()).unwrap();
        let blob = repo.find_blob(entry.id).unwrap();
        assert_eq!(blob.data, b"brand new content\n");
    }
}
