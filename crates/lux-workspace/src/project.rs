//! Project state: multiple workspace roots, and the `.lux-project` file that
//! declares them. This module is pure data plus path resolution — no file
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

/// A multi-root workspace: an ordered set of folders opened together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    roots: Vec<Root>,
    next_root_id: u64,
    excludes: Vec<String>,
    settings: Map<String, Value>,
}

impl Workspace {
    pub fn new() -> Self {
        Workspace {
            roots: Vec::new(),
            next_root_id: 0,
            excludes: Vec::new(),
            settings: Map::new(),
        }
    }

    /// Opens a single folder as a one-root workspace, with no excludes or
    /// settings — the common case (`lux some/dir`).
    pub fn open_folder(path: impl Into<PathBuf>) -> Self {
        let mut workspace = Self::new();
        workspace.add_root(path, None);
        workspace
    }

    /// Opens `dir` as a project: if it contains a `.lux-project` file, loads
    /// it (its folders become the workspace's roots, resolved relative to
    /// `dir`); otherwise falls back to a single-root workspace for `dir`
    /// itself. This is how "open a folder" and "open a multi-root project"
    /// become the same entry point.
    pub fn discover(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref();
        let project_path = dir.join(PROJECT_FILE_NAME);
        if project_path.is_file() {
            let project = ProjectFile::load(&project_path)?;
            Ok(Workspace::from_project_file(&project, dir))
        } else {
            Ok(Workspace::open_folder(dir))
        }
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

    /// Builds a workspace from an already-loaded `.lux-project` file.
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
        workspace
    }

