//! Commit creation and amend: write a real commit object from the
//! repository's currently staged index, in-process via `gix` (no
//! shell-out — ADR 0010).

use std::fmt;

use gix::date::parse::TimeBuf;

use crate::staging::{StagingError, open_or_create_index};

/// Failure building or writing a commit.
#[derive(Debug)]
pub enum CommitError {
    Staging(StagingError),
    HeadTree(gix::reference::head_tree_id::Error),
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    BuildTree(Box<dyn std::error::Error + Send + Sync>),
    ResolveHead(Box<dyn std::error::Error + Send + Sync>),
    Write(Box<dyn std::error::Error + Send + Sync>),
    NoCommitToAmend(Box<dyn std::error::Error + Send + Sync>),
    DecodeCommit(Box<dyn std::error::Error + Send + Sync>),
    MessageNotUtf8,
}

impl fmt::Display for CommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitError::Staging(e) => write!(f, "failed to read the index: {e}"),
            CommitError::HeadTree(e) => write!(f, "failed to resolve HEAD tree: {e}"),
            CommitError::ReadTree(e) => write!(f, "failed to read HEAD tree: {e}"),
            CommitError::BuildTree(e) => write!(f, "failed to build tree from index: {e}"),
            CommitError::ResolveHead(e) => write!(f, "failed to resolve HEAD: {e}"),
            CommitError::Write(e) => write!(f, "failed to write commit: {e}"),
            CommitError::NoCommitToAmend(e) => write!(f, "no commit to amend: {e}"),
            CommitError::DecodeCommit(e) => write!(f, "failed to decode commit: {e}"),
            CommitError::MessageNotUtf8 => write!(f, "commit message is not valid UTF-8"),
        }
    }
}

impl std::error::Error for CommitError {}

/// Builds a tree object from `repo`'s currently staged index, starting
/// from `HEAD`'s tree (or an empty one, on an unborn `HEAD`) and applying
/// every index entry as an upsert — so untouched paths keep whatever
/// they were at `HEAD`, and only what's actually staged changes.
pub(crate) fn tree_from_index(repo: &gix::Repository) -> Result<gix::ObjectId, CommitError> {
    let index = open_or_create_index(repo).map_err(CommitError::Staging)?;
    let base_tree_id = repo
        .head_tree_id_or_empty()
        .map_err(CommitError::HeadTree)?;
    let base_tree = repo
        .find_tree(base_tree_id)
        .map_err(|e| CommitError::ReadTree(Box::new(e)))?;
    let mut editor = base_tree
        .edit()
        .map_err(|e| CommitError::BuildTree(Box::new(e)))?;

    for entry in index.entries() {
        let path = entry.path(&index);
        let kind = entry
            .mode
            .to_tree_entry_mode()
            .map(|mode| mode.kind())
            .expect("staging only ever writes FILE/FILE_EXECUTABLE entries");
        editor
            .upsert(path, kind, entry.id)
            .map_err(|e| CommitError::BuildTree(Box::new(e)))?;
    }

    editor
        .write()
        .map(|id| id.detach())
        .map_err(|e| CommitError::BuildTree(Box::new(e)))
}

pub(crate) fn signature(name: &str, email: &str) -> gix::actor::Signature {
    gix::actor::Signature {
        name: name.into(),
        email: email.into(),
        time: gix::date::Time::now_local_or_utc(),
    }
}

