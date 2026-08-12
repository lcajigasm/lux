//! Stash: set aside the working directory's current changes to tracked
//! files and restore `HEAD`'s committed state, with the ability to bring
//! them back later. In-process via `gix` (no shell-out — ADR 0010).
//!
//! **Compatibility note.** This is `lux`'s own stash, not a byte-for-byte
//! reimplementation of `git stash`'s on-disk format. It uses the real
//! `refs/stash` ref name, but the stack itself is threaded through the
//! stash commits' own second parent (`save` points a new stash commit at
//! `[HEAD, previous top-of-stack]`; `pop`/`list` follow that second
//! parent to walk the stack) rather than `git`'s reflog-based history on
//! `refs/stash`. Real `git stash` also writes a two- or three-parent
//! commit that separately records the index state from the working tree;
//! this only snapshots one tree (the working tree's current content for
//! every path tracked at `HEAD`). A repository stashed here isn't
//! guaranteed to also work with the real `git stash` CLI against the same
//! repository — but `save`/`list`/`pop` are genuinely functional, tested
//! operations against `lux`'s own stack, not a stub.

use std::fmt;
use std::path::PathBuf;

use gix::bstr::ByteSlice;
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};

use crate::branches::{BranchError, checkout_tree, tree_paths};
use crate::commit::signature;

const STASH_REF: &str = "refs/stash";

/// One entry in the stash stack, newest first from [`list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub commit_id: gix::ObjectId,
    pub message: String,
}

/// Failure saving, listing, or popping a stash.
#[derive(Debug)]
pub enum StashError {
    BareRepository,
    NoCommitYet,
    NothingToStash,
    NoStash,
    Branches(BranchError),
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    ReadCommit(Box<dyn std::error::Error + Send + Sync>),
    Io(std::io::Error),
    Write(Box<dyn std::error::Error + Send + Sync>),
    RefEdit(Box<dyn std::error::Error + Send + Sync>),
    MessageNotUtf8,
}

impl fmt::Display for StashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StashError::BareRepository => write!(f, "repository has no working directory"),
            StashError::NoCommitYet => write!(f, "you do not have the initial commit yet"),
            StashError::NothingToStash => write!(f, "no local changes to save"),
            StashError::NoStash => write!(f, "no stash entries found"),
            StashError::Branches(e) => write!(f, "{e}"),
            StashError::ReadTree(e) => write!(f, "failed to read tree: {e}"),
            StashError::ReadCommit(e) => write!(f, "failed to read stash commit: {e}"),
            StashError::Io(e) => write!(f, "failed to read working directory: {e}"),
            StashError::Write(e) => write!(f, "failed to write stash commit: {e}"),
            StashError::RefEdit(e) => write!(f, "failed to update {STASH_REF}: {e}"),
            StashError::MessageNotUtf8 => write!(f, "stash message is not valid UTF-8"),
        }
    }
}

impl std::error::Error for StashError {}

fn update_stash_ref(
    repo: &gix::Repository,
    target: gix::ObjectId,
    message: impl Into<gix::bstr::BString>,
) -> Result<(), StashError> {
    repo.edit_reference(RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: true,
                message: message.into(),
            },
            expected: PreviousValue::Any,
            new: gix::refs::Target::Object(target),
        },
        name: STASH_REF
            .try_into()
            .map_err(|e: gix::validate::reference::name::Error| StashError::RefEdit(Box::new(e)))?,
        deref: false,
    })
    .map_err(|e| StashError::RefEdit(Box::new(e)))?;
    Ok(())
}

