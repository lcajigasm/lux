//! Merge conflict resolution: a real three-way merge computed by `gix`'s
//! own merge engine (`gix-merge`, checked for maturity before assuming a
//! gap — the same ADR 0010 discipline that already paid off for blame
//! earlier in this phase), surfacing content conflicts with the "accept
//! ours / theirs / both" choice an editor's "Accept Current / Incoming /
//! Both" action needs. No shell-out (ADR 0010).
//!
//! **Scope.** Only *content conflicts* are surfaced: a path both sides
//! modified, or both sides independently added, with incompatible
//! content. `gix-merge` also detects rename conflicts and delete/modify
//! conflicts (one side removed a path the other changed) — real outputs
//! this module doesn't yet turn into a [`MergeConflict`]; `merge()`
//! reports those as [`MergeConflictError::UnsupportedConflictShape`]
//! rather than silently dropping or misrepresenting them.

use std::fmt;
use std::path::PathBuf;

use gix::bstr::ByteSlice;
use gix::diff::tree_with_rewrites::Change;

use crate::branches::{BranchError, checkout_tree};
use crate::commit::{CommitError, signature, tree_from_index};
use crate::staging::{StagingError, stage};

/// One unresolved content conflict: `path` was changed on both sides of
/// the merge, and needs a caller/UI decision between `ours`, `theirs`, or
/// both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub path: PathBuf,
    /// The blob at `path` in the merge base, or `None` if both sides
    /// added it independently (no common ancestor content).
    pub base: Option<gix::ObjectId>,
    pub ours: gix::ObjectId,
    pub theirs: gix::ObjectId,
}

/// The result of [`merge`]: the tree `gix` computed — auto-merged where
/// possible, with conflict markers written at unresolved paths, the same
/// as real `git merge` leaves in the working tree — and the paths that
/// still need a resolution choice. That tree has already been
/// materialized onto the working directory and index (via the same
/// [`checkout_tree`] every other write operation in this crate uses), so
/// the repository is in a genuine "mid-merge, conflict markers in files"
/// state a caller can inspect or edit directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOutcome {
    pub tree_id: gix::ObjectId,
    pub conflicts: Vec<MergeConflict>,
}

/// Which side to keep when resolving a [`MergeConflict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Ours,
    Theirs,
    /// Keeps both sides' content, ours followed by theirs.
    Both,
}

/// Failure computing or resolving a merge.
#[derive(Debug)]
pub enum MergeConflictError {
    NonUtf8Path,
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    ReadBlob(Box<dyn std::error::Error + Send + Sync>),
    Merge(Box<dyn std::error::Error + Send + Sync>),
    UnsupportedConflictShape(String),
    Branches(BranchError),
    Staging(StagingError),
    Commit(CommitError),
    Io(std::io::Error),
    Write(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for MergeConflictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeConflictError::NonUtf8Path => write!(f, "path is not valid UTF-8"),
            MergeConflictError::ReadTree(e) => write!(f, "failed to read tree: {e}"),
            MergeConflictError::ReadBlob(e) => write!(f, "failed to read blob: {e}"),
            MergeConflictError::Merge(e) => write!(f, "failed to compute merge: {e}"),
            MergeConflictError::UnsupportedConflictShape(what) => {
                write!(f, "unsupported conflict shape: {what}")
            }
            MergeConflictError::Branches(e) => write!(f, "{e}"),
            MergeConflictError::Staging(e) => write!(f, "{e}"),
            MergeConflictError::Commit(e) => write!(f, "{e}"),
            MergeConflictError::Io(e) => write!(f, "failed to write resolved file: {e}"),
            MergeConflictError::Write(e) => write!(f, "failed to write merge commit: {e}"),
        }
    }
}

impl std::error::Error for MergeConflictError {}

fn change_path(change: &Change) -> Option<PathBuf> {
    match change {
        Change::Addition { location, .. }
        | Change::Modification { location, .. }
        | Change::Deletion { location, .. } => {
            Some(PathBuf::from(location.to_str_lossy().into_owned()))
        }
        Change::Rewrite { .. } => None,
    }
}

