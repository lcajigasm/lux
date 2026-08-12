//! Changes tree (grouped by folder) and side-by-side diff view model.
//!
//! Scope: this module answers "which files changed, and what does one of
//! them look like side by side" — it doesn't stage, commit, or write
//! anything. Reading the working tree happens in-process via `gix` and the
//! same `ignore`-crate gitignore matcher `lux-search`/`lux-workspace`
//! already use elsewhere in this workspace (Phase 4); no shell-out to
//! `git` (see `docs/adr/0010-git-backend-gix.md`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use crate::gutter::diff_hunks;

/// Where a changed path stands relative to `HEAD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
}

/// One changed path, relative to the repository's working directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub status: ChangeStatus,
}

/// Failure reading a repository's `HEAD` tree or its working directory.
#[derive(Debug)]
pub enum DiffPanelError {
    BareRepository,
    HeadTree(gix::reference::head_tree_id::Error),
    ReadTree(Box<dyn std::error::Error + Send + Sync>),
    ReadBlob(Box<dyn std::error::Error + Send + Sync>),
    Walk(ignore::Error),
    Io(std::io::Error),
}

impl fmt::Display for DiffPanelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffPanelError::BareRepository => write!(f, "repository has no working directory"),
            DiffPanelError::HeadTree(e) => write!(f, "failed to resolve HEAD tree: {e}"),
            DiffPanelError::ReadTree(e) => write!(f, "failed to read HEAD tree: {e}"),
            DiffPanelError::ReadBlob(e) => write!(f, "failed to read blob: {e}"),
            DiffPanelError::Walk(e) => write!(f, "failed to walk working directory: {e}"),
            DiffPanelError::Io(e) => write!(f, "failed to read working directory: {e}"),
        }
    }
}

impl std::error::Error for DiffPanelError {}

/// Compares `repo`'s on-disk working tree (respecting `.gitignore`, same as
/// `git status`) against its `HEAD` blobs, returning one [`ChangedFile`]
/// per path that differs, sorted by path for deterministic, folder-grouped
/// display.
pub fn changed_files(repo: &gix::Repository) -> Result<Vec<ChangedFile>, DiffPanelError> {
    let work_dir = repo.workdir().ok_or(DiffPanelError::BareRepository)?;

    let tree_id = repo
        .head_tree_id_or_empty()
        .map_err(DiffPanelError::HeadTree)?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| DiffPanelError::ReadTree(Box::new(e)))?;
    let head_entries = tree
        .traverse()
        .breadthfirst
        .files()
        .map_err(|e| DiffPanelError::ReadTree(Box::new(e)))?;
    let head_blobs: BTreeMap<PathBuf, gix::ObjectId> = head_entries
        .into_iter()
        .filter(|entry| entry.mode.is_blob())
        .map(|entry| (PathBuf::from(entry.filepath.to_string()), entry.oid))
        .collect();

    let mut seen = BTreeSet::new();
    let mut changed = Vec::new();
    let walker = ignore::WalkBuilder::new(work_dir).hidden(false).build();
    for result in walker {
        let entry = result.map_err(DiffPanelError::Walk)?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if path.components().any(|c| c.as_os_str() == ".git") {
            continue;
        }
        let rel = path
            .strip_prefix(work_dir)
            .expect("walker yields paths under work_dir")
            .to_path_buf();
        seen.insert(rel.clone());

        match head_blobs.get(&rel) {
            Some(oid) => {
                let disk_bytes = std::fs::read(path).map_err(DiffPanelError::Io)?;
                let blob = repo
                    .find_blob(*oid)
                    .map_err(|e| DiffPanelError::ReadBlob(Box::new(e)))?;
                if blob.data != disk_bytes {
                    changed.push(ChangedFile {
                        path: rel,
                        status: ChangeStatus::Modified,
                    });
                }
            }
            None => changed.push(ChangedFile {
                path: rel,
                status: ChangeStatus::Added,
            }),
        }
    }

    for path in head_blobs.keys() {
        if !seen.contains(path) {
            changed.push(ChangedFile {
                path: path.clone(),
                status: ChangeStatus::Deleted,
            });
        }
    }

    changed.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(changed)
}

/// A folder in the changes tree, grouping its direct file changes and
/// nested subfolders. The root folder's own `name` is empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeTreeFolder {
    pub name: String,
    pub folders: Vec<ChangeTreeFolder>,
    pub files: Vec<ChangedFile>,
}

impl ChangeTreeFolder {
    fn child(&mut self, name: &str) -> &mut ChangeTreeFolder {
        if let Some(index) = self.folders.iter().position(|f| f.name == name) {
            &mut self.folders[index]
        } else {
            self.folders.push(ChangeTreeFolder {
                name: name.to_string(),
                ..Default::default()
            });
            self.folders.last_mut().expect("just pushed")
        }
    }
}

