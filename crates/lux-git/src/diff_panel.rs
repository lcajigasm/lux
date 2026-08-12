//! Changes tree: which files changed, grouped by folder.
//!
//! Scope: this module answers "which files changed" — it doesn't stage,
//! commit, or write anything, and it doesn't render a diff (see
//! `diff_view` for side-by-side/inline rendering). Reading the working
//! tree happens in-process via `gix` and the same `ignore`-crate
//! gitignore matcher `lux-search`/`lux-workspace` already use elsewhere
//! in this workspace (Phase 4); no shell-out to `git` (see
//! `docs/adr/0010-git-backend-gix.md`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

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

    let mut changed = Vec::new();

    // Every tracked path, checked directly on disk rather than through the
    // gitignore-aware walk below: a `.gitignore` rule never hides a path git
    // already tracks (the rule only ever governs *untracked*-file discovery),
    // so a tracked file that also happens to match one — e.g. a project's
    // own `.vscode/settings.json` committed before a later-added ignore rule
    // — must still be compared, not treated as silently deleted just because
    // the walk skipped it.
    for (rel, oid) in &head_blobs {
        let absolute = work_dir.join(rel);
        match std::fs::read(&absolute) {
            Ok(disk_bytes) => {
                let blob = repo
                    .find_blob(*oid)
                    .map_err(|e| DiffPanelError::ReadBlob(Box::new(e)))?;
                if blob.data != disk_bytes {
                    changed.push(ChangedFile {
                        path: rel.clone(),
                        status: ChangeStatus::Modified,
                    });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                changed.push(ChangedFile {
                    path: rel.clone(),
                    status: ChangeStatus::Deleted,
                });
            }
            Err(e) => return Err(DiffPanelError::Io(e)),
        }
    }

    // The gitignore-aware walk only needs to find *untracked* files now —
    // every tracked path was already handled above regardless of ignore
    // status.
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
        if !head_blobs.contains_key(&rel) {
            changed.push(ChangedFile {
                path: rel,
                status: ChangeStatus::Added,
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

        #[test]
        fn a_tracked_file_that_matches_a_later_gitignore_rule_is_not_reported_as_deleted() {
            // Real-world case found verifying against rust-lang/rust: files like
            // `.vscode/settings.json` are committed, then a later `.gitignore` rule
            // starts hiding that same path from ignore-aware directory walks — but
            // the file is still tracked and unchanged, so `git status` (and this
            // function) must not call it deleted just because a walk that respects
            // `.gitignore` no longer visits it.
            let dir = tempfile::tempdir().expect("tempdir");
            let repo =
                init_repo_with_committed_files(dir.path(), &[("settings.local.json", "{}\n")]);
            fs::write(dir.path().join(".gitignore"), "settings.local.json\n").unwrap();

            let files = changed_files(&repo).expect("changed_files");
            assert!(
                !files
                    .iter()
                    .any(|f| f.path == Path::new("settings.local.json")),
                "the tracked, unmodified, now-ignored file must not show up as changed: {files:?}"
            );
        }
    }
}