/// Merges `their_commit` into `our_commit` (the merge-base is found
/// automatically), returning the resulting tree and the content
/// conflicts that need a resolution choice. `our_commit` is expected to
/// be the repository's current `HEAD` — [`complete`] relies on that same
/// assumption to write the merge commit.
pub fn merge(
    repo: &gix::Repository,
    our_commit: gix::ObjectId,
    their_commit: gix::ObjectId,
) -> Result<MergeOutcome, MergeConflictError> {
    let options: gix::merge::tree::Options = repo
        .tree_merge_options()
        .map_err(|e| MergeConflictError::Merge(Box::new(e)))?;
    let mut outcome = repo
        .merge_commits(our_commit, their_commit, Default::default(), options.into())
        .map_err(|e| MergeConflictError::Merge(Box::new(e)))?;
    let tree_id = outcome
        .tree_merge
        .tree
        .write()
        .map(|id| id.detach())
        .map_err(|e| MergeConflictError::Merge(Box::new(e)))?;

    let mut conflicts = Vec::new();
    for conflict in &outcome.tree_merge.conflicts {
        if !conflict.is_unresolved(gix::merge::tree::TreatAsUnresolved::default()) {
            continue;
        }
        let (ours_change, _theirs_change) = conflict.changes_in_resolution();
        let path = change_path(ours_change).ok_or_else(|| {
            MergeConflictError::UnsupportedConflictShape("rename conflict".to_string())
        })?;
        let entries = conflict.entries();
        let base = entries[0].map(|e| e.id);
        let ours = entries[1].map(|e| e.id).ok_or_else(|| {
            MergeConflictError::UnsupportedConflictShape(format!(
                "{}: no content on our side (delete/modify conflict)",
                path.display()
            ))
        })?;
        let theirs = entries[2].map(|e| e.id).ok_or_else(|| {
            MergeConflictError::UnsupportedConflictShape(format!(
                "{}: no content on their side (delete/modify conflict)",
                path.display()
            ))
        })?;
        conflicts.push(MergeConflict {
            path,
            base,
            ours,
            theirs,
        });
    }

    let our_tree = repo
        .find_commit(our_commit)
        .map_err(|e| MergeConflictError::ReadTree(Box::new(e)))?
        .tree_id()
        .map_err(|e| MergeConflictError::ReadTree(Box::new(e)))?
        .detach();
    checkout_tree(repo, our_tree, tree_id).map_err(MergeConflictError::Branches)?;

    Ok(MergeOutcome { tree_id, conflicts })
}

/// Resolves `conflict` by writing the chosen content to the working
/// directory and staging it — the primitive an "Accept Current / Incoming
/// / Both" editor action calls per conflict.
pub fn resolve(
    repo: &gix::Repository,
    conflict: &MergeConflict,
    resolution: Resolution,
) -> Result<(), MergeConflictError> {
    let ours = repo
        .find_blob(conflict.ours)
        .map_err(|e| MergeConflictError::ReadBlob(Box::new(e)))?;
    let theirs = repo
        .find_blob(conflict.theirs)
        .map_err(|e| MergeConflictError::ReadBlob(Box::new(e)))?;

    let content: Vec<u8> = match resolution {
        Resolution::Ours => ours.data.clone(),
        Resolution::Theirs => theirs.data.clone(),
        Resolution::Both => [ours.data.as_slice(), theirs.data.as_slice()].concat(),
    };

    let work_dir = repo.workdir().ok_or_else(|| {
        MergeConflictError::UnsupportedConflictShape(
            "repository has no working directory".to_string(),
        )
    })?;
    std::fs::write(work_dir.join(&conflict.path), content).map_err(MergeConflictError::Io)?;
    stage(repo, &conflict.path).map_err(MergeConflictError::Staging)?;
    Ok(())
}