/// Groups a flat list of changed files into a tree by folder, the shape a
/// "changes" panel renders (folders expandable, files as leaves).
pub fn group_by_folder(files: Vec<ChangedFile>) -> ChangeTreeFolder {
    let mut root = ChangeTreeFolder::default();
    for file in files {
        let mut components: Vec<String> = file
            .path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let Some(_file_name) = components.pop() else {
            continue;
        };
        let mut folder = &mut root;
        for name in &components {
            folder = folder.child(name);
        }
        folder.files.push(file);
    }
    sort_tree(&mut root);
    root
}

fn sort_tree(folder: &mut ChangeTreeFolder) {
    folder.folders.sort_by(|a, b| a.name.cmp(&b.name));
    folder.files.sort_by(|a, b| a.path.cmp(&b.path));
    for child in &mut folder.folders {
        sort_tree(child);
    }
}

/// A single row of a side-by-side diff: `before`/`after` are `None` where
/// that side has nothing to show (padding, so the other side's changed
/// lines still line up visually).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideRow {
    pub before: Option<SideBySideLine>,
    pub after: Option<SideBySideLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideBySideLineKind {
    Context,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideLine {
    /// 1-based line number in its own file.
    pub number: u32,
    pub text: String,
    pub kind: SideBySideLineKind,
}

/// Builds a side-by-side (split) diff of `before` against `after`: every
/// line of both files, paired up so unchanged lines align and changed
/// hunks sit next to each other, with blank padding on whichever side has
/// fewer lines in a given hunk.
pub fn side_by_side(before: &str, after: &str) -> Vec<SideBySideRow> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let hunks = diff_hunks(before, after);

    let mut rows = Vec::new();
    let mut before_pos = 0usize;
    let mut after_pos = 0usize;

    for hunk in &hunks {
        let hunk_before_start = (hunk.before_start - 1) as usize;
        let hunk_after_start = (hunk.after_start - 1) as usize;
        push_context(
            &mut rows,
            &before_lines,
            &after_lines,
            &mut before_pos,
            &mut after_pos,
            hunk_before_start,
            hunk_after_start,
        );

        let before_len = hunk.before_len as usize;
        let after_len = hunk.after_len as usize;
        for i in 0..before_len.max(after_len) {
            rows.push(SideBySideRow {
                before: (i < before_len)
                    .then(|| line(&before_lines, before_pos + i, SideBySideLineKind::Changed)),
                after: (i < after_len)
                    .then(|| line(&after_lines, after_pos + i, SideBySideLineKind::Changed)),
            });
        }
        before_pos += before_len;
        after_pos += after_len;
    }

    push_context(
        &mut rows,
        &before_lines,
        &after_lines,
        &mut before_pos,
        &mut after_pos,
        before_lines.len(),
        after_lines.len(),
    );
    rows
}

/// Appends unchanged (context) rows until either side reaches its target
/// position, advancing both cursors in lockstep.
#[allow(clippy::too_many_arguments)]
fn push_context(
    rows: &mut Vec<SideBySideRow>,
    before_lines: &[&str],
    after_lines: &[&str],
    before_pos: &mut usize,
    after_pos: &mut usize,
    before_end: usize,
    after_end: usize,
) {
    while *before_pos < before_end && *after_pos < after_end {
        rows.push(SideBySideRow {
            before: Some(line(before_lines, *before_pos, SideBySideLineKind::Context)),
            after: Some(line(after_lines, *after_pos, SideBySideLineKind::Context)),
        });
        *before_pos += 1;
        *after_pos += 1;
    }
}

