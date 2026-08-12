//! Repository status summary: current branch, dirty/clean state, and a
//! count of changed files — the data a status bar shows. In-process via
//! `gix`, reusing `diff_panel::changed_files` for the change count (one
//! change-detection path, not two — no shell-out, ADR 0010).

use std::fmt;

use gix::bstr::ByteSlice;

use crate::diff_panel::{DiffPanelError, changed_files};

/// Where `HEAD` currently points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchState {
    /// On a real branch, by its short name (e.g. `"main"`, not
    /// `"refs/heads/main"`).
    Named(String),
    /// `HEAD` points directly at a commit, not through a branch.
    Detached(gix::ObjectId),
    /// A branch name exists but has no commits yet (a fresh repository).
    Unborn(String),
}

/// The at-a-glance repository state a status bar renders: branch, dirty
/// or clean, and how many files changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryStatus {
    pub branch: BranchState,
    pub changed_files: usize,
}

impl RepositoryStatus {
    pub fn is_clean(&self) -> bool {
        self.changed_files == 0
    }
}

/// Failure reading the repository's branch or change state.
#[derive(Debug)]
pub enum StatusError {
    ReadHead(Box<dyn std::error::Error + Send + Sync>),
    NonUtf8Name,
    Changes(DiffPanelError),
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusError::ReadHead(e) => write!(f, "failed to read HEAD: {e}"),
            StatusError::NonUtf8Name => write!(f, "branch name is not valid UTF-8"),
            StatusError::Changes(e) => write!(f, "failed to compute changed files: {e}"),
        }
    }
}

impl std::error::Error for StatusError {}

fn branch_state(repo: &gix::Repository) -> Result<BranchState, StatusError> {
    let head = repo
        .head()
        .map_err(|e| StatusError::ReadHead(Box::new(e)))?;
    match head.kind {
        gix::head::Kind::Symbolic(reference) => {
            let name = reference
                .name
                .shorten()
                .to_str()
                .map_err(|_| StatusError::NonUtf8Name)?
                .to_string();
            Ok(BranchState::Named(name))
        }
        gix::head::Kind::Unborn(full_name) => {
            let name = full_name
                .shorten()
                .to_str()
                .map_err(|_| StatusError::NonUtf8Name)?
                .to_string();
            Ok(BranchState::Unborn(name))
        }
        gix::head::Kind::Detached { target, .. } => Ok(BranchState::Detached(target)),
    }
}

/// Summarizes `repo`'s current state: the branch (or detached/unborn
/// state) `HEAD` is on, and how many files differ from `HEAD` (working
/// tree vs. committed content, respecting `.gitignore` — the same
/// comparison [`changed_files`] performs for the changes panel).
pub fn status(repo: &gix::Repository) -> Result<RepositoryStatus, StatusError> {
    let branch = branch_state(repo)?;
    let changed = changed_files(repo).map_err(StatusError::Changes)?;
    Ok(RepositoryStatus {
        branch,
        changed_files: changed.len(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::branches::{create, switch};
    use crate::commit::commit;
    use crate::staging::stage;

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init temp repo")
    }

    #[test]
    fn unborn_repo_reports_no_changes_and_an_unborn_branch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let status = status(&repo).unwrap();
        assert!(matches!(status.branch, BranchState::Unborn(_)));
        assert_eq!(status.changed_files, 0);
        assert!(status.is_clean());
    }

    #[test]
    fn clean_repo_after_commit_reports_the_branch_name_and_no_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();

        let status = status(&repo).unwrap();
        match &status.branch {
            BranchState::Named(name) => assert!(name == "main" || name == "master"),
            other => panic!("expected a named branch, got {other:?}"),
        }
        assert!(status.is_clean());
    }

    #[test]
    fn dirty_working_tree_is_counted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "two\n").unwrap();
        fs::write(dir.path().join("b.rs"), "new\n").unwrap();

        let status = status(&repo).unwrap();
        assert_eq!(status.changed_files, 2);
        assert!(!status.is_clean());
    }

    #[test]
    fn switching_branches_updates_the_reported_branch_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let root = commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();
        create(&repo, "feature", root, "Lux Test", "lux-test@example.com").unwrap();

        switch(&repo, "feature", "Lux Test", "lux-test@example.com").unwrap();
        let status = status(&repo).unwrap();
        assert_eq!(status.branch, BranchState::Named("feature".to_string()));
    }
}
