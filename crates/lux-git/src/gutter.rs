//! Gutter change indicators and hunk navigation.
//!
//! Diffs a file's current working-copy text against its content at `HEAD`,
//! in-process via `gix` (see `docs/adr/0010-git-backend-gix.md` — no
//! shell-out to the `git` binary, on any platform).

use std::fmt;
use std::path::Path;

use gix::diff::blob::{Algorithm, InternedInput, diff_with_slider_heuristics};

/// One contiguous block of changed lines, in 1-based line numbers.
///
/// `before_start`/`after_start` follow the same convention `gix` itself
/// uses for unified-diff headers: for a pure insertion or deletion
/// (`*_len == 0`) the start points at the line immediately preceding where
/// the change happens, since the change owns no line on that side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hunk {
    pub before_start: u32,
    pub before_len: u32,
    pub after_start: u32,
    pub after_len: u32,
}

impl Hunk {
    pub fn kind(&self) -> HunkKind {
        if self.before_len == 0 {
            HunkKind::Added
        } else if self.after_len == 0 {
            HunkKind::Removed
        } else {
            HunkKind::Modified
        }
    }

    /// The working-copy line this hunk anchors to, for gutter placement and
    /// navigation: its first line, or — for a pure deletion, which owns no
    /// line of its own in the working copy — the line right after it.
    pub fn anchor_line(&self) -> u32 {
        self.after_start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkKind {
    Added,
    Removed,
    Modified,
}

/// Diffs `before` against `after` and returns one [`Hunk`] per contiguous
/// change, in file order. Pure text operation, no repository involved.
pub fn diff_hunks(before: &str, after: &str) -> Vec<Hunk> {
    let input = InternedInput::new(before, after);
    let diff = diff_with_slider_heuristics(Algorithm::Histogram, &input);
    diff.hunks()
        .map(|h| Hunk {
            before_start: h.before.start + 1,
            before_len: h.before.end - h.before.start,
            after_start: h.after.start + 1,
            after_len: h.after.end - h.after.start,
        })
        .collect()
}

/// One gutter-line decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GutterMarker {
    /// 1-based working-copy line this marker decorates.
    pub line: u32,
    pub kind: HunkKind,
}

/// Expands `hunks` into one marker per affected working-copy line. A pure
/// deletion produces no line of its own to mark, so its marker sits on
/// [`Hunk::anchor_line`] instead — the same place most editors render a
/// "removed" indicator between two lines.
pub fn gutter_markers(hunks: &[Hunk]) -> Vec<GutterMarker> {
    let mut markers = Vec::new();
    for hunk in hunks {
        let kind = hunk.kind();
        if hunk.after_len == 0 {
            markers.push(GutterMarker {
                line: hunk.anchor_line(),
                kind,
            });
        } else {
            markers.extend(
                (hunk.after_start..hunk.after_start + hunk.after_len)
                    .map(|line| GutterMarker { line, kind }),
            );
        }
    }
    markers
}

/// Navigates between [`Hunk`]s by working-copy line, the way an editor's
/// "next/previous change" command does: wraps around at either end so it
/// never dead-ends.
pub struct HunkNavigator<'a> {
    hunks: &'a [Hunk],
}

impl<'a> HunkNavigator<'a> {
    pub fn new(hunks: &'a [Hunk]) -> Self {
        Self { hunks }
    }

    /// The first hunk whose anchor line is after `line`, wrapping to the
    /// first hunk overall if there is none.
    pub fn next_from(&self, line: u32) -> Option<&'a Hunk> {
        self.hunks
            .iter()
            .find(|h| h.anchor_line() > line)
            .or_else(|| self.hunks.first())
    }

    /// The last hunk whose anchor line is before `line`, wrapping to the
    /// last hunk overall if there is none.
    pub fn previous_from(&self, line: u32) -> Option<&'a Hunk> {
        self.hunks
            .iter()
            .rev()
            .find(|h| h.anchor_line() < line)
            .or_else(|| self.hunks.last())
    }
}

/// Failure reading a file's `HEAD` content to diff against.
#[derive(Debug)]
pub enum GutterError {
    HeadTree(gix::reference::head_tree_id::Error),
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    ReadBlob(Box<dyn std::error::Error + Send + Sync>),
    NotUtf8,
}

