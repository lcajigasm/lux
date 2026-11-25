//! Per-path git state for the file tree: which letter a row shows and
//! which color its name takes.

use std::collections::HashMap;
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

/// One mark per changed path plus the strongest mark of its contents on
/// every parent directory under `root`, and one `Ignored` mark per
/// collapsed ignored path. The repository root itself is unmarked.
pub(crate) fn git_mark_map(
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

/// The mark for `path`: its own, or `Ignored` when an ancestor is ignored.
pub(crate) fn mark_for(marks: &HashMap<PathBuf, GitMark>, path: &Path) -> Option<GitMark> {
    if let Some(mark) = marks.get(path) {
        return Some(*mark);
    }
    path.ancestors()
        .skip(1)
        .any(|dir| marks.get(dir) == Some(&GitMark::Ignored))
        .then_some(GitMark::Ignored)
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
    let marks = git_mark_map(&root, &app.ui.git.changed_files, &app.ui.git.ignored);
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
        let marks = git_mark_map(Path::new("/repo"), &[], &["target".into()]);
        assert_eq!(
            mark_for(&marks, Path::new("/repo/target/debug/lux")),
            Some(GitMark::Ignored)
        );
        assert_eq!(mark_for(&marks, Path::new("/repo/src/main.rs")), None);
        assert_eq!(GitMark::Ignored.letter(), None);
    }
}
