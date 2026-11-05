use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::build::detect_build_variants;
use crate::document::DocumentEntry;
use crate::input::dispatch::execute_keymap_command;
use crate::project::{ProjectSwitchState, RecentProjects, handle_project_switch_key};
use lux_core::buffer::Buffer;
use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{FileDialogOptions, FileDialogs, Key, NamedKey, PlatformContext};
use tempfile::tempdir;

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        vec![]
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
        lux_platform::dialog::SaveDecision::Discard
    }
}

static DUMMY_DIALOGS: DummyDialogs = DummyDialogs;

struct DummyContext;
impl lux_platform::Clipboard for DummyContext {
    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
        Ok(())
    }
}
impl PlatformContext for DummyContext {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(DummyDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &DUMMY_DIALOGS
    }
    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        self
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
        panic!("unused")
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
    fn exit(&self) {}
}

fn create_test_app() -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.viewport.width = 800.0;
    app.viewport.height = 600.0;
    let path = PathBuf::from("main.py");
    app.documents.path = Some(path.clone());
    let tab_id = app.documents.tabs.open("main.py");
    let buf = Buffer::from_bytes(b"print('hello')\n").unwrap();
    app.documents
        .entries
        .push(DocumentEntry::new(tab_id, Some(path), buf));
    app
}

#[test]
fn test_open_sublime_project_loads_folders_and_settings() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    let project_json = r#"{
        "folders": [
            {
                "path": "src",
                "name": "Sources",
                "folder_exclude_patterns": ["target", ".git"]
            }
        ],
        "settings": {
            "tab_size": 2
        },
        "build_systems": [
            {
                "name": "Custom Make",
                "cmd": ["make", "check"]
            }
        ]
    }"#;
    let proj_file = dir.path().join("my_app.sublime-project");
    std::fs::write(&proj_file, project_json).unwrap();

    let mut app = create_test_app();
    let ctx = DummyContext;
    let opened = app.open_project_file(&proj_file, Some(&ctx));
    assert!(opened);

    assert_eq!(app.ui.project.active_project_path, Some(proj_file.clone()));
    assert_eq!(app.workspace_root(), Some(dir.path().to_path_buf()));
    assert_eq!(app.buffer.indent().width, 2);
    assert_eq!(
        app.buffer.indent().style,
        lux_core::buffer::IndentStyle::Spaces
    );

    let custom_builds = app.ui.project.custom_build_systems();
    assert_eq!(custom_builds.len(), 1);
    assert_eq!(custom_builds[0].name, "Custom Make");
    assert_eq!(custom_builds[0].cmd, vec!["make", "check"]);

    // Test that the custom build system is picked up by build variant detection
    let variants = detect_build_variants(app.workspace_root().as_deref(), None, custom_builds);
    assert!(!variants.is_empty());
    assert_eq!(variants[0].name, "Custom Make");
    assert_eq!(variants[0].command, "make");
    assert_eq!(variants[0].args, vec!["check"]);
}

#[test]
fn project_tab_size_keeps_detected_tabs() {
    let dir = tempdir().unwrap();
    let proj_file = dir.path().join("tabs.sublime-project");
    std::fs::write(
        &proj_file,
        r#"{ "folders": [{ "path": "." }], "settings": { "tab_size": 2 } }"#,
    )
    .unwrap();

    let mut app = create_test_app();
    let original = "fn main() {\n\tlet x = 1;\n}\n";
    app.buffer = Buffer::from_bytes(original.as_bytes()).unwrap();
    assert_eq!(
        app.buffer.indent().style,
        lux_core::buffer::IndentStyle::Tabs
    );
    assert!(app.open_project_file(&proj_file, Some(&DummyContext)));
    assert_eq!(
        app.buffer.indent().style,
        lux_core::buffer::IndentStyle::Tabs
    );

    app.cursors = lux_core::multicursor::MultiCursor::new(lux_core::selection::Selection::cursor(
        lux_core::position::Position::document_start(),
    ));
    crate::input::indent::insert_tab(&mut app);
    assert_eq!(app.buffer.text(), format!("\t{original}"));
    execute_keymap_command(&mut app, "undo", None, None);
    assert_eq!(app.buffer.text(), original);
    execute_keymap_command(&mut app, "redo", None, None);
    assert_eq!(app.buffer.text(), format!("\t{original}"));
}