impl fmt::Display for GutterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GutterError::HeadTree(e) => write!(f, "failed to resolve HEAD tree: {e}"),
            GutterError::ReadTree(e) => write!(f, "failed to read tree entry: {e}"),
            GutterError::ReadBlob(e) => write!(f, "failed to read blob: {e}"),
            GutterError::NotUtf8 => write!(f, "file content at HEAD is not valid UTF-8"),
        }
    }
}

impl std::error::Error for GutterError {}

/// Reads `path`'s content at `HEAD` in `repo`, or `None` if it doesn't
/// exist there (a file that's new since the last commit).
fn head_blob_utf8(repo: &gix::Repository, path: &Path) -> Result<Option<String>, GutterError> {
    let tree_id = repo
        .head_tree_id_or_empty()
        .map_err(GutterError::HeadTree)?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| GutterError::ReadTree(Box::new(e)))?;
    let Some(entry) = tree
        .lookup_entry_by_path(path)
        .map_err(|e| GutterError::ReadTree(Box::new(e)))?
    else {
        return Ok(None);
    };
    let object = entry
        .object()
        .map_err(|e| GutterError::ReadBlob(Box::new(e)))?;
    String::from_utf8(object.data.clone())
        .map(Some)
        .map_err(|_| GutterError::NotUtf8)
}

