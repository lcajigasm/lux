//! Changes tree: which files changed, grouped by folder.
//!
//! Scope: this module answers "which files changed" — it doesn't stage,
//! commit, or write anything, and it doesn't render a diff (see
//! `diff_view` for side-by-side/inline rendering). Reading the working
//! tree happens in-process via `gix`, with `gix`'s own exclude stack for
//! `.gitignore` (see `worktree`); no shell-out to `git` (see
//! `docs/adr/0010-git-backend-gix.md`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

/// Where a changed path stands relative to `HEAD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    /// Not in `HEAD`, staged in the index.
    Added,
    Modified,
    Deleted,
    /// Not in `HEAD`, not in the index, not ignored.
    Untracked,
    /// The index holds an unresolved merge conflict for this path.
    Conflicted,
}

/// Everything `git status --ignored` would show: the changes plus the
/// ignored paths, collapsed to the outermost ignored directory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorktreeStatus {
    pub changes: Vec<ChangedFile>,
    /// Untracked directories reported whole, like `git status` shows
    /// `?? dir/`. Each also appears in `changes` as one `Untracked` entry;
    /// everything below one is untracked or ignored.
    pub untracked_dirs: Vec<PathBuf>,
    pub ignored: Vec<PathBuf>,
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
    Index(Box<dyn std::error::Error + Send + Sync>),
    Io(std::io::Error),
}

impl fmt::Display for DiffPanelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffPanelError::BareRepository => write!(f, "repository has no working directory"),
            DiffPanelError::HeadTree(e) => write!(f, "failed to resolve HEAD tree: {e}"),
            DiffPanelError::ReadTree(e) => write!(f, "failed to read HEAD tree: {e}"),
            DiffPanelError::ReadBlob(e) => write!(f, "failed to read blob: {e}"),
            DiffPanelError::Index(e) => write!(f, "failed to read the index: {e}"),
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
    worktree_status(repo).map(|status| status.changes)
}

