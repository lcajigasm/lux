//! Per-path git state for the file tree: which letter a row shows and
//! which color its name takes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use lux_git::diff_panel::{ChangeStatus, ChangedFile};

use crate::app::App;

/// The state a tree row shows. Ordered weakest first: a directory takes
/// the strongest state of anything below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum GitMark {
    Ignored,
    Untracked,
    Added,
    Deleted,
    Modified,
    Conflicted,
}

impl GitMark {
    pub(crate) fn from_status(status: ChangeStatus) -> Self {
        match status {
            ChangeStatus::Added => GitMark::Added,
            ChangeStatus::Modified => GitMark::Modified,
            ChangeStatus::Deleted => GitMark::Deleted,
            ChangeStatus::Untracked => GitMark::Untracked,
            ChangeStatus::Conflicted => GitMark::Conflicted,
        }
    }

    /// The letter beside the name. Ignored rows show none; they are dimmed.
    pub(crate) fn letter(self) -> Option<char> {
        match self {
            GitMark::Ignored => None,
            GitMark::Untracked => Some('U'),
            GitMark::Added => Some('A'),
            GitMark::Deleted => Some('D'),
            GitMark::Modified => Some('M'),
            GitMark::Conflicted => Some('C'),
        }
    }
}

/// The tree's git marks: one per changed path, its parent directories and
/// each collapsed ignored path, plus the untracked directories `git status`
/// reports whole, whose contents inherit their mark.
#[derive(Debug, Default)]
pub(crate) struct TreeMarks {
    pub(crate) marks: HashMap<PathBuf, GitMark>,
    pub(crate) untracked_dirs: HashSet<PathBuf>,
}

impl TreeMarks {
    /// The mark for `path`: its own, or the mark of a collapsed ignored or
    /// untracked directory above it.
    pub(crate) fn mark_for(&self, path: &Path) -> Option<GitMark> {
        if let Some(mark) = self.marks.get(path) {
            return Some(*mark);
        }
        path.ancestors().skip(1).find_map(|dir| {
            if self.marks.get(dir) == Some(&GitMark::Ignored) {
                Some(GitMark::Ignored)
            } else if self.untracked_dirs.contains(dir) {
                Some(GitMark::Untracked)
            } else {
                None
            }
        })
    }
}

/// Builds [`TreeMarks`] for a repository at `root`. The repository root
/// itself is unmarked.
pub(crate) fn tree_marks(
    root: &Path,
    files: &[ChangedFile],
    untracked_dirs: &[PathBuf],
    ignored: &[PathBuf],
) -> TreeMarks {
    TreeMarks {
        marks: git_mark_map(root, files, ignored),
        untracked_dirs: untracked_dirs.iter().map(|dir| root.join(dir)).collect(),
    }
}

/// One mark per changed path plus the strongest mark of its contents on
/// every parent directory under `root`, and one `Ignored` mark per
/// collapsed ignored path. The repository root itself is unmarked.
fn git_mark_map(
    root: &Path,
    files: &[ChangedFile],
    ignored: &[PathBuf],
) -> HashMap<PathBuf, GitMark> {
    let mut marks = HashMap::new();
    for path in ignored {
        marks.insert(root.join(path), GitMark::Ignored);
    }
    for file in files {
        let mark = GitMark::from_status(file.status);
        let absolute = root.join(&file.path);
        marks.insert(absolute.clone(), mark);
        let mut parent = absolute.parent();
        while let Some(dir) = parent {
            if dir == root {
                break;
            }
            let slot = marks.entry(dir.to_path_buf()).or_insert(mark);
            if *slot < mark {
                *slot = mark;
            }
            parent = dir.parent();
        }
    }
    marks
}

/// Probe and test label: relative paths, sorted, `path:M`. Ignored paths
/// are listed as `path:!`, the letter `git status --short` uses.
pub(crate) fn git_marks_label(files: &[ChangedFile], ignored: &[PathBuf]) -> String {
    let mut parts = files
        .iter()
        .map(|file| {
            let letter = GitMark::from_status(file.status).letter().unwrap_or('?');
            format!("{}:{letter}", file.path.display())
        })
        .chain(ignored.iter().map(|path| format!("{}:!", path.display())))
        .collect::<Vec<_>>();
    parts.sort();
    parts.join(",")
}

pub(crate) fn publish_git_marks(app: &mut App) {
    let Some(root) = app.ui.git.repo_root.clone() else {
        return;
    };
    let marks = tree_marks(
        &root,
        &app.ui.git.changed_files,
        &app.ui.git.untracked_dirs,
        &app.ui.git.ignored,
    );
    if let Some(sidebar) = app.ui.sidebar.as_mut() {
        sidebar.git_marks = marks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(path: &str, status: ChangeStatus) -> ChangedFile {
        ChangedFile {
            path: path.into(),
            status,
        }
    }

    #[test]
    fn a_directory_takes_the_strongest_state_below_it() {
        let files = vec![
            changed("src/new.rs", ChangeStatus::Untracked),
            changed("src/clash.rs", ChangeStatus::Conflicted),
            changed("src/edit.rs", ChangeStatus::Modified),
        ];
        let marks = git_mark_map(Path::new("/repo"), &files, &[]);
        assert_eq!(
            marks.get(Path::new("/repo/src")),
            Some(&GitMark::Conflicted)
        );
        assert_eq!(
            marks.get(Path::new("/repo/src/new.rs")),
            Some(&GitMark::Untracked)
        );
    }

    #[test]
    fn rows_inside_an_ignored_directory_are_ignored() {
        let marks = tree_marks(Path::new("/repo"), &[], &[], &["target".into()]);
        assert_eq!(
            marks.mark_for(Path::new("/repo/target/debug/lux")),
            Some(GitMark::Ignored)
        );
        assert_eq!(marks.mark_for(Path::new("/repo/src/main.rs")), None);
        assert_eq!(GitMark::Ignored.letter(), None);
    }

    #[test]
    fn rows_inside_a_collapsed_untracked_directory_are_untracked() {
        // `src/` only aggregates its untracked child; it must not pass the
        // mark on to tracked siblings the way a collapsed directory does.
        let files = vec![changed("src/scratch", ChangeStatus::Untracked)];
        let marks = tree_marks(Path::new("/repo"), &files, &["src/scratch".into()], &[]);
        assert_eq!(
            marks.mark_for(Path::new("/repo/src/scratch/deep/a.rs")),
            Some(GitMark::Untracked)
        );
        assert_eq!(marks.mark_for(Path::new("/repo/src/lib.rs")), None);
        assert_eq!(
            marks.mark_for(Path::new("/repo/src")),
            Some(GitMark::Untracked)
        );
    }
}
