//! Stage/unstage: mutate the repository's index (`.git/index`) to match
//! real files on disk, in-process via `gix` (no shell-out — ADR 0010).

use std::fmt;
use std::path::Path;

use gix::bstr::BStr;
use gix::index::entry;

/// Failure staging, unstaging, or reading a repository's index.
#[derive(Debug)]
pub enum StagingError {
    BareRepository,
    NonUtf8Path,
    OpenIndex(Box<dyn std::error::Error + Send + Sync>),
    ReadFile(std::io::Error),
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

/// Stages `path` (relative to the repository's working directory): writes
/// its current on-disk content as a blob and records it in the index at
/// stage 0, replacing any existing entry for the same path. `path` must
/// name a real file in the working tree.
pub fn stage(repo: &gix::Repository, path: &Path) -> Result<(), StagingError> {
    let work_dir = repo.workdir().ok_or(StagingError::BareRepository)?;
    let absolute = work_dir.join(path);
    let rel = path_to_bstr(path)?;

    let content = std::fs::read(&absolute).map_err(StagingError::ReadFile)?;
    let blob_id = repo
        .write_blob(&content)
        .map_err(|e| StagingError::WriteBlob(Box::new(e)))?;
    let metadata =
        gix::index::fs::Metadata::from_path_no_follow(&absolute).map_err(StagingError::Metadata)?;
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
}
