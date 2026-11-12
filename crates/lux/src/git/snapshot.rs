//! Repository reads used by the git worker and by the feature tests.
//! Callers that run on the UI thread only schedule these.

use std::path::{Path, PathBuf};

use lux_git::diff_panel::{ChangedFile, changed_files};
use lux_git::diff_view::{InlineDiffLineKind, inline_diff};
use lux_git::gutter::{GutterMarker, Hunk, gutter_markers, hunks_against_head};

use super::{discover_repo, head_label};

#[derive(Debug, Clone)]
pub(crate) struct FileShot {
    pub(crate) repo_root: Option<PathBuf>,
    pub(crate) branch_name: Option<String>,
    pub(crate) hunks: Vec<Hunk>,
    pub(crate) markers: Vec<GutterMarker>,
}

#[derive(Debug, Clone)]
pub(crate) struct StatusShot {
    pub(crate) root: PathBuf,
    pub(crate) branch: Option<String>,
    pub(crate) files: Vec<ChangedFile>,
}

pub(crate) fn file_shot(path: Option<&Path>, text: &str) -> FileShot {
    let Some(path) = path else {
        return FileShot {
            repo_root: None,
            branch_name: None,
            hunks: Vec::new(),
            markers: Vec::new(),
        };
    };
    let Some(repo) = discover_repo(path) else {
        return FileShot {
            repo_root: None,
            branch_name: None,
            hunks: Vec::new(),
            markers: Vec::new(),
        };
    };
    let repo_root = repo.workdir().map(Path::to_path_buf);
    let mut hunks = Vec::new();
    if path.is_file()
        && let Some(root) = &repo_root
    {
        let rel = path_in_repo(root, path);
        if let Ok(found) = hunks_against_head(&repo, &rel, text) {
            hunks = found;
        }
    }
    let markers = gutter_markers(&hunks);
    FileShot {
        repo_root,
        branch_name: head_label(&repo),
        hunks,
        markers,
    }
}

pub(crate) fn status_of(start: &Path) -> Result<StatusShot, String> {
    let repo = discover_repo(start).ok_or_else(|| "not a git repository".to_string())?;
    let root = repo
        .workdir()
        .map(Path::to_path_buf)
        .ok_or_else(|| "repository has no working directory".to_string())?;
    let branch = match lux_git::status::status(&repo) {
        Ok(st) => match st.branch {
            lux_git::status::BranchState::Named(name) => Some(name),
            lux_git::status::BranchState::Unborn(name) => Some(format!("{name} (unborn)")),
            lux_git::status::BranchState::Detached(id) => {
                Some(id.to_string().chars().take(7).collect())
            }
        },
        Err(_) => head_label(&repo),
    };
    let files = changed_files(&repo).map_err(|error| error.to_string())?;
    Ok(StatusShot {
        root,
        branch,
        files,
    })
}

pub(crate) fn diff_text(root: &Path, file: &Path) -> Result<String, String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let head_text = head_blob(&repo, file).unwrap_or_default();
    let working_text = std::fs::read_to_string(root.join(file)).unwrap_or_default();
    let mut content = format!("--- a/{}\n+++ b/{}\n", file.display(), file.display());
    for line in inline_diff(&head_text, &working_text) {
        match line.kind {
            InlineDiffLineKind::Context => content.push_str(&format!(" {}\n", line.text)),
            InlineDiffLineKind::Added => content.push_str(&format!("+{}\n", line.text)),
            InlineDiffLineKind::Removed => content.push_str(&format!("-{}\n", line.text)),
        }
    }
    Ok(content)
}

pub(crate) fn blame_text(root: &Path, file: &Path) -> Result<String, String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let commit = repo.head_id().map_err(|error| error.to_string())?;
    let hunks = lux_git::blame::blame_by_commit(&repo, file, commit.detach())
        .map_err(|error| error.to_string())?;
    let mut text = String::new();
    for hunk in hunks {
        let id = hunk.commit_id.to_string();
        let short = &id[..id.len().min(7)];
        text.push_str(&format!("{short} {}+{}\n", hunk.start_line, hunk.len));
    }
    Ok(text)
}

pub(crate) fn history_text(root: &Path, file: &Path) -> Result<String, String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let commit = repo.head_id().map_err(|error| error.to_string())?;
    let infos = lux_git::history::file_history(&repo, commit.detach(), file)
        .map_err(|error| error.to_string())?;
    let mut text = String::new();
    for info in infos {
        let id = info.id.to_string();
        let short = &id[..id.len().min(7)];
        text.push_str(&format!("{short} {}\n", info.summary));
    }
    Ok(text)
}

pub(crate) fn commit_index(root: &Path, message: &str) -> Result<(), String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    lux_git::commit::commit(&repo, message, "Lux User", "lux@example.com")
        .map(|_| ())
        .map_err(|error| error.to_string())
}

const GIT_NAME: &str = "Lux User";
const GIT_EMAIL: &str = "lux@example.com";

pub(crate) fn branch_names(root: &Path) -> Result<Vec<String>, String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let mut names = lux_git::branches::list(&repo)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|branch| branch.name)
        .collect::<Vec<_>>();
    names.sort();
    Ok(names)
}

/// Checks out `name` only when the worktree matches `HEAD`. A dirty tree
/// is left untouched: `branches::switch` rewrites every file.
pub(crate) fn switch_branch(root: &Path, name: &str) -> Result<String, String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let status = lux_git::status::status(&repo).map_err(|error| error.to_string())?;
    if !status.is_clean() {
        return Err("Commit or discard changes before switching branches".into());
    }
    lux_git::branches::switch(&repo, name, GIT_NAME, GIT_EMAIL)
        .map(|branch| branch.name)
        .map_err(|error| error.to_string())
}

