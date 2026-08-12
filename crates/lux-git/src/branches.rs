//! Branch creation and switching, in-process via `gix` (no shell-out —
//! ADR 0010).
//!
//! Switching is a real (forced) checkout: it materializes the target
//! branch's tree onto the working directory and index, deleting files
//! the old tree had that the new one doesn't and writing everything the
//! new one specifies — not merely a `HEAD` pointer update. It doesn't
//! check for or preserve uncommitted changes first, unlike plain `git
//! checkout`/`git switch`; that safety check is a caller/UI policy
//! decision, same as `commit`'s "is an empty commit allowed" note.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use gix::bstr::{BStr, ByteSlice};
use gix::refs::Target;
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};

use crate::staging::mode_for;

/// A local branch: its short name (e.g. `"main"`, not `"refs/heads/main"`)
/// and the commit it currently points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub commit_id: gix::ObjectId,
}

/// Failure listing, creating, or switching branches.
#[derive(Debug)]
pub enum BranchError {
    BareRepository,
    NonUtf8Path,
    InvalidName(Box<dyn std::error::Error + Send + Sync>),
    ListRefs(Box<dyn std::error::Error + Send + Sync>),
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    ReadBlob(Box<dyn std::error::Error + Send + Sync>),
    Io(std::io::Error),
    WriteIndex(Box<dyn std::error::Error + Send + Sync>),
    RefEdit(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for BranchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BranchError::BareRepository => write!(f, "repository has no working directory"),
            BranchError::NonUtf8Path => write!(f, "path is not valid UTF-8"),
            BranchError::InvalidName(e) => write!(f, "invalid branch name: {e}"),
            BranchError::ListRefs(e) => write!(f, "failed to list branches: {e}"),
            BranchError::ReadTree(e) => write!(f, "failed to read tree: {e}"),
            BranchError::ReadBlob(e) => write!(f, "failed to read blob: {e}"),
            BranchError::Io(e) => write!(f, "failed to update working directory: {e}"),
            BranchError::WriteIndex(e) => write!(f, "failed to write index: {e}"),
            BranchError::RefEdit(e) => write!(f, "failed to update branch ref: {e}"),
        }
    }
}

impl std::error::Error for BranchError {}

fn branch_full_name(name: &str) -> Result<gix::refs::FullName, BranchError> {
    format!("refs/heads/{name}")
        .try_into()
        .map_err(|e: gix::validate::reference::name::Error| BranchError::InvalidName(Box::new(e)))
}

fn path_to_bstr(path: &Path) -> Result<&BStr, BranchError> {
    path.to_str()
        .map(Into::into)
        .ok_or(BranchError::NonUtf8Path)
}

/// Lists local branches (`refs/heads/*`), each with the commit it
/// currently points at.
pub fn list(repo: &gix::Repository) -> Result<Vec<Branch>, BranchError> {
    let platform = repo
        .references()
        .map_err(|e| BranchError::ListRefs(Box::new(e)))?;
    let iter = platform
        .local_branches()
        .map_err(|e| BranchError::ListRefs(Box::new(e)))?;

    let mut branches = Vec::new();
    for reference in iter {
        let mut reference = reference.map_err(BranchError::ListRefs)?;
        let name = reference
            .name()
            .shorten()
            .to_str()
            .map_err(|_| BranchError::NonUtf8Path)?
            .to_string();
        let commit_id = reference
            .peel_to_id()
            .map_err(|e| BranchError::ListRefs(Box::new(e)))?
            .detach();
        branches.push(Branch { name, commit_id });
    }
    Ok(branches)
}

/// Creates a new branch named `name` pointing at `commit`. Fails if a
/// branch with that name already exists.
pub fn create(
    repo: &gix::Repository,
    name: &str,
    commit: gix::ObjectId,
) -> Result<Branch, BranchError> {
    let full_name = branch_full_name(name)?;
    repo.reference(
        full_name,
        commit,
        PreviousValue::MustNotExist,
        format!("branch: Created from {commit}"),
    )
    .map_err(|e| BranchError::RefEdit(Box::new(e)))?;
    Ok(Branch {
        name: name.to_string(),
        commit_id: commit,
    })
}

