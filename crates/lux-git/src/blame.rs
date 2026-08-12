//! Line-level and per-commit blame, using `gix`'s own blame algorithm —
//! no shell-out, no hand-rolled line-attribution (ADR 0010).

use std::fmt;
use std::path::Path;

use gix::bstr::BStr;

/// One contiguous run of lines in the blamed file that a single commit
/// introduced — the "por commit" view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameHunk {
    /// 1-based first line in the blamed file this hunk covers.
    pub start_line: u32,
    pub len: u32,
    pub commit_id: gix::ObjectId,
}

/// One line of the blamed file, attributed to the commit that introduced
/// it — the "en línea" view, for inline gutter-style annotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    /// 1-based line number in the blamed file.
    pub line: u32,
    pub commit_id: gix::ObjectId,
}

/// Failure computing blame for a file.
#[derive(Debug)]
pub enum BlameError {
    NonUtf8Path,
    Blame(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for BlameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlameError::NonUtf8Path => write!(f, "path is not valid UTF-8"),
            BlameError::Blame(e) => write!(f, "failed to compute blame: {e}"),
        }
    }
}

impl std::error::Error for BlameError {}

fn path_to_bstr(path: &Path) -> Result<&BStr, BlameError> {
    path.to_str().map(Into::into).ok_or(BlameError::NonUtf8Path)
}

/// Blames `path` as it exists at `commit`, one hunk per contiguous run of
/// lines a single commit introduced — the shape most blame UIs render
/// directly (a gutter annotation that only repeats the author/commit on
/// the first line of each run).
pub fn blame_by_commit(
    repo: &gix::Repository,
    path: &Path,
    commit: gix::ObjectId,
) -> Result<Vec<BlameHunk>, BlameError> {
    let file_path = path_to_bstr(path)?;
    let outcome = repo
        .blame_file(
            file_path,
            commit,
            gix::repository::blame_file::Options::default(),
        )
        .map_err(|e| BlameError::Blame(Box::new(e)))?;

    Ok(outcome
        .entries
        .iter()
        .map(|entry| BlameHunk {
            start_line: entry.start_in_blamed_file + 1,
            len: entry.len.get(),
            commit_id: entry.commit_id,
        })
        .collect())
}

/// Expands `hunks` into one entry per line, for inline per-line
/// annotations. Pure transform — doesn't recompute the blame.
pub fn expand_to_lines(hunks: &[BlameHunk]) -> Vec<BlameLine> {
    hunks
        .iter()
        .flat_map(|hunk| {
            (hunk.start_line..hunk.start_line + hunk.len).map(|line| BlameLine {
                line,
                commit_id: hunk.commit_id,
            })
        })
        .collect()
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
    fn blame_attributes_every_line_of_a_single_commit_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\ntwo\nthree\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let commit_id = commit(&repo, "add a", "Lux Test", "lux-test@example.com").unwrap();

        let hunks = blame_by_commit(&repo, Path::new("a.rs"), commit_id).unwrap();
        assert_eq!(
            hunks,
            vec![BlameHunk {
                start_line: 1,
                len: 3,
                commit_id
            }]
        );

        let lines = expand_to_lines(&hunks);
        assert_eq!(
            lines,
            vec![
                BlameLine { line: 1, commit_id },
                BlameLine { line: 2, commit_id },
                BlameLine { line: 3, commit_id },
            ]
        );
    }

    #[test]
    fn blame_attributes_added_lines_to_the_commit_that_added_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\ntwo\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let first = commit(&repo, "first", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "one\ntwo\nthree\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let second = commit(&repo, "second", "Lux Test", "lux-test@example.com").unwrap();

        let hunks = blame_by_commit(&repo, Path::new("a.rs"), second).unwrap();
        assert_eq!(
            hunks,
            vec![
                BlameHunk {
                    start_line: 1,
                    len: 2,
                    commit_id: first
                },
                BlameHunk {
                    start_line: 3,
                    len: 1,
                    commit_id: second
                }
            ]
        );
    }

    #[test]
    fn blame_attributes_an_edited_line_to_the_commit_that_edited_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\ntwo\nthree\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let first = commit(&repo, "first", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "one\nTWO\nthree\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let second = commit(&repo, "second", "Lux Test", "lux-test@example.com").unwrap();

        let hunks = blame_by_commit(&repo, Path::new("a.rs"), second).unwrap();
        assert_eq!(
            hunks,
            vec![
                BlameHunk {
                    start_line: 1,
                    len: 1,
                    commit_id: first
                },
                BlameHunk {
                    start_line: 2,
                    len: 1,
                    commit_id: second
                },
                BlameHunk {
                    start_line: 3,
                    len: 1,
                    commit_id: first
                },
            ]
        );
    }

    #[test]
    fn blaming_an_earlier_commit_ignores_later_history() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = init_repo(dir.path());
        fs::write(dir.path().join("a.rs"), "one\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        let first = commit(&repo, "first", "Lux Test", "lux-test@example.com").unwrap();

        fs::write(dir.path().join("a.rs"), "ONE\n").unwrap();
        stage(&repo, Path::new("a.rs")).unwrap();
        commit(&repo, "second", "Lux Test", "lux-test@example.com").unwrap();

        let hunks = blame_by_commit(&repo, Path::new("a.rs"), first).unwrap();
        assert_eq!(
            hunks,
            vec![BlameHunk {
                start_line: 1,
                len: 1,
                commit_id: first
            }]
        );
    }
}