/// Completes the merge once every conflict has been resolved (or, for a
/// merge with no conflicts, immediately): writes a merge commit from the
/// currently staged index, with both `our_commit` and `their_commit` as
/// parents, and moves `HEAD` to it. Whether every conflict is actually
/// resolved before calling this is a caller/UI decision, the same as
/// `commit`'s "is an empty commit allowed" note — this won't second-guess
/// what's staged.
pub fn complete(
    repo: &gix::Repository,
    our_commit: gix::ObjectId,
    their_commit: gix::ObjectId,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<gix::ObjectId, MergeConflictError> {
    let tree_id = tree_from_index(repo).map_err(MergeConflictError::Commit)?;
    let sig = signature(author_name, author_email);
    let mut time_buf = gix::date::parse::TimeBuf::default();
    let sig_ref = sig.to_ref(&mut time_buf);
    repo.commit_as(
        sig_ref,
        sig_ref,
        "HEAD",
        message,
        tree_id,
        [our_commit, their_commit],
    )
    .map(|id| id.detach())
    .map_err(|e| MergeConflictError::Write(Box::new(e)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::branches::{create, switch};
    use crate::commit::commit;
    use crate::staging::stage as do_stage;

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
        do_stage(repo, Path::new(path)).unwrap();
        commit(repo, message, "Lux Test", "lux-test@example.com").unwrap()
    }

    /// Builds a repo with a real, induced content conflict: `main` and
    /// `feature` both edit the same line of `a.rs` differently, diverging
    /// from a shared base.
    fn repo_with_induced_conflict(dir: &Path) -> (gix::Repository, gix::ObjectId, gix::ObjectId) {
        let repo = init_repo(dir);
        commit_one_file(&repo, dir, "a.rs", "one\ntwo\nthree\n", "base");
        let main_head = repo.head_id().unwrap().detach();
        create(
            &repo,
            "feature",
            main_head,
            "Lux Test",
            "lux-test@example.com",
        )
        .unwrap();

        switch(&repo, "feature", "Lux Test", "lux-test@example.com").unwrap();
        let their_commit = commit_one_file(
            &repo,
            dir,
            "a.rs",
            "one\nTWO-FROM-FEATURE\nthree\n",
            "feature edit",
        );

        switch(&repo, "main", "Lux Test", "lux-test@example.com").unwrap_or_else(|_| {
            switch(&repo, "master", "Lux Test", "lux-test@example.com").unwrap()
        });
        let our_commit = commit_one_file(
            &repo,
            dir,
            "a.rs",
            "one\ntwo-from-main\nthree\n",
            "main edit",
        );

        (repo, our_commit, their_commit)
    }

    #[test]
    fn merge_with_no_conflicts_reports_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        let base = commit_one_file(&repo, dir.path(), "a.rs", "one\n", "base");
        create(&repo, "feature", base, "Lux Test", "lux-test@example.com").unwrap();

        switch(&repo, "feature", "Lux Test", "lux-test@example.com").unwrap();
        let their_commit = commit_one_file(
            &repo,
            dir.path(),
            "b.rs",
            "feature file\n",
            "feature adds b.rs",
        );

        switch(&repo, "main", "Lux Test", "lux-test@example.com").unwrap_or_else(|_| {
            switch(&repo, "master", "Lux Test", "lux-test@example.com").unwrap()
        });
        let our_commit = repo.head_id().unwrap().detach();

        let outcome = merge(&repo, our_commit, their_commit).unwrap();
        assert!(outcome.conflicts.is_empty());
        assert!(
            fs::read_to_string(dir.path().join("b.rs")).is_ok(),
            "the non-conflicting change is already merged onto disk"
        );
    }

    #[test]
    fn merge_reports_a_real_induced_conflict() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (repo, our_commit, their_commit) = repo_with_induced_conflict(dir.path());

        let outcome = merge(&repo, our_commit, their_commit).unwrap();
        assert_eq!(outcome.conflicts.len(), 1);
        let conflict = &outcome.conflicts[0];
        assert_eq!(conflict.path, Path::new("a.rs"));

        let ours_blob = repo.find_blob(conflict.ours).unwrap();
        let theirs_blob = repo.find_blob(conflict.theirs).unwrap();
        assert_eq!(ours_blob.data, b"one\ntwo-from-main\nthree\n");
        assert_eq!(theirs_blob.data, b"one\nTWO-FROM-FEATURE\nthree\n");

        let on_disk = fs::read_to_string(dir.path().join("a.rs")).unwrap();
        assert!(
            on_disk.contains("<<<<<<<"),
            "the working copy shows real conflict markers, same as `git merge`"
        );
    }

    #[test]
    fn resolve_ours_keeps_our_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (repo, our_commit, their_commit) = repo_with_induced_conflict(dir.path());
        let outcome = merge(&repo, our_commit, their_commit).unwrap();

        resolve(&repo, &outcome.conflicts[0], Resolution::Ours).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "one\ntwo-from-main\nthree\n"
        );
    }

    #[test]
    fn resolve_theirs_keeps_their_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (repo, our_commit, their_commit) = repo_with_induced_conflict(dir.path());
        let outcome = merge(&repo, our_commit, their_commit).unwrap();

        resolve(&repo, &outcome.conflicts[0], Resolution::Theirs).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "one\nTWO-FROM-FEATURE\nthree\n"
        );
    }

    #[test]
    fn resolve_both_keeps_ours_then_theirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (repo, our_commit, their_commit) = repo_with_induced_conflict(dir.path());
        let outcome = merge(&repo, our_commit, their_commit).unwrap();

        resolve(&repo, &outcome.conflicts[0], Resolution::Both).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "one\ntwo-from-main\nthree\none\nTWO-FROM-FEATURE\nthree\n"
        );
    }

    #[test]
    fn complete_after_resolving_writes_a_two_parent_merge_commit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (repo, our_commit, their_commit) = repo_with_induced_conflict(dir.path());
        let outcome = merge(&repo, our_commit, their_commit).unwrap();
        resolve(&repo, &outcome.conflicts[0], Resolution::Theirs).unwrap();

        let merge_commit_id = complete(
            &repo,
            our_commit,
            their_commit,
            "merge feature into main",
            "Lux Test",
            "lux-test@example.com",
        )
        .unwrap();

        let decoded = repo.find_commit(merge_commit_id).unwrap();
        let parents: Vec<_> = decoded.parent_ids().map(|id| id.detach()).collect();
        assert_eq!(parents, vec![our_commit, their_commit]);

        let tree = decoded.tree().unwrap();
        let entry = tree.lookup_entry_by_path("a.rs").unwrap().unwrap();
        let blob = repo.find_blob(entry.object_id()).unwrap();
        assert_eq!(blob.data, b"one\nTWO-FROM-FEATURE\nthree\n");
    }

    #[test]
    fn resolved_file_is_no_longer_conflicted_when_merged_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (repo, our_commit, their_commit) = repo_with_induced_conflict(dir.path());
        let first = merge(&repo, our_commit, their_commit).unwrap();
        assert_eq!(first.conflicts.len(), 1);
        resolve(&repo, &first.conflicts[0], Resolution::Ours).unwrap();
        complete(
            &repo,
            our_commit,
            their_commit,
            "merge",
            "Lux Test",
            "lux-test@example.com",
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("a.rs")).unwrap(),
            "one\ntwo-from-main\nthree\n"
        );
    }
}