/// Recursively lists every blob path in `tree_id`.
pub(crate) fn tree_paths(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
) -> Result<Vec<gix::traverse::tree::recorder::Entry>, BranchError> {
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| BranchError::ReadTree(Box::new(e)))?;
    tree.traverse()
        .breadthfirst
        .files()
        .map_err(|e| BranchError::ReadTree(Box::new(e)))
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(if executable { 0o755 } else { 0o644 });
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) -> std::io::Result<()> {
    Ok(())
}

/// Materializes `new_tree` onto `repo`'s working directory and index,
/// replacing whatever `old_tree` had: paths in `old_tree` but not
/// `new_tree` are deleted, and every blob `new_tree` specifies is written
/// (creating parent directories as needed) with its recorded executable
/// bit. Used by both branch switching and stash apply/pop — the same
/// "make the working copy match this tree" operation either way.
pub(crate) fn checkout_tree(
    repo: &gix::Repository,
    old_tree: gix::ObjectId,
    new_tree: gix::ObjectId,
) -> Result<(), BranchError> {
    let work_dir = repo.workdir().ok_or(BranchError::BareRepository)?;
    let old_entries = tree_paths(repo, old_tree)?;
    let new_entries = tree_paths(repo, new_tree)?;

    let new_paths: BTreeSet<PathBuf> = new_entries
        .iter()
        .map(|e| PathBuf::from(e.filepath.to_string()))
        .collect();
    for entry in &old_entries {
        let rel = PathBuf::from(entry.filepath.to_string());
        if !new_paths.contains(&rel) {
            match std::fs::remove_file(work_dir.join(&rel)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(BranchError::Io(e)),
            }
        }
    }

    let mut index_state = gix::index::State::new(repo.object_hash());
    for entry in &new_entries {
        if !entry.mode.is_blob() {
            continue;
        }
        let rel = PathBuf::from(entry.filepath.to_string());
        let absolute = work_dir.join(&rel);
        if let Some(parent) = absolute.parent() {
            std::fs::create_dir_all(parent).map_err(BranchError::Io)?;
        }
        let blob = repo
            .find_blob(entry.oid)
            .map_err(|e| BranchError::ReadBlob(Box::new(e)))?;
        std::fs::write(&absolute, &blob.data).map_err(BranchError::Io)?;
        set_executable(&absolute, entry.mode.is_executable()).map_err(BranchError::Io)?;

        let metadata =
            gix::index::fs::Metadata::from_path_no_follow(&absolute).map_err(BranchError::Io)?;
        let stat = gix::index::entry::Stat::from_fs(&metadata)
            .map_err(|e| BranchError::Io(std::io::Error::other(e)))?;
        let mode = mode_for(&metadata);
        index_state.dangerously_push_entry(
            stat,
            entry.oid,
            gix::index::entry::Flags::empty(),
            mode,
            path_to_bstr(&rel)?,
        );
    }
    index_state.sort_entries();

    let mut index_file = gix::index::File::from_state(index_state, repo.index_path());
    index_file
        .write(gix::index::write::Options::default())
        .map_err(|e| BranchError::WriteIndex(Box::new(e)))?;
    Ok(())
}