/// [`changed_files`] plus the ignored paths, from one walk.
pub fn worktree_status(repo: &gix::Repository) -> Result<WorktreeStatus, DiffPanelError> {
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

    // Every tracked path, checked directly on disk rather than through the
    // gitignore-aware walk below: a `.gitignore` rule never hides a path git
    // already tracks (the rule only ever governs *untracked*-file discovery),
    // so a tracked file that also happens to match one — e.g. a project's
    // own `.vscode/settings.json` committed before a later-added ignore rule
    // — must still be compared, not treated as silently deleted just because
    // the walk skipped it. The index's stat cache answers for every file
    // that has not been touched since it was staged (see `tracked`).
    let (index, facts) = crate::worktree::index_facts(repo)?;
    let mut changed = Vec::new();
    crate::tracked::compare(repo, work_dir, &head_blobs, &index, &mut changed)?;

    // Everything `HEAD` does not track: staged new files, untracked files,
    // and ignored paths. A path git tracks in `HEAD` or the index is never
    // ignored, and neither is a directory holding one.
    let tracked: std::collections::BTreeSet<PathBuf> = head_blobs.keys().cloned().collect();
    let keep = crate::worktree::with_parents(tracked.iter().chain(facts.staged.iter()));
    let untracked = crate::worktree::walk(repo, &index, &tracked, &keep)?;
    for rel in untracked.files {
        let status = if facts.staged.contains(&rel) {
            ChangeStatus::Added
        } else {
            ChangeStatus::Untracked
        };
        changed.push(ChangedFile { path: rel, status });
    }
    for rel in &untracked.dirs {
        changed.push(ChangedFile {
            path: rel.clone(),
            status: ChangeStatus::Untracked,
        });
    }

    // A conflicted path wins over whatever the content comparison said:
    // the file holds markers, and the user must resolve it.
    for path in &facts.conflicted {
        match changed.iter_mut().find(|file| &file.path == path) {
            Some(file) => file.status = ChangeStatus::Conflicted,
            None => changed.push(ChangedFile {
                path: path.clone(),
                status: ChangeStatus::Conflicted,
            }),
        }
    }

    changed.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(WorktreeStatus {
        changes: changed,
        untracked_dirs: untracked.dirs,
        ignored: untracked.ignored,
    })
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
                        status: ChangeStatus::Untracked
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
        fn a_tracked_file_replaced_by_a_directory_stays_a_deletion() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_files(dir.path(), &[("a.rs", "one\n")]);
            fs::remove_file(dir.path().join("a.rs")).unwrap();
            fs::create_dir(dir.path().join("a.rs")).unwrap();
            fs::write(dir.path().join("a.rs").join("inside.txt"), "two\n").unwrap();

            let files = changed_files(&repo).expect("changed_files");
            assert!(
                files.iter().any(|file| {
                    file.path == Path::new("a.rs") && file.status == ChangeStatus::Deleted
                }),
                "{files:?}"
            );
            assert!(
                files
                    .iter()
                    .any(|file| file.path == Path::new("a.rs/inside.txt")
                        && file.status == ChangeStatus::Untracked),
                "{files:?}"
            );
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

        #[test]
        fn a_staged_new_file_is_added_and_an_unstaged_one_untracked() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_files(dir.path(), &[("a.rs", "content\n")]);
            fs::write(dir.path().join("staged.rs"), "staged\n").unwrap();
            fs::write(dir.path().join("loose.rs"), "loose\n").unwrap();
            crate::staging::stage(&repo, Path::new("staged.rs")).unwrap();

            let files = changed_files(&repo).expect("changed_files");
            let status_of = |name: &str| {
                files
                    .iter()
                    .find(|file| file.path == Path::new(name))
                    .map(|file| file.status)
            };
            assert_eq!(
                status_of("staged.rs"),
                Some(ChangeStatus::Added),
                "{files:?}"
            );
            assert_eq!(
                status_of("loose.rs"),
                Some(ChangeStatus::Untracked),
                "{files:?}"
            );
            assert_eq!(status_of("a.rs"), None, "{files:?}");
        }

        #[test]
        fn an_unmerged_index_entry_is_conflicted() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_files(dir.path(), &[("a.rs", "base\n")]);
            fs::write(
                dir.path().join("a.rs"),
                "<<<<<<< ours\nmine\n=======\ntheirs\n>>>>>>> theirs\n",
            )
            .unwrap();
            let mut index = crate::staging::open_or_create_index(&repo).unwrap();
            for stage in [
                gix::index::entry::Stage::Base,
                gix::index::entry::Stage::Ours,
                gix::index::entry::Stage::Theirs,
            ] {
                let blob = repo.write_blob(format!("{stage:?}\n").as_bytes()).unwrap();
                index.dangerously_push_entry(
                    gix::index::entry::Stat::default(),
                    blob.detach(),
                    gix::index::entry::Flags::from_stage(stage),
                    gix::index::entry::Mode::FILE,
                    "a.rs".into(),
                );
            }
            index.sort_entries();
            index.write(gix::index::write::Options::default()).unwrap();

            let files = changed_files(&repo).expect("changed_files");
            assert_eq!(
                files,
                vec![ChangedFile {
                    path: PathBuf::from("a.rs"),
                    status: ChangeStatus::Conflicted
                }]
            );
        }

        #[test]
        fn ignored_paths_collapse_to_the_ignored_directory() {
            let dir = tempfile::tempdir().expect("tempdir");
            let repo = init_repo_with_committed_files(
                dir.path(),
                &[(".gitignore", "build/\n*.log\n"), ("kept.log", "tracked\n")],
            );
            fs::create_dir_all(dir.path().join("build/deep")).unwrap();
            fs::write(dir.path().join("build/deep/out.o"), "junk").unwrap();
            fs::write(dir.path().join("debug.log"), "noise").unwrap();

            let status = worktree_status(&repo).expect("worktree_status");
            assert_eq!(
                status.ignored,
                vec![PathBuf::from("build"), PathBuf::from("debug.log")]
            );
            assert!(status.changes.is_empty(), "{:?}", status.changes);
        }
    }
}
