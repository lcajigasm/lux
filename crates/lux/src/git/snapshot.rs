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
        && let Ok(rel) = path.strip_prefix(root)
        && let Ok(found) = hunks_against_head(&repo, rel, text)
    {
        hunks = found;
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
    let rel = file.strip_prefix(root).unwrap_or(file);
    lux_git::staging::stage_hunk(&repo, rel, hunk).map_err(|error| error.to_string())
}

fn head_blob(repo: &lux_git::gix::Repository, file: &Path) -> Option<String> {
    let tree_id = repo.head_tree_id_or_empty().ok()?;
    let tree = repo.find_tree(tree_id).ok()?;
    let entry = tree.lookup_entry_by_path(file).ok()??;
    let object = entry.object().ok()?;
    String::from_utf8(object.data.clone()).ok()
}