    /// The inverse of `from_project_file`: dumps roots/excludes/settings
    /// back to the `.lux-project` schema. Root paths are stored as given
    /// (i.e. absolute, since that's what `Root::path` holds) — resolving
    /// them to relative paths would require knowing where the file will be
    /// saved, which is the caller's decision, not this method's.
    pub fn to_project_file(&self) -> ProjectFile {
        ProjectFile {
            folders: self
                .roots
                .iter()
                .map(|r| FolderEntry {
                    path: r.path.clone(),
                    name: Some(r.name.clone()),
                })
                .collect(),
            excludes: self.excludes.clone(),
            settings: self.settings.clone(),
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

/// The on-disk schema of a `.lux-project` file: plain JSON (see ADR 0015 for
/// why not JSONC here). `settings` is an untyped bag — the actual settings
/// schema belongs to `lux-config` (Phase 5), not to the project file format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub folders: Vec<FolderEntry>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub settings: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderEntry {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-workspace-project-test-{:?}-{}",
            std::thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn new_workspace_has_no_roots() {
        let workspace = Workspace::new();
        assert!(workspace.roots().is_empty());
        assert!(!workspace.is_multi_root());
    }

    #[test]
    fn open_folder_creates_a_single_root_with_a_derived_name() {
        let workspace = Workspace::open_folder("/home/user/my-project");
        assert_eq!(workspace.roots().len(), 1);
        assert_eq!(
            workspace.roots()[0].path,
            Path::new("/home/user/my-project")
        );
        assert_eq!(workspace.roots()[0].name, "my-project");
        assert!(!workspace.is_multi_root());
    }

    #[test]
    fn add_root_is_idempotent_for_the_same_path() {
        let mut workspace = Workspace::new();
        let first = workspace.add_root("/a", None);
        let second = workspace.add_root("/a", Some("renamed".to_owned()));
        assert_eq!(first, second);
        assert_eq!(workspace.roots().len(), 1);
        assert_eq!(workspace.roots()[0].name, "a");
    }

    #[test]
    fn add_root_grows_a_real_multi_root_workspace() {
        let mut workspace = Workspace::new();
        workspace.add_root("/a", None);
        workspace.add_root("/b", None);
        assert_eq!(workspace.roots().len(), 2);
        assert!(workspace.is_multi_root());
    }

    #[test]
    fn remove_root_drops_the_matching_root_and_reports_success() {
        let mut workspace = Workspace::new();
        let id = workspace.add_root("/a", None);
        workspace.add_root("/b", None);

        assert!(workspace.remove_root(id));
        assert_eq!(workspace.roots().len(), 1);
        assert_eq!(workspace.roots()[0].path, Path::new("/b"));
    }

    #[test]
    fn remove_root_returns_false_for_an_unknown_id() {
        let mut workspace = Workspace::new();
        let id = workspace.add_root("/a", None);
        workspace.remove_root(id);
        assert!(!workspace.remove_root(id));
    }

    #[test]
    fn root_ids_stay_stable_across_removal_of_other_roots() {
        let mut workspace = Workspace::new();
        let a = workspace.add_root("/a", None);
        let b = workspace.add_root("/b", None);
        workspace.remove_root(a);
        assert_eq!(workspace.root(b).unwrap().path, Path::new("/b"));
    }

    #[test]
    fn root_for_path_resolves_to_the_containing_root() {
        let mut workspace = Workspace::new();
        workspace.add_root("/projects/backend", None);
        workspace.add_root("/projects/frontend", None);

        let found = workspace
            .root_for_path("/projects/backend/src/main.rs")
            .unwrap();
        assert_eq!(found.path, Path::new("/projects/backend"));
    }

    #[test]
    fn root_for_path_returns_none_outside_every_root() {
        let mut workspace = Workspace::new();
        workspace.add_root("/projects/backend", None);
        assert!(workspace.root_for_path("/elsewhere/file.rs").is_none());
    }

    #[test]
    fn root_for_path_picks_the_most_specific_root_when_nested() {
        let mut workspace = Workspace::new();
        workspace.add_root("/projects", None);
        workspace.add_root("/projects/backend", None);

        let found = workspace
            .root_for_path("/projects/backend/src/main.rs")
            .unwrap();
        assert_eq!(found.path, Path::new("/projects/backend"));
    }

    #[test]
    fn project_file_round_trips_through_json() {
        let mut settings = Map::new();
        settings.insert("tabSize".to_owned(), Value::from(2));

        let project = ProjectFile {
            folders: vec![
                FolderEntry {
                    path: PathBuf::from("backend"),
                    name: Some("API".to_owned()),
                },
                FolderEntry {
                    path: PathBuf::from("../frontend"),
                    name: None,
                },
            ],
            excludes: vec!["target/**".to_owned(), "*.lock".to_owned()],
            settings,
        };

        let json = serde_json::to_string(&project).unwrap();
        let parsed: ProjectFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, project);
    }

    #[test]
    fn project_file_load_defaults_missing_excludes_and_settings() {
        let dir = temp_dir();
        let path = dir.join(PROJECT_FILE_NAME);
        fs::write(&path, r#"{"folders":[{"path":"."}]}"#).unwrap();

        let project = ProjectFile::load(&path).unwrap();
        assert_eq!(
            project.folders,
            vec![FolderEntry {
                path: PathBuf::from("."),
                name: None,
            }]
        );
        assert!(project.excludes.is_empty());
        assert!(project.settings.is_empty());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn project_file_load_surfaces_missing_file_as_io_error() {
        let err = ProjectFile::load("/nonexistent/.lux-project").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn project_file_load_surfaces_invalid_json_as_invalid_data() {
        let dir = temp_dir();
        let path = dir.join(PROJECT_FILE_NAME);
        fs::write(&path, "not json").unwrap();

        let err = ProjectFile::load(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn project_file_save_and_load_round_trip_through_a_real_file() {
        let dir = temp_dir();
        let path = dir.join(PROJECT_FILE_NAME);

        let project = ProjectFile {
            folders: vec![FolderEntry {
                path: PathBuf::from("src"),
                name: Some("Source".to_owned()),
            }],
            excludes: vec!["node_modules/**".to_owned()],
            settings: Map::new(),
        };
        project.save(&path).unwrap();

        let loaded = ProjectFile::load(&path).unwrap();
        assert_eq!(loaded, project);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn workspace_from_project_file_resolves_relative_folders_against_base_dir() {
        let project = ProjectFile {
            folders: vec![FolderEntry {
                path: PathBuf::from("backend"),
                name: None,
            }],
            excludes: vec![],
            settings: Map::new(),
        };
        let workspace = Workspace::from_project_file(&project, "/projects/acme");
        assert_eq!(
            workspace.roots()[0].path,
            Path::new("/projects/acme/backend")
        );
    }

    #[test]
    fn workspace_from_project_file_keeps_absolute_folders_untouched() {
        let project = ProjectFile {
            folders: vec![FolderEntry {
                path: PathBuf::from("/elsewhere/lib"),
                name: None,
            }],
            excludes: vec![],
            settings: Map::new(),
        };
        let workspace = Workspace::from_project_file(&project, "/projects/acme");
        assert_eq!(workspace.roots()[0].path, Path::new("/elsewhere/lib"));
    }

    #[test]
    fn workspace_carries_excludes_and_settings_from_the_project_file() {
        let mut settings = Map::new();
        settings.insert("formatOnSave".to_owned(), Value::Bool(true));
        let project = ProjectFile {
            folders: vec![],
            excludes: vec!["dist/**".to_owned()],
            settings: settings.clone(),
        };
        let workspace = Workspace::from_project_file(&project, "/projects/acme");
        assert_eq!(workspace.excludes(), &["dist/**".to_owned()]);
        assert_eq!(workspace.settings(), &settings);
    }

    #[test]
    fn workspace_to_project_file_round_trips_roots_excludes_and_settings() {
        let mut workspace = Workspace::new();
        workspace.add_root("/projects/acme/backend", Some("API".to_owned()));
        workspace.set_excludes(vec!["target/**".to_owned()]);
        let mut settings = Map::new();
        settings.insert("tabSize".to_owned(), Value::from(4));
        workspace.set_settings(settings.clone());

        let project = workspace.to_project_file();
        assert_eq!(
            project.folders,
            vec![FolderEntry {
                path: PathBuf::from("/projects/acme/backend"),
                name: Some("API".to_owned()),
            }]
        );
        assert_eq!(project.excludes, vec!["target/**".to_owned()]);
        assert_eq!(project.settings, settings);

        let rebuilt = Workspace::from_project_file(&project, "/projects/acme");
        assert_eq!(rebuilt, workspace);
    }

    #[test]
    fn discover_loads_the_project_file_when_present() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join("backend")).unwrap();
        fs::write(
            dir.join(PROJECT_FILE_NAME),
            r#"{"folders":[{"path":"backend","name":"API"}],"excludes":["target/**"]}"#,
        )
        .unwrap();

        let workspace = Workspace::discover(&dir).unwrap();
        assert_eq!(workspace.roots().len(), 1);
        assert_eq!(workspace.roots()[0].path, dir.join("backend"));
        assert_eq!(workspace.roots()[0].name, "API");
        assert_eq!(workspace.excludes(), &["target/**".to_owned()]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discover_falls_back_to_a_single_root_when_no_project_file_exists() {
        let dir = temp_dir();
        fs::remove_file(dir.join(PROJECT_FILE_NAME)).ok();

        let workspace = Workspace::discover(&dir).unwrap();
        assert_eq!(workspace.roots().len(), 1);
        assert_eq!(workspace.roots()[0].path, dir);
        assert!(workspace.excludes().is_empty());

        fs::remove_dir_all(&dir).ok();
    }
}
