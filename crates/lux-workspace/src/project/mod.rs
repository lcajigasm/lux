//! Project state: multiple workspace roots, and the `.lux-project` / `.sublime-project` files that
//! declare them. This module is pure data plus path resolution — no file
//! watching, no indexing, no search. Those are separate Phase 4 deliverables
//! that key their own per-root state off the [`RootId`]s defined here.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The dot-file `Workspace::discover` looks for at the root of a project
/// folder, analogous to `.editorconfig` or `.gitignore`.
pub const PROJECT_FILE_NAME: &str = ".lux-project";

/// The file extension used by Sublime Text project files.
pub const SUBLIME_PROJECT_EXT: &str = "sublime-project";

/// Identifies a [`Root`] independently of its position in
/// [`Workspace::roots`], so removing one root never changes the identity of
/// another. Other subsystems (search, commands, per-root state) hold this id
/// to scope their work to one root without the others leaking in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub id: RootId,
    pub path: PathBuf,
    pub name: String,
}

/// A custom build system command definition from a project file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectBuildSystem {
    pub name: String,
    #[serde(default)]
    pub cmd: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
}

/// A multi-root workspace: an ordered set of folders opened together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    roots: Vec<Root>,
    next_root_id: u64,
    excludes: Vec<String>,
    settings: Map<String, Value>,
    project_path: Option<PathBuf>,
    build_systems: Vec<ProjectBuildSystem>,
}

impl Workspace {
    pub fn new() -> Self {
        Workspace {
            roots: Vec::new(),
            next_root_id: 0,
            excludes: Vec::new(),
            settings: Map::new(),
            project_path: None,
            build_systems: Vec::new(),
        }
    }

    /// Opens a single folder as a one-root workspace, with no excludes or
    /// settings — the common case (`lux some/dir`).
    pub fn open_folder(path: impl Into<PathBuf>) -> Self {
        let mut workspace = Self::new();
        workspace.add_root(path, None);
        workspace
    }

    /// Opens `dir` as a project: if it contains a `.lux-project` or `*.sublime-project` file,
    /// loads it; otherwise falls back to a single-root workspace for `dir` itself.
    pub fn discover(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref();
        let project_path = dir.join(PROJECT_FILE_NAME);
        if project_path.is_file() {
            Self::open_project_file(&project_path)
        } else if let Some(sublime_project) = find_sublime_project(dir) {
            Self::open_project_file(&sublime_project)
        } else {
            Ok(Workspace::open_folder(dir))
        }
    }

    /// Loads a project directly from a `.lux-project` or `.sublime-project` file.
    pub fn open_project_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let project = ProjectFile::load(path)?;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let mut ws = Workspace::from_project_file(&project, base_dir);
        ws.project_path = Some(path.to_path_buf());
        Ok(ws)
    }

    /// Adds `path` as a root, deriving its display name from the last path
    /// component unless `name` overrides it. Adding a path that's already a
    /// root is a no-op that returns the existing root's id, so callers don't
    /// need to check for duplicates themselves.
    pub fn add_root(&mut self, path: impl Into<PathBuf>, name: Option<String>) -> RootId {
        let path = path.into();
        if let Some(existing) = self.roots.iter().find(|r| r.path == path) {
            return existing.id;
        }
        let id = RootId(self.next_root_id);
        self.next_root_id += 1;
        let name = name.unwrap_or_else(|| default_root_name(&path));
        self.roots.push(Root { id, path, name });
        id
    }

    /// Removes the root with the given id. Returns whether a root was
    /// actually removed.
    pub fn remove_root(&mut self, id: RootId) -> bool {
        let len_before = self.roots.len();
        self.roots.retain(|r| r.id != id);
        self.roots.len() != len_before
    }

    pub fn roots(&self) -> &[Root] {
        &self.roots
    }

    pub fn root(&self, id: RootId) -> Option<&Root> {
        self.roots.iter().find(|r| r.id == id)
    }

    /// Which root, if any, contains `path` — the most specific (deepest)
    /// root when roots are nested. This is how search/commands/per-root
    /// state resolve "which root does this file belong to" without each
    /// reimplementing the path logic.
    pub fn root_for_path(&self, path: impl AsRef<Path>) -> Option<&Root> {
        let path = path.as_ref();
        self.roots
            .iter()
            .filter(|r| path.starts_with(&r.path))
            .max_by_key(|r| r.path.components().count())
    }

    pub fn is_multi_root(&self) -> bool {
        self.roots.len() > 1
    }

    pub fn excludes(&self) -> &[String] {
        &self.excludes
    }

    pub fn set_excludes(&mut self, excludes: Vec<String>) {
        self.excludes = excludes;
    }

    pub fn settings(&self) -> &Map<String, Value> {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: Map<String, Value>) {
        self.settings = settings;
    }

    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    pub fn set_project_path(&mut self, path: Option<PathBuf>) {
        self.project_path = path;
    }

    pub fn build_systems(&self) -> &[ProjectBuildSystem] {
        &self.build_systems
    }

    pub fn set_build_systems(&mut self, build_systems: Vec<ProjectBuildSystem>) {
        self.build_systems = build_systems;
    }

    /// Builds a workspace from an already-loaded project file.
    /// Relative folder paths are resolved against `base_dir` (the directory
    /// the project file lives in); absolute paths pass through unchanged.
    pub fn from_project_file(project: &ProjectFile, base_dir: impl AsRef<Path>) -> Self {
        let base_dir = base_dir.as_ref();
        let mut workspace = Self::new();
        for folder in &project.folders {
            let path = if folder.path.is_absolute() {
                folder.path.clone()
            } else {
                base_dir.join(&folder.path)
            };
            workspace.add_root(path, folder.name.clone());
        }
        workspace.excludes = project.excludes.clone();
        workspace.settings = project.settings.clone();
        workspace.build_systems = project.build_systems.clone();
        workspace
    }

    /// The inverse of `from_project_file`: dumps roots/excludes/settings/build_systems
    /// back to the project file schema. Root paths are stored as given.
    pub fn to_project_file(&self) -> ProjectFile {
        ProjectFile {
            folders: self
                .roots
                .iter()
                .map(|r| FolderEntry {
                    path: r.path.clone(),
                    name: Some(r.name.clone()),
                    folder_exclude_patterns: Vec::new(),
                    file_exclude_patterns: Vec::new(),
                })
                .collect(),
            excludes: self.excludes.clone(),
            settings: self.settings.clone(),
            build_systems: self.build_systems.clone(),
        }
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

fn default_root_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Scans `dir` for any file ending with `.sublime-project`.
pub fn find_sublime_project(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == SUBLIME_PROJECT_EXT)
        {
            return Some(path);
        }
    }
    None
}

/// The on-disk schema of a `.lux-project` or `.sublime-project` file: plain JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub folders: Vec<FolderEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excludes: Vec<String>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub settings: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_systems: Vec<ProjectBuildSystem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderEntry {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folder_exclude_patterns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_exclude_patterns: Vec<String>,
}

impl ProjectFile {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, text)
    }
}

#[cfg(test)]
mod tests;
