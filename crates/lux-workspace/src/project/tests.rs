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
    workspace.add_root("/home/user/repo", None);

    let root = workspace
        .root_for_path("/home/user/repo/src/main.rs")
        .unwrap();
    assert_eq!(root.path, Path::new("/home/user/repo"));
}

#[test]
fn root_for_path_picks_the_most_specific_root_when_nested() {
    let mut workspace = Workspace::new();
    workspace.add_root("/repo", None);
    workspace.add_root("/repo/nested", None);

    let root = workspace.root_for_path("/repo/nested/file.txt").unwrap();
    assert_eq!(root.path, Path::new("/repo/nested"));
}

#[test]
fn root_for_path_returns_none_outside_every_root() {
    let mut workspace = Workspace::new();
    workspace.add_root("/repo", None);
    assert!(workspace.root_for_path("/other/file.txt").is_none());
}

#[test]
fn project_file_round_trips_through_json() {
    let project = ProjectFile {
        folders: vec![
            FolderEntry {
                path: PathBuf::from("src"),
                name: Some("Source Code".to_owned()),
                folder_exclude_patterns: vec!["target".to_owned()],
                file_exclude_patterns: vec!["*.tmp".to_owned()],
            },
            FolderEntry {
                path: PathBuf::from("/absolute/path"),
                name: None,
                folder_exclude_patterns: vec![],
                file_exclude_patterns: vec![],
            },
        ],
        excludes: vec!["target/**".to_owned()],
        settings: Map::new(),
        build_systems: vec![ProjectBuildSystem {
            name: "Cargo Check".to_owned(),
            cmd: vec!["cargo".to_owned(), "check".to_owned()],
            file_regex: None,
            selector: None,
        }],
    };

    let json = serde_json::to_string(&project).unwrap();
    let back: ProjectFile = serde_json::from_str(&json).unwrap();
    assert_eq!(project, back);
}

#[test]
fn project_file_save_and_load_round_trip_through_a_real_file() {
    let dir = temp_dir();
    let file = dir.join(PROJECT_FILE_NAME);

    let project = ProjectFile {
        folders: vec![FolderEntry {
            path: PathBuf::from("nested"),
            name: Some("Nested".to_owned()),
            folder_exclude_patterns: vec![],
            file_exclude_patterns: vec![],
        }],
        excludes: vec!["node_modules/**".to_owned()],
        settings: Map::new(),
        build_systems: vec![],
    };

    project.save(&file).unwrap();
    let loaded = ProjectFile::load(&file).unwrap();
    assert_eq!(project, loaded);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn project_file_load_surfaces_missing_file_as_io_error() {
    let dir = temp_dir();
    let missing = dir.join("does-not-exist");
    assert!(ProjectFile::load(&missing).is_err());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn project_file_load_surfaces_invalid_json_as_invalid_data() {
    let dir = temp_dir();
    let file = dir.join(PROJECT_FILE_NAME);
    fs::write(&file, "not json at all").unwrap();

    let err = ProjectFile::load(&file).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn project_file_load_defaults_missing_excludes_and_settings() {
    let dir = temp_dir();
    let file = dir.join(PROJECT_FILE_NAME);
    fs::write(&file, r#"{"folders":[{"path":"src"}]}"#).unwrap();

    let project = ProjectFile::load(&file).unwrap();
    assert_eq!(project.folders.len(), 1);
    assert_eq!(project.folders[0].path, Path::new("src"));
    assert_eq!(project.folders[0].name, None);
    assert!(project.excludes.is_empty());
    assert!(project.settings.is_empty());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn workspace_from_project_file_resolves_relative_folders_against_base_dir() {
    let project = ProjectFile {
        folders: vec![FolderEntry {
            path: PathBuf::from("crates/lux"),
            name: None,
            folder_exclude_patterns: vec![],
            file_exclude_patterns: vec![],
        }],
        excludes: vec![],
        settings: Map::new(),
        build_systems: vec![],
    };

    let workspace = Workspace::from_project_file(&project, "/home/user/lux");
    assert_eq!(workspace.roots().len(), 1);
    assert_eq!(
        workspace.roots()[0].path,
        Path::new("/home/user/lux/crates/lux")
    );
    assert_eq!(workspace.roots()[0].name, "lux");
}

#[test]
fn workspace_from_project_file_keeps_absolute_folders_untouched() {
    let project = ProjectFile {
        folders: vec![FolderEntry {
            path: PathBuf::from("/var/log"),
            name: Some("Logs".to_owned()),
            folder_exclude_patterns: vec![],
            file_exclude_patterns: vec![],
        }],
        excludes: vec![],
        settings: Map::new(),
        build_systems: vec![],
    };

    let workspace = Workspace::from_project_file(&project, "/home/user/lux");
    assert_eq!(workspace.roots().len(), 1);
    assert_eq!(workspace.roots()[0].path, Path::new("/var/log"));
    assert_eq!(workspace.roots()[0].name, "Logs");
}

#[test]
fn workspace_carries_excludes_and_settings_from_the_project_file() {
    let mut settings = Map::new();
    settings.insert("formatOnSave".to_owned(), Value::Bool(true));
    let project = ProjectFile {
        folders: vec![],
        excludes: vec!["dist/**".to_owned()],
        settings: settings.clone(),
        build_systems: vec![],
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
            folder_exclude_patterns: vec![],
            file_exclude_patterns: vec![],
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
fn discover_loads_sublime_project_when_present() {
    let dir = temp_dir();
    fs::create_dir_all(dir.join("src")).unwrap();
    let sublime_project_json = r#"{
        "folders": [
            {
                "path": "src",
                "name": "Sources",
                "folder_exclude_patterns": [".git", "build"],
                "file_exclude_patterns": ["*.pyc"]
            }
        ],
        "settings": {
            "tab_size": 2
        },
        "build_systems": [
            {
                "name": "Build My App",
                "cmd": ["make", "app"]
            }
        ]
    }"#;
    fs::write(dir.join("my_app.sublime-project"), sublime_project_json).unwrap();

    let workspace = Workspace::discover(&dir).unwrap();
    assert_eq!(workspace.roots().len(), 1);
    assert_eq!(workspace.roots()[0].path, dir.join("src"));
    assert_eq!(workspace.roots()[0].name, "Sources");
    assert_eq!(
        workspace.settings().get("tab_size").unwrap(),
        &Value::from(2)
    );
    assert_eq!(workspace.build_systems().len(), 1);
    assert_eq!(workspace.build_systems()[0].name, "Build My App");
    assert_eq!(workspace.build_systems()[0].cmd, vec!["make", "app"]);
    assert_eq!(
        workspace.project_path(),
        Some(dir.join("my_app.sublime-project").as_path())
    );

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
