//! Repository-wide and per-file commit history, in-process via `gix`'s
//! revision walk (no shell-out — ADR 0010).

use std::fmt;
use std::path::Path;

use gix::bstr::ByteSlice;
use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;

/// The information a history view needs about one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub id: gix::ObjectId,
    pub parents: Vec<gix::ObjectId>,
    pub author_name: String,
    pub author_email: String,
    pub time: gix::date::Time,
    /// The message's first line.
    pub summary: String,
    pub message: String,
}

/// Failure walking history or reading a commit.
#[derive(Debug)]
pub enum HistoryError {
    Walk(Box<dyn std::error::Error + Send + Sync>),
    ReadCommit(Box<dyn std::error::Error + Send + Sync>),
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    MessageNotUtf8,
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HistoryError::Walk(e) => write!(f, "failed to walk history: {e}"),
            HistoryError::ReadCommit(e) => write!(f, "failed to read commit: {e}"),
            HistoryError::ReadTree(e) => write!(f, "failed to read tree: {e}"),
            HistoryError::MessageNotUtf8 => write!(f, "commit message is not valid UTF-8"),
        }
    }
}

impl std::error::Error for HistoryError {}

fn commit_info(repo: &gix::Repository, id: gix::ObjectId) -> Result<CommitInfo, HistoryError> {
    let commit = repo
        .find_commit(id)
        .map_err(|e| HistoryError::ReadCommit(Box::new(e)))?;
    let decoded = commit
        .decode()
        .map_err(|e| HistoryError::ReadCommit(Box::new(e)))?;
    let author = decoded
        .author()
        .map_err(|e| HistoryError::ReadCommit(Box::new(e)))?;
    let message = decoded
        .message
        .to_str()
        .map_err(|_| HistoryError::MessageNotUtf8)?;
    let summary = message.lines().next().unwrap_or_default().to_string();
    Ok(CommitInfo {
        id,
        parents: decoded.parents().collect(),
        author_name: author.name.to_string(),
        author_email: author.email.to_string(),
        time: author
            .time()
            .map_err(|e| HistoryError::ReadCommit(Box::new(e)))?,
        summary,
        message: message.to_string(),
    })
}

/// Walks every commit reachable from `start`, newest first.
fn walk(
    repo: &gix::Repository,
    start: gix::ObjectId,
) -> Result<impl Iterator<Item = Result<gix::ObjectId, HistoryError>> + '_, HistoryError> {
    let walk = repo
        .rev_walk([start])
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .all()
        .map_err(|e| HistoryError::Walk(Box::new(e)))?;
    Ok(walk.map(|info| {
        info.map(|i| i.id)
            .map_err(|e| HistoryError::Walk(Box::new(e)))
    }))
}

/// Every commit reachable from `start`, newest first — the repository's
/// full history.
pub fn repository_history(
    repo: &gix::Repository,
    start: gix::ObjectId,
) -> Result<Vec<CommitInfo>, HistoryError> {
    walk(repo, start)?
        .map(|id| commit_info(repo, id?))
        .collect()
}

/// The blob id `path` resolves to in `commit`'s tree, or `None` if `path`
/// doesn't exist there.
fn blob_id_at(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    path: &Path,
) -> Result<Option<gix::ObjectId>, HistoryError> {
    let commit = repo
        .find_commit(commit)
        .map_err(|e| HistoryError::ReadCommit(Box::new(e)))?;
    let tree = commit
        .tree()
        .map_err(|e| HistoryError::ReadTree(Box::new(e)))?;
    Ok(tree
        .lookup_entry_by_path(path)
        .map_err(|e| HistoryError::ReadTree(Box::new(e)))?
        .map(|entry| entry.object_id()))
}

/// Every commit reachable from `start` that actually changed `path`'s
/// content, newest first — the commits `path`'s own history view would
/// show. A commit counts as touching `path` when its blob there differs
/// from every one of its parents' (or, for a root commit with no
/// parents, when `path` simply exists); this mirrors `git log`'s default
/// path-limited behavior of hiding a merge that's identical to at least
/// one parent, without implementing full history simplification.
pub fn file_history(
    repo: &gix::Repository,
    start: gix::ObjectId,
    path: &Path,
) -> Result<Vec<CommitInfo>, HistoryError> {
    let mut infos = Vec::new();
    for id in walk(repo, start)? {
        let id = id?;
        let info = commit_info(repo, id)?;
        let blob = blob_id_at(repo, id, path)?;

        let touched = if info.parents.is_empty() {
            blob.is_some()
        } else {
            let mut differs_from_every_parent = true;
            for parent in &info.parents {
                if blob_id_at(repo, *parent, path)? == blob {
                    differs_from_every_parent = false;
                    break;
                }
            }
            differs_from_every_parent
        };

        if touched {
            infos.push(info);
        }
    }
    Ok(infos)
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

    #[test]
    fn repository_history_lists_every_commit_newest_first() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let first = commit(&repo, "first", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "two\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let second = commit(&repo, "second", "Lux Test", "lux-test@example.com").unwrap();

        let history = repository_history(&repo, second).unwrap();
        let ids: Vec<_> = history.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![second, first], "newest first");
        assert_eq!(history[0].summary, "second");
        assert_eq!(history[0].parents, vec![first]);
        assert_eq!(history[1].parents, Vec::<gix::ObjectId>::new());
    }

    #[test]
    fn commit_info_exposes_author_and_message() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let id = commit(
            &repo,
            "multi-line\n\nbody text",
            "Lux Test",
            "lux-test@example.com",
        )
        .unwrap();

        let info = commit_info(&repo, id).unwrap();
        assert_eq!(info.author_name, "Lux Test");
        assert_eq!(info.author_email, "lux-test@example.com");
        assert_eq!(info.summary, "multi-line");
        assert_eq!(info.message, "multi-line\n\nbody text");
    }

    #[test]
    fn file_history_only_includes_commits_that_touched_the_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());

        fs::write(dir.path().join("a.rs"), "one\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let touches_a_1 = commit(&repo, "add a", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("b.rs"), "unrelated\n").unwrap();
        stage(&repo, Path::new("b.rs")).unwrap();
        commit(
            &repo,
            "add b, doesn't touch a",
            "Lux Test",
            "lux-test@example.com",
        )
        .unwrap();

        fs::write(dir.path().join("a.rs"), "two\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let touches_a_2 = commit(&repo, "edit a", "Lux Test", "lux-test@example.com").unwrap();

        let history = file_history(&repo, touches_a_2, Path::new("a.rs")).unwrap();
        let ids: Vec<_> = history.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![touches_a_2, touches_a_1]);
    }

    #[test]
    fn file_history_of_a_nonexistent_path_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "content\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let id = commit(&repo, "only a", "Lux Test", "lux-test@example.com").unwrap();

        let history = file_history(&repo, id, Path::new("never-existed.rs")).unwrap();
        assert!(history.is_empty());
    }
}