/// Builds a tree reflecting the *current on-disk content* of every path
/// tracked at `head_tree` — deleted files are dropped, modified files get
/// their current content, untouched files are carried over unchanged.
/// Matches plain `git stash`'s default scope: tracked files only, no
/// untracked files (no `-u`).
fn snapshot_working_tree(
    repo: &gix::Repository,
    head_tree: gix::ObjectId,
) -> Result<gix::ObjectId, StashError> {
    let work_dir = repo.workdir().ok_or(StashError::BareRepository)?;
    let tree = repo
        .find_tree(head_tree)
        .map_err(|e| StashError::ReadTree(Box::new(e)))?;
    let mut editor = tree.edit().map_err(|e| StashError::ReadTree(Box::new(e)))?;

    for entry in tree_paths(repo, head_tree).map_err(StashError::Branches)? {
        if !entry.mode.is_blob() {
            continue;
        }
        let rel = PathBuf::from(entry.filepath.to_string());
        let absolute = work_dir.join(&rel);
        match std::fs::read(&absolute) {
            Ok(content) => {
                let blob_id = repo
                    .write_blob(&content)
                    .map_err(|e| StashError::Write(Box::new(e)))?;
                editor
                    .upsert(entry.filepath.clone(), entry.mode.kind(), blob_id.detach())
                    .map_err(|e| StashError::Write(Box::new(e)))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                editor
                    .remove(entry.filepath.clone())
                    .map_err(|e| StashError::Write(Box::new(e)))?;
            }
            Err(e) => return Err(StashError::Io(e)),
        }
    }

    editor
        .write()
        .map(|id| id.detach())
        .map_err(|e| StashError::Write(Box::new(e)))
}

/// A stash commit's own message and its "beneath it in the stack" parent,
/// if any.
fn stash_commit_info(
    repo: &gix::Repository,
    id: gix::ObjectId,
) -> Result<(String, Option<gix::ObjectId>), StashError> {
    let commit = repo
        .find_commit(id)
        .map_err(|e| StashError::ReadCommit(Box::new(e)))?;
    let decoded = commit
        .decode()
        .map_err(|e| StashError::ReadCommit(Box::new(e)))?;
    let message = decoded
        .message
        .to_str()
        .map_err(|_| StashError::MessageNotUtf8)?
        .to_string();
    let beneath = decoded.parents().nth(1);
    Ok((message, beneath))
}