/// Commits `repo`'s currently staged content (its index) as a new commit
/// on `HEAD`, with `message` (which may be multi-line) and
/// `author_name`/`author_email` as both author and committer. An empty
/// index commits an empty tree, same as `git commit --allow-empty` — this
/// function doesn't reject that; whether an empty commit is allowed is a
/// caller/UI policy decision, not this write primitive's to make.
pub fn commit(
    repo: &gix::Repository,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<gix::ObjectId, CommitError> {
    let tree_id = tree_from_index(repo)?;
    let mut head = repo
        .head()
        .map_err(|e| CommitError::ResolveHead(Box::new(e)))?;
    let parent = head
        .try_peel_to_id()
        .map_err(|e| CommitError::ResolveHead(Box::new(e)))?;

    let sig = signature(author_name, author_email);
    let mut time_buf = TimeBuf::default();
    let sig_ref = sig.to_ref(&mut time_buf);
    repo.commit_as(
        sig_ref,
        sig_ref,
        "HEAD",
        message,
        tree_id,
        parent.map(|id| id.detach()),
    )
    .map(|id| id.detach())
    .map_err(|e| CommitError::Write(Box::new(e)))
}

/// Amends the current `HEAD` commit: replaces its tree with `repo`'s
/// currently staged index content, keeping the same parents and original
/// author as the commit being amended (matching `git commit --amend`'s
/// default of preserving authorship; only the committer identity and time
/// change, same as real `git`). `message` replaces the original message
/// when given, or reuses it unchanged. Fails if there's no commit yet to
/// amend (an unborn `HEAD`).
pub fn amend(
    repo: &gix::Repository,
    message: Option<&str>,
    committer_name: &str,
    committer_email: &str,
) -> Result<gix::ObjectId, CommitError> {
    let original_commit = repo
        .head_commit()
        .map_err(|e| CommitError::NoCommitToAmend(Box::new(e)))?;
    let original_id = original_commit.id;
    let original = original_commit
        .decode()
        .map_err(|e| CommitError::DecodeCommit(Box::new(e)))?;
    let author: gix::actor::Signature = original
        .author()
        .map_err(|e| CommitError::DecodeCommit(Box::new(e)))?
        .into();
    let parents: Vec<gix::ObjectId> = original.parents().collect();
    let message: gix::bstr::BString = match message {
        Some(m) => m.into(),
        None => original.message.into(),
    };

    let tree_id = tree_from_index(repo)?;
    let committer = signature(committer_name, committer_email);

    // Unlike `commit()`, this can't go through `Repository::commit_as()`: that convenience
    // assumes it's creating a normal successor commit, where `HEAD` currently equals the new
    // commit's first parent, and rejects the ref update otherwise. Amend replaces `HEAD` itself
    // with a new commit that shares its *predecessor's* parents, so the object is written
    // directly and the `HEAD` update is expressed by hand, expecting the commit being replaced
    // as the ref's current value instead.
    let new_commit = gix::objs::Commit {
        tree: tree_id,
        parents: parents.into(),
        author,
        committer,
        encoding: None,
        message,
        extra_headers: Default::default(),
    };
    let new_id = repo
        .write_object(&new_commit)
        .map_err(|e| CommitError::Write(Box::new(e)))?
        .detach();

    let reference: gix::refs::FullName = "HEAD"
        .try_into()
        .map_err(|e: gix::validate::reference::name::Error| CommitError::Write(Box::new(e)))?;
    let edit = gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: gix::reference::log::message(
                    "commit (amend)",
                    new_commit.message.as_ref(),
                    new_commit.parents.len(),
                ),
            },
            expected: gix::refs::transaction::PreviousValue::MustExistAndMatch(
                gix::refs::Target::Object(original_id),
            ),
            new: gix::refs::Target::Object(new_id),
        },
        name: reference,
        deref: true,
    };
    repo.edit_reference(edit)
        .map_err(|e| CommitError::Write(Box::new(e)))?;
    Ok(new_id)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::staging::stage;

    fn init_repo(dir: &Path) -> gix::Repository {
        gix::init(dir).expect("init temp repo")
    }

    #[test]
    fn first_commit_has_no_parent_and_contains_staged_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();

        let commit_id =
            commit(&repo, "initial commit", "Lux Test", "lux-test@example.com").unwrap();

        let head_commit = repo.find_commit(commit_id).unwrap();
        let decoded = head_commit.decode().unwrap();
        assert_eq!(decoded.parents().count(), 0);
        assert_eq!(decoded.message, "initial commit");

        let tree = head_commit.tree().unwrap();
        let entry = tree
            .lookup_entry_by_path("a.rs")
            .unwrap()
            .expect("staged file is in the tree");
        let blob = repo.find_blob(entry.object_id()).unwrap();
        assert_eq!(blob.data, b"content\n");
    }

    #[test]
    fn second_commit_has_first_as_parent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let first = commit(&repo, "first", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("b.rs"), "two\n").unwrap();
        stage(&repo, Path::new("b.rs")).unwrap();
        let second = commit(&repo, "second", "Lux Test", "lux-test@example.com").unwrap();

        let decoded = repo.find_commit(second).unwrap();
        let parents: Vec<_> = decoded.parent_ids().map(|id| id.detach()).collect();
        assert_eq!(parents, vec![first]);

        // Files staged and committed in the first commit are still present in the tree,
        // since the tree builder starts from HEAD's tree, not an empty one.
        let tree = decoded.tree().unwrap();
        assert!(tree.lookup_entry_by_path("a.rs").unwrap().is_some());
        assert!(tree.lookup_entry_by_path("b.rs").unwrap().is_some());
    }

    #[test]
    fn commit_only_includes_staged_files_not_every_working_copy_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("staged.rs"), "staged\n").unwrap();
        fs::write(dir.path().join("unstaged.rs"), "unstaged\n").unwrap();
        stage(&repo, Path::new("staged.rs")).unwrap();

        let commit_id = commit(&repo, "partial", "Lux Test", "lux-test@example.com").unwrap();

        let tree = repo.find_commit(commit_id).unwrap().tree().unwrap();
        assert!(tree.lookup_entry_by_path("staged.rs").unwrap().is_some());
        assert!(tree.lookup_entry_by_path("unstaged.rs").unwrap().is_none());
    }

    #[test]
    fn amend_replaces_tree_and_message_but_keeps_parents_and_author() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("base.rs"), "base\n").unwrap();
        stage(&repo, Path::new("base.rs")).unwrap();
        commit(&repo, "root", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "one\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let original = commit(
            &repo,
            "original message",
            "Original Author",
            "original@example.com",
        )
        .unwrap();
        let original_decoded = repo.find_commit(original).unwrap();
        let original_parents: Vec<_> = original_decoded
            .parent_ids()
            .map(|id| id.detach())
            .collect();

        fs::write(dir.path().join("b.rs"), "two\n").unwrap();
        stage(&repo, Path::new("b.rs")).unwrap();
        let amended = amend(
            &repo,
            Some("amended message"),
            "Amender",
            "amender@example.com",
        )
        .unwrap();

        assert_ne!(amended, original, "amend produces a new commit object");
        let decoded = repo.find_commit(amended).unwrap();
        assert_eq!(decoded.message().unwrap().title, "amended message");
        let parents: Vec<_> = decoded.parent_ids().map(|id| id.detach()).collect();
        assert_eq!(
            parents, original_parents,
            "amend keeps the original parents, not the commit it replaces"
        );
        assert_eq!(
            decoded.author().unwrap().name,
            "Original Author",
            "amend preserves the original author"
        );
        assert_eq!(
            decoded.committer().unwrap().name,
            "Amender",
            "amend updates the committer"
        );

        let tree = decoded.tree().unwrap();
        assert!(tree.lookup_entry_by_path("a.rs").unwrap().is_some());
        assert!(
            tree.lookup_entry_by_path("b.rs").unwrap().is_some(),
            "amend picks up newly staged files"
        );
    }

    #[test]
    fn amend_without_a_new_message_keeps_the_original() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        commit(
            &repo,
            "keep this message",
            "Lux Test",
            "lux-test@example.com",
        )
        .unwrap();

        fs::write(dir.path().join("b.rs"), "more\n").unwrap();
        stage(&repo, Path::new("b.rs")).unwrap();
        let amended = amend(&repo, None, "Lux Test", "lux-test@example.com").unwrap();

        let decoded = repo.find_commit(amended).unwrap();
        assert_eq!(decoded.message().unwrap().title, "keep this message");
    }

    #[test]
    fn amending_an_unborn_head_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        assert!(
            amend(
                &repo,
                Some("nothing to amend"),
                "Lux Test",
                "lux-test@example.com"
            )
            .is_err()
        );
    }
}