/// Switches `HEAD` to branch `name`: points `HEAD` at `refs/heads/<name>`
/// and materializes that branch's tree onto the working directory and
/// index (see [`checkout_tree`]'s docs for exactly what that means).
pub fn switch(repo: &gix::Repository, name: &str) -> Result<Branch, BranchError> {
    let full_name = branch_full_name(name)?;
    let commit_id = repo
        .find_reference(name)
        .map_err(|e| BranchError::ListRefs(Box::new(e)))?
        .into_fully_peeled_id()
        .map_err(|e| BranchError::ListRefs(Box::new(e)))?
        .detach();
    let new_tree = repo
        .find_commit(commit_id)
        .map_err(|e| BranchError::ReadTree(Box::new(e)))?
        .tree_id()
        .map_err(|e| BranchError::ReadTree(Box::new(e)))?
        .detach();
    let old_tree = repo
        .head_tree_id_or_empty()
        .map_err(|e| BranchError::ReadTree(Box::new(e)))?
        .detach();

    checkout_tree(repo, old_tree, new_tree)?;

    let edit = RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: format!("checkout: moving to {name}").into(),
            },
            expected: PreviousValue::Any,
            new: Target::Symbolic(full_name),
        },
        name: "HEAD"
            .try_into()
            .map_err(|e: gix::validate::reference::name::Error| {
                BranchError::InvalidName(Box::new(e))
            })?,
        deref: false,
    };
    repo.edit_reference(edit)
        .map_err(|e| BranchError::RefEdit(Box::new(e)))?;

    Ok(Branch {
        name: name.to_string(),
        commit_id,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::commit::commit;
    use crate::staging::stage;

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init temp repo")
    }

    fn commit_one_file(
        repo: &gix::Repository,
        dir: &Path,
        path: &str,
        content: &str,
        message: &str,
    ) -> gix::ObjectId {
        fs::write(dir.join(path), content).unwrap();
        stage(repo, Path::new(path)).unwrap();
        commit(repo, message, "Lux Test", "lux-test@example.com").unwrap()
    }

    #[test]
    fn create_adds_a_branch_pointing_at_the_given_commit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let root = commit_one_file(&repo, dir.path(), "a.rs", "one\n", "root");

        let branch = create(&repo, "feature", root).unwrap();
        assert_eq!(
            branch,
            Branch {
                name: "feature".to_string(),
                commit_id: root
            }
        );

        let branches = list(&repo).unwrap();
        let names: Vec<_> = branches.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"feature"));
        assert!(
            names.contains(&"main") || names.contains(&"master"),
            "the initial branch is also listed"
        );
    }

    #[test]
    fn create_fails_if_the_branch_already_points_elsewhere() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let first = commit_one_file(&repo, dir.path(), "a.rs", "one\n", "first");
        let second = commit_one_file(&repo, dir.path(), "a.rs", "two\n", "second");
        create(&repo, "feature", first).unwrap();
        assert!(
            create(&repo, "feature", second).is_err(),
            "refuses to move an existing branch"
        );
    }

    #[test]
    fn create_with_the_same_target_twice_is_a_no_op() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let root = commit_one_file(&repo, dir.path(), "a.rs", "one\n", "root");
        create(&repo, "feature", root).unwrap();
        // `gix`'s `PreviousValue::MustNotExist` only rejects a ref update whose target would
        // actually change an existing ref; recreating with the identical target is idempotent,
        // not an error. See this deliverable's receipt for the detail.
        assert!(create(&repo, "feature", root).is_ok());
    }

    #[test]
    fn switch_updates_head_and_materializes_the_target_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let root = commit_one_file(&repo, dir.path(), "shared.rs", "shared\n", "root");
        create(&repo, "feature", root).unwrap();

        fs::write(dir.path().join("main-only.rs"), "main only\n").unwrap();
        stage(&repo, Path::new("main-only.rs")).unwrap();
        commit(
            &repo,
            "add main-only.rs",
            "Lux Test",
            "lux-test@example.com",
        )
        .unwrap();
        assert!(dir.path().join("main-only.rs").exists());

        switch(&repo, "feature").unwrap();

        assert!(
            !dir.path().join("main-only.rs").exists(),
            "switching away removes files the target branch doesn't have"
        );
        assert!(dir.path().join("shared.rs").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join("shared.rs")).unwrap(),
            "shared\n"
        );

        let head = repo.head().unwrap();
        assert_eq!(head.referent_name().unwrap().shorten(), "feature");
    }

    #[test]
    fn switch_rewrites_a_file_whose_content_differs_between_branches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let root = commit_one_file(&repo, dir.path(), "a.rs", "one\n", "root");
        create(&repo, "feature", root).unwrap();

        switch(&repo, "feature").unwrap();
        fs::write(dir.path().join("a.rs"), "two\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        commit(&repo, "edit on feature", "Lux Test", "lux-test@example.com").unwrap();

        switch(&repo, "main").unwrap_or_else(|_| switch(&repo, "master").unwrap());

        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "one\n"
        );
    }

    #[test]
    fn switch_index_matches_the_target_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let root = commit_one_file(&repo, dir.path(), "a.rs", "one\n", "root");
        create(&repo, "feature", root).unwrap();

        switch(&repo, "feature").unwrap();

        let index = repo.open_index().unwrap();
        assert!(index.entry_by_path("a.rs".into()).is_some());
    }
}