/// Snapshots the working directory's current state of tracked files into
/// a new stash entry — pointing its second parent at whatever was
/// previously on top of the stack, so the stack threads through the
/// commits' own parents — then restores the working directory and index
/// to `HEAD`: the changes are set aside, not merely copied. Fails if
/// there's no commit yet (`NoCommitYet`) or nothing has actually changed
/// since `HEAD` (`NothingToStash`).
pub fn save(
    repo: &gix::Repository,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<gix::ObjectId, StashError> {
    let head_commit_id = repo
        .head_id()
        .map_err(|_| StashError::NoCommitYet)?
        .detach();
    let head_tree = repo
        .find_commit(head_commit_id)
        .map_err(|e| StashError::ReadTree(Box::new(e)))?
        .tree_id()
        .map_err(|e| StashError::ReadTree(Box::new(e)))?
        .detach();

    let snapshot_tree = snapshot_working_tree(repo, head_tree)?;
    if snapshot_tree == head_tree {
        return Err(StashError::NothingToStash);
    }

    let previous_top = repo
        .find_reference(STASH_REF)
        .ok()
        .and_then(|mut r| r.peel_to_id().ok())
        .map(|id| id.detach());
    let mut parents = vec![head_commit_id];
    parents.extend(previous_top);

    let sig = signature(author_name, author_email);
    let commit_obj = gix::objs::Commit {
        tree: snapshot_tree,
        parents: parents.into(),
        author: sig.clone(),
        committer: sig,
        encoding: None,
        message: message.into(),
        extra_headers: Default::default(),
    };
    let stash_id = repo
        .write_object(&commit_obj)
        .map_err(|e| StashError::Write(Box::new(e)))?
        .detach();

    update_stash_ref(repo, stash_id, format!("stash: {message}"))?;
    checkout_tree(repo, snapshot_tree, head_tree).map_err(StashError::Branches)?;
    Ok(stash_id)
}

/// Lists the stash stack, newest first, by following each stash commit's
/// second parent down to the bottom.
pub fn list(repo: &gix::Repository) -> Result<Vec<StashEntry>, StashError> {
    let Ok(mut reference) = repo.find_reference(STASH_REF) else {
        return Ok(Vec::new());
    };
    let mut current = Some(
        reference
            .peel_to_id()
            .map_err(|e| StashError::ReadCommit(Box::new(e)))?
            .detach(),
    );

    let mut entries = Vec::new();
    while let Some(id) = current {
        let (message, beneath) = stash_commit_info(repo, id)?;
        entries.push(StashEntry {
            commit_id: id,
            message,
        });
        current = beneath;
    }
    Ok(entries)
}

/// Applies the most recent stash entry onto the working directory and
/// index, then removes it from the stack (moving `refs/stash` to the
/// entry beneath it, or deleting the ref if the stack is now empty).
/// Fails with [`StashError::NoStash`] if there's nothing to pop.
pub fn pop(repo: &gix::Repository) -> Result<gix::ObjectId, StashError> {
    let mut reference = repo
        .find_reference(STASH_REF)
        .map_err(|_| StashError::NoStash)?;
    let top_id = reference
        .peel_to_id()
        .map_err(|e| StashError::ReadCommit(Box::new(e)))?
        .detach();

    let (_, beneath) = stash_commit_info(repo, top_id)?;
    let stash_tree = repo
        .find_commit(top_id)
        .map_err(|e| StashError::ReadTree(Box::new(e)))?
        .tree_id()
        .map_err(|e| StashError::ReadTree(Box::new(e)))?
        .detach();
    let current_head_tree = repo
        .head_tree_id_or_empty()
        .map_err(|e| StashError::ReadTree(Box::new(e)))?
        .detach();
    checkout_tree(repo, current_head_tree, stash_tree).map_err(StashError::Branches)?;

    match beneath {
        Some(next_top) => update_stash_ref(repo, next_top, "stash: pop")?,
        None => {
            repo.edit_reference(RefEdit {
                change: Change::Delete {
                    expected: PreviousValue::Any,
                    log: RefLog::AndReference,
                },
                name: STASH_REF.try_into().map_err(
                    |e: gix::validate::reference::name::Error| StashError::RefEdit(Box::new(e)),
                )?,
                deref: false,
            })
            .map_err(|e| StashError::RefEdit(Box::new(e)))?;
        }
    }

    Ok(top_id)
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
    fn save_restores_head_and_pop_brings_the_change_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        commit_one_file(&repo, dir.path(), "a.rs", "one\n", "root");

        fs::write(dir.path().join("a.rs"), "two\n").unwrap();
        save(&repo, "WIP", "Lux Test", "lux-test@example.com").unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "one\n",
            "working tree is reset to HEAD after save"
        );

        pop(&repo).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "two\n",
            "pop brings the change back"
        );
    }

    #[test]
    fn save_with_no_changes_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        commit_one_file(&repo, dir.path(), "a.rs", "one\n", "root");
        assert!(matches!(
            save(&repo, "WIP", "Lux Test", "lux-test@example.com"),
            Err(StashError::NothingToStash)
        ));
    }

    #[test]
    fn pop_with_no_stash_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        commit_one_file(&repo, dir.path(), "a.rs", "one\n", "root");
        assert!(matches!(pop(&repo), Err(StashError::NoStash)));
    }

    #[test]
    fn list_shows_multiple_stashes_newest_first_and_pop_consumes_from_the_top() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        commit_one_file(&repo, dir.path(), "a.rs", "base\n", "root");

        fs::write(dir.path().join("a.rs"), "first change\n").unwrap();
        save(&repo, "first stash", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "second change\n").unwrap();
        save(&repo, "second stash", "Lux Test", "lux-test@example.com").unwrap();

        let entries = list(&repo).unwrap();
        let messages: Vec<_> = entries.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(messages, vec!["second stash", "first stash"]);

        pop(&repo).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "second change\n"
        );
        assert_eq!(list(&repo).unwrap().len(), 1, "one stash remains");

        pop(&repo).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "first change\n"
        );
        assert!(
            list(&repo).unwrap().is_empty(),
            "stack is empty after popping both"
        );
    }

    #[test]
    fn stashing_a_deleted_tracked_file_and_popping_restores_the_deletion() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        commit_one_file(&repo, dir.path(), "a.rs", "content\n", "root");

        fs::remove_file(dir.path().join("a.rs")).unwrap();
        save(&repo, "deleted a.rs", "Lux Test", "lux-test@example.com").unwrap();
        assert!(
            dir.path().join("a.rs").exists(),
            "HEAD's version comes back after save resets the tree"
        );

        pop(&repo).unwrap();
        assert!(
            !dir.path().join("a.rs").exists(),
            "pop reapplies the deletion"
        );
    }
}