fn line(lines: &[&str], index: usize, kind: SideBySideLineKind) -> SideBySideLine {
    SideBySideLine {
        number: index as u32 + 1,
        text: lines[index].to_string(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(path: &str, status: ChangeStatus) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            status,
        }
    }

    #[test]
    fn groups_files_at_root() {
        let tree = group_by_folder(vec![
            changed("a.rs", ChangeStatus::Modified),
            changed("b.rs", ChangeStatus::Added),
        ]);
        assert!(tree.folders.is_empty());
        assert_eq!(tree.files.len(), 2);
        assert_eq!(tree.files[0].path, PathBuf::from("a.rs"));
        assert_eq!(tree.files[1].path, PathBuf::from("b.rs"));
    }

    #[test]
    fn groups_nested_files_under_shared_folders() {
        let tree = group_by_folder(vec![
            changed("src/lib.rs", ChangeStatus::Modified),
            changed("src/gutter.rs", ChangeStatus::Added),
            changed("docs/README.md", ChangeStatus::Deleted),
        ]);
        assert_eq!(tree.files.len(), 0);
        assert_eq!(tree.folders.len(), 2, "src and docs");

        let docs = tree.folders.iter().find(|f| f.name == "docs").unwrap();
        assert_eq!(docs.files.len(), 1);
        assert_eq!(docs.files[0].path, PathBuf::from("docs/README.md"));

        let src = tree.folders.iter().find(|f| f.name == "src").unwrap();
        assert_eq!(src.files.len(), 2);
        assert_eq!(
            src.files[0].path,
            PathBuf::from("src/gutter.rs"),
            "sorted by full path"
        );
    }

    #[test]
    fn groups_deeply_nested_files_creating_intermediate_folders() {
        let tree = group_by_folder(vec![changed("a/b/c/d.rs", ChangeStatus::Added)]);
        let a = &tree.folders[0];
        assert_eq!(a.name, "a");
        let b = &a.folders[0];
        assert_eq!(b.name, "b");
        let c = &b.folders[0];
        assert_eq!(c.name, "c");
        assert_eq!(c.files[0].path, PathBuf::from("a/b/c/d.rs"));
    }

    #[test]
    fn side_by_side_of_identical_text_is_all_context() {
        let text = "one\ntwo\nthree\n";
        let rows = side_by_side(text, text);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| {
            r.before.as_ref().unwrap().kind == SideBySideLineKind::Context
                && r.after.as_ref().unwrap().kind == SideBySideLineKind::Context
        }));
    }

    #[test]
    fn side_by_side_pairs_a_modified_line() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nTWO\nthree\n";
        let rows = side_by_side(before, after);
        assert_eq!(rows.len(), 3);
        let changed_row = &rows[1];
        assert_eq!(changed_row.before.as_ref().unwrap().text, "two");
        assert_eq!(changed_row.after.as_ref().unwrap().text, "TWO");
        assert_eq!(
            changed_row.before.as_ref().unwrap().kind,
            SideBySideLineKind::Changed
        );
    }

    #[test]
    fn side_by_side_pads_the_shorter_side_on_pure_insertion() {
        let before = "one\ntwo\n";
        let after = "one\nnew1\nnew2\ntwo\n";
        let rows = side_by_side(before, after);
        // context "one", two blank-before/added rows, context "two".
        assert_eq!(rows.len(), 4);
        assert!(rows[1].before.is_none());
        assert_eq!(rows[1].after.as_ref().unwrap().text, "new1");
        assert!(rows[2].before.is_none());
        assert_eq!(rows[2].after.as_ref().unwrap().text, "new2");
        assert_eq!(rows[3].before.as_ref().unwrap().text, "two");
    }

    #[test]
    fn side_by_side_pads_the_shorter_side_on_pure_deletion() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nthree\n";
        let rows = side_by_side(before, after);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].before.as_ref().unwrap().text, "two");
        assert!(rows[1].after.is_none());
    }

    mod against_real_repo {
        use std::fs;
        use std::path::Path;

        use super::*;

        fn signature() -> gix::actor::SignatureRef<'static> {
            gix::actor::SignatureRef::from_bytes(
                b"Lux Test <lux-test@example.com> 1700000000 +0000",
            )
            .unwrap()
        }

        fn init_repo_with_committed_files(dir: &Path, files: &[(&str, &str)]) -> gix::Repository {
            let repo = gix::init(dir).expect("init temp repo");
            let mut entries: Vec<_> = files
                .iter()
                .map(|(path, content)| {
                    let blob_id = repo.write_blob(content.as_bytes()).expect("write blob");
                    gix::objs::tree::Entry {
                        mode: gix::objs::tree::EntryKind::Blob.into(),
                        filename: (*path).into(),
                        oid: blob_id.detach(),
                    }
                })
                .collect();
            entries.sort();
            let tree_id = repo
                .write_object(&gix::objs::Tree { entries })
                .expect("write tree");
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
            for (path, content) in files {
                fs::write(dir.join(path), content).expect("write working copy file");
            }
            repo
        }

        #[test]
        fn detects_added_modified_and_deleted_files() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_files(
                dir.path(),
                &[
                    ("keep.rs", "unchanged\n"),
                    ("edit.rs", "old\n"),
                    ("remove.rs", "gone soon\n"),
                ],
            );

            fs::write(dir.path().join("edit.rs"), "new\n").unwrap();
            fs::remove_file(dir.path().join("remove.rs")).unwrap();
            fs::write(dir.path().join("new.rs"), "brand new\n").unwrap();

            let mut files = changed_files(&repo).expect("changed_files");
            files.sort_by(|a, b| a.path.cmp(&b.path));

            assert_eq!(
                files,
                vec![
                    ChangedFile {
                        path: PathBuf::from("edit.rs"),
                        status: ChangeStatus::Modified
                    },
                    ChangedFile {
                        path: PathBuf::from("new.rs"),
                        status: ChangeStatus::Added
                    },
                    ChangedFile {
                        path: PathBuf::from("remove.rs"),
                        status: ChangeStatus::Deleted
                    },
                ]
            );
        }

        #[test]
        fn clean_working_tree_has_no_changes() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_files(dir.path(), &[("a.rs", "content\n")]);
            assert_eq!(changed_files(&repo).expect("changed_files"), vec![]);
        }

        #[test]
        fn ignored_files_are_not_reported_as_added() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_files(dir.path(), &[("a.rs", "content\n")]);
            fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
            fs::create_dir(dir.path().join("build")).unwrap();
            fs::write(dir.path().join("build/output.o"), "binary junk").unwrap();

            let files = changed_files(&repo).expect("changed_files");
            assert!(
                !files
                    .iter()
                    .any(|f| f.path.to_string_lossy().contains("build"))
            );
        }
    }
}