/// Creates `name` at the current commit and moves `HEAD` onto it.
/// The worktree is not rewritten, so an unsaved edit stays on disk.
pub(crate) fn create_branch(root: &Path, name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Branch name is empty".into());
    }
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let commit = repo.head_id().map_err(|error| error.to_string())?.detach();
    lux_git::branches::create(&repo, name, commit, GIT_NAME, GIT_EMAIL)
        .map_err(|error| error.to_string())?;
    lux_git::branches::attach(&repo, name, GIT_NAME, GIT_EMAIL)
        .map(|branch| branch.name)
        .map_err(|error| error.to_string())
}

pub(crate) fn stage_hunk_on_disk(root: &Path, file: &Path, hunk: &Hunk) -> Result<(), String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let rel = path_in_repo(root, file);
    lux_git::staging::stage_hunk(&repo, &rel, hunk).map_err(|error| error.to_string())
}

/// The file's path relative to `root`. `gix` stores the workdir with
/// symlinks resolved, and the opened path may still be the link
/// (`/var` versus `/private/var` on macOS). A failed strip used to hand
/// the absolute path to the index, which then found no hunk.
fn path_in_repo(root: &Path, file: &Path) -> PathBuf {
    if let Ok(rel) = file.strip_prefix(root) {
        return rel.to_path_buf();
    }
    let Ok(root) = std::fs::canonicalize(root) else {
        return file.to_path_buf();
    };
    let Ok(file) = std::fs::canonicalize(file) else {
        return file.to_path_buf();
    };
    file.strip_prefix(root).unwrap_or(&file).to_path_buf()
}

fn head_blob(repo: &lux_git::gix::Repository, file: &Path) -> Option<String> {
    let tree_id = repo.head_tree_id_or_empty().ok()?;
    let tree = repo.find_tree(tree_id).ok()?;
    let entry = tree.lookup_entry_by_path(file).ok()??;
    let object = entry.object().ok()?;
    String::from_utf8(object.data.clone()).ok()
}

/// Unstages the staged hunk whose new side covers 1-based `line`.
/// The working tree file is not rewritten.
pub(crate) fn unstage_hunk_at_line(root: &Path, file: &Path, line: u32) -> Result<(), String> {
    let repo = discover_repo(root).ok_or_else(|| "not a git repository".to_string())?;
    let rel = path_in_repo(root, file);
    let hunks = lux_git::staging::staged_hunks(&repo, &rel).map_err(|error| error.to_string())?;
    let Some(hunk) = hunks.into_iter().find(|hunk| {
        let start = hunk.after_start;
        let end = start + hunk.after_len.max(1);
        line >= start && line < end
    }) else {
        return Err("No staged hunk on this line".into());
    };
    lux_git::staging::unstage_hunk(&repo, &rel, &hunk).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::path_in_repo;
    use std::path::Path;

    #[test]
    fn path_in_repo_resolves_a_symlink_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let note = real.join("note.rs");
        std::fs::write(&note, "one\n").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(
            path_in_repo(&real, &link.join("note.rs")),
            Path::new("note.rs")
        );
    }

    fn git(dir: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .args(args)
            .status()
            .expect("git");
        assert!(status.success(), "{args:?} -> {status}");
    }

    #[test]
    fn unstage_hunk_at_line_uses_a_system_git_index() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "-b", "main"]);
        git(repo, &["config", "user.email", "lux@example.com"]);
        git(repo, &["config", "user.name", "Lux User"]);
        let note = repo.join("note.rs");
        std::fs::write(&note, "one\ntwo\nthree\n").unwrap();
        git(repo, &["add", "note.rs"]);
        git(repo, &["commit", "-m", "base"]);
        std::fs::write(&note, "ONE\ntwo\nTHREE\n").unwrap();
        git(repo, &["add", "note.rs"]);

        super::unstage_hunk_at_line(repo, &note, 1).expect("unstage line 1");

        let cached = std::process::Command::new("git")
            .current_dir(repo)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .args(["diff", "--cached", "--", "note.rs"])
            .output()
            .expect("git diff");
        let text = String::from_utf8_lossy(&cached.stdout);
        assert!(
            cached.status.success() && !text.contains("+ONE") && text.contains("+THREE"),
            "system git index is not the unstaged hunk:\n{text}"
        );
        let worktree = std::process::Command::new("git")
            .current_dir(repo)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .args(["diff", "--", "note.rs"])
            .output()
            .expect("git diff worktree");
        let worktree_text = String::from_utf8_lossy(&worktree.stdout);
        assert!(
            worktree.status.success()
                && worktree_text.contains("+ONE")
                && !worktree_text.contains("+THREE"),
            "worktree diff ignored the file:\n{worktree_text}"
        );
        let index_bytes = std::fs::read(repo.join(".git/index")).unwrap();
        assert!(
            !index_bytes.windows(4).any(|window| window == b"TREE"),
            "index still has a tree cache"
        );

        let opened = lux_git::gix::open(repo).unwrap();
        let index = opened.open_index().unwrap();
        let entry = index.entry_by_path("note.rs".into()).unwrap();
        let blob = opened.find_blob(entry.id).unwrap();
        assert_eq!(&blob.data[..], b"one\ntwo\nTHREE\n");
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "ONE\ntwo\nTHREE\n");
    }
}