/// Diffs `path`'s working-copy content (`working_copy`) against its content
/// at `HEAD` in `repo`. A file that doesn't exist at `HEAD` yet diffs
/// against an empty string, so the whole file comes back as one `Added`
/// hunk.
pub fn hunks_against_head(
    repo: &gix::Repository,
    path: &Path,
    working_copy: &str,
) -> Result<Vec<Hunk>, GutterError> {
    let before = head_blob_utf8(repo, path)?.unwrap_or_default();
    Ok(diff_hunks(&before, working_copy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes_produces_no_hunks() {
        let text = "fn main() {\n    println!(\"hi\");\n}\n";
        assert!(diff_hunks(text, text).is_empty());
    }

    #[test]
    fn pure_insertion_is_added() {
        let before = "one\ntwo\nthree\n";
        let after = "one\ntwo\ntwo-and-a-half\nthree\n";
        let hunks = diff_hunks(before, after);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].kind(), HunkKind::Added);
        assert_eq!(hunks[0].before_len, 0);
        assert_eq!(hunks[0].after_start, 3);
        assert_eq!(hunks[0].after_len, 1);
    }

    #[test]
    fn pure_deletion_is_removed() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nthree\n";
        let hunks = diff_hunks(before, after);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].kind(), HunkKind::Removed);
        assert_eq!(hunks[0].after_len, 0);
        assert_eq!(hunks[0].before_start, 2);
        assert_eq!(hunks[0].before_len, 1);
    }

    #[test]
    fn replaced_line_is_modified() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nTWO\nthree\n";
        let hunks = diff_hunks(before, after);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].kind(), HunkKind::Modified);
        assert_eq!(hunks[0].before_start, 2);
        assert_eq!(hunks[0].after_start, 2);
    }

    #[test]
    fn separate_edits_yield_separate_hunks_in_order() {
        let before = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let after = "A\nb\nc\nd\ne\nf\ng\nH\n";
        let hunks = diff_hunks(before, after);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].after_start, 1);
        assert_eq!(hunks[1].after_start, 8);
    }

    #[test]
    fn markers_cover_every_changed_line_for_multi_line_hunks() {
        let before = "keep\n";
        let after = "keep\nnew1\nnew2\nnew3\n";
        let hunks = diff_hunks(before, after);
        let markers = gutter_markers(&hunks);
        assert_eq!(
            markers,
            vec![
                GutterMarker {
                    line: 2,
                    kind: HunkKind::Added
                },
                GutterMarker {
                    line: 3,
                    kind: HunkKind::Added
                },
                GutterMarker {
                    line: 4,
                    kind: HunkKind::Added
                },
            ]
        );
    }

    #[test]
    fn deletion_marker_anchors_after_the_deletion_point() {
        let before = "one\ntwo\nthree\nfour\n";
        let after = "one\nfour\n";
        let hunks = diff_hunks(before, after);
        let markers = gutter_markers(&hunks);
        assert_eq!(
            markers,
            vec![GutterMarker {
                line: 2,
                kind: HunkKind::Removed
            }],
            "anchors on \"four\", the line right after the deletion"
        );
    }

    #[test]
    fn navigator_advances_and_wraps() {
        let hunks = vec![
            Hunk {
                before_start: 0,
                before_len: 0,
                after_start: 3,
                after_len: 1,
            },
            Hunk {
                before_start: 0,
                before_len: 0,
                after_start: 10,
                after_len: 1,
            },
        ];
        let nav = HunkNavigator::new(&hunks);

        assert_eq!(nav.next_from(0).unwrap().after_start, 3);
        assert_eq!(nav.next_from(3).unwrap().after_start, 10);
        assert_eq!(
            nav.next_from(10).unwrap().after_start,
            3,
            "wraps to the first hunk"
        );

        assert_eq!(nav.previous_from(10).unwrap().after_start, 3);
        assert_eq!(
            nav.previous_from(3).unwrap().after_start,
            10,
            "wraps to the last hunk"
        );
    }

    #[test]
    fn navigator_on_empty_hunks_returns_none() {
        let nav = HunkNavigator::new(&[]);
        assert_eq!(nav.next_from(5), None);
        assert_eq!(nav.previous_from(5), None);
    }

    #[test]
    fn navigator_with_single_hunk_stays_put() {
        let hunks = vec![Hunk {
            before_start: 0,
            before_len: 0,
            after_start: 5,
            after_len: 1,
        }];
        let nav = HunkNavigator::new(&hunks);
        assert_eq!(nav.next_from(5).unwrap().after_start, 5);
        assert_eq!(nav.previous_from(5).unwrap().after_start, 5);
    }

    mod against_real_repo {
        use std::fs;

        use super::*;

        fn signature() -> gix::actor::SignatureRef<'static> {
            gix::actor::SignatureRef::from_bytes(
                b"Lux Test <lux-test@example.com> 1700000000 +0000",
            )
            .unwrap()
        }

        fn init_repo_with_committed_file(dir: &Path, path: &str, content: &str) -> gix::Repository {
            let repo = gix::init(dir).expect("init temp repo");
            let blob_id = repo.write_blob(content.as_bytes()).expect("write blob");
            let entry = gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: path.into(),
                oid: blob_id.detach(),
            };
            let tree = gix::objs::Tree {
                entries: vec![entry],
            };
            let tree_id = repo.write_object(&tree).expect("write tree");
            let sig = signature();
            repo.commit_as(
                sig,
                sig,
                "HEAD",
                "initial commit",
                tree_id.detach(),
                std::iter::empty::<gix::ObjectId>(),
            )
            .expect("commit");
            repo
        }

        #[test]
        fn diffs_working_copy_against_head() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo =
                init_repo_with_committed_file(dir.path(), "lib.rs", "fn a() {}\nfn b() {}\n");

            let working_copy = "fn a() {}\nfn b() {}\nfn c() {}\n";
            let hunks = hunks_against_head(&repo, Path::new("lib.rs"), working_copy)
                .expect("diff against HEAD");

            assert_eq!(hunks.len(), 1);
            assert_eq!(hunks[0].kind(), HunkKind::Added);
            assert_eq!(hunks[0].after_start, 3);
        }

        #[test]
        fn new_file_not_yet_at_head_is_all_added() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_file(dir.path(), "committed.rs", "existing\n");

            let working_copy = "brand\nnew\nfile\n";
            let hunks = hunks_against_head(&repo, Path::new("new.rs"), working_copy)
                .expect("diff against HEAD");

            assert_eq!(hunks.len(), 1);
            assert_eq!(hunks[0].kind(), HunkKind::Added);
            assert_eq!(hunks[0].after_len, 3);
        }

        #[test]
        fn unborn_head_diffs_against_empty() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = gix::init(dir.path()).expect("init temp repo");
            fs::write(dir.path().join("f.rs"), "line\n").unwrap();

            let hunks =
                hunks_against_head(&repo, Path::new("f.rs"), "line\n").expect("diff against HEAD");
            assert_eq!(hunks.len(), 1);
            assert_eq!(hunks[0].kind(), HunkKind::Added);
        }
    }
}