#[test]
fn test_project_switch_picker_filtering_and_navigation() {
    let p1 = PathBuf::from("/Users/dev/lux/lux.sublime-project");
    let p2 = PathBuf::from("/Users/dev/engine/engine.sublime-project");
    let p3 = PathBuf::from("/Users/dev/web/site.lux-project");

    let mut picker = ProjectSwitchState {
        projects: vec![p1.clone(), p2.clone(), p3.clone()],
        query: crate::field::QueryField::new(),
        selected: 0,
    };

    assert_eq!(picker.filtered().len(), 3);
    assert_eq!(picker.selected, 0);

    // Navigation
    picker.select_next(3);
    assert_eq!(picker.selected, 1);
    picker.select_next(3);
    assert_eq!(picker.selected, 2);
    picker.select_next(3); // clamped at 2
    assert_eq!(picker.selected, 2);
    picker.select_prev();
    assert_eq!(picker.selected, 1);

    // Filter by query
    picker.query = crate::field::QueryField::from_text("engine");
    let filtered = picker.filtered();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0], &p2);
}

#[test]
fn test_quick_switch_project_key_lifecycle() {
    let mut app = create_test_app();
    let ctx = DummyContext;

    let dir = tempdir().unwrap();
    let proj_file = dir.path().join("demo.sublime-project");
    std::fs::write(&proj_file, r#"{"folders":[{"path":"."}]}"#).unwrap();

    app.open_project_file(&proj_file, Some(&ctx));
    let opened = app.quick_switch_project();
    assert!(opened);
    assert!(app.ui.project.picker.is_some());

    // Type query
    let key_d = KeyEvent {
        key: Key::Character("d".into()),
        text: Some("d".into()),
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    handle_project_switch_key(&mut app, &ctx, &key_d);
    assert_eq!(app.ui.project.picker.as_ref().unwrap().query, "d");

    // Backspace
    let key_bs = KeyEvent {
        key: Key::Named(NamedKey::Backspace),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    handle_project_switch_key(&mut app, &ctx, &key_bs);
    assert_eq!(app.ui.project.picker.as_ref().unwrap().query, "");

    // Escape closes picker
    let key_esc = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    handle_project_switch_key(&mut app, &ctx, &key_esc);
    assert!(app.ui.project.picker.is_none());
}

#[test]
fn test_save_project_as_and_close_project() {
    let mut app = create_test_app();
    let dir = tempdir().unwrap();
    let save_path = dir.path().join("saved.sublime-project");

    let saved = app.save_project_as(&save_path);
    assert!(saved);
    assert!(save_path.exists());
    assert_eq!(app.ui.project.active_project_path, Some(save_path.clone()));

    // Verify written file is valid JSON
    let content = std::fs::read_to_string(&save_path).unwrap();
    assert!(content.contains("folders"));

    // Close project
    app.close_project();
    assert!(app.ui.project.active_project_path.is_none());
    assert!(app.ui.project.workspace.is_none());
}

#[test]
fn test_project_keymap_command_dispatch() {
    let mut app = create_test_app();
    let ctx = DummyContext;

    let dir = tempdir().unwrap();
    let proj = dir.path().join("test.sublime-project");
    std::fs::write(&proj, r#"{"folders":[]}"#).unwrap();
    app.ui.project.active_project_path = Some(proj);

    // Dispatch quick_switch_project
    execute_keymap_command(&mut app, "quick_switch_project", Some(&ctx), None);
    assert!(app.ui.project.picker.is_some());

    // Dismiss
    let key_esc = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    handle_project_switch_key(&mut app, &ctx, &key_esc);
    assert!(app.ui.project.picker.is_none());

    // Dispatch close_project
    execute_keymap_command(&mut app, "close_project", Some(&ctx), None);
    assert!(app.ui.project.active_project_path.is_none());
}

#[test]
fn test_recent_projects_record_and_limit() {
    let mut recents = RecentProjects::default();
    for i in 0..25 {
        recents.record(PathBuf::from(format!(
            "/path/to/project_{}.sublime-project",
            i
        )));
    }
    assert_eq!(recents.paths.len(), 20);
    assert_eq!(
        recents.paths[0],
        PathBuf::from("/path/to/project_24.sublime-project")
    );
}
