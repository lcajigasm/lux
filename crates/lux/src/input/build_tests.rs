use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::build::{
    BuildPickerState, BuildVariant, detect_build_variants, handle_build_key, parse_build_errors,
};
use crate::document::DocumentEntry;
use crate::input::dispatch::execute_keymap_command;
use lux_core::buffer::Buffer;
use lux_platform::event::KeyEvent;
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
fn test_detect_build_variants_cargo() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"foo\"\n").unwrap();

    let variants = detect_build_variants(Some(dir.path()), None, &[]);
    assert!(!variants.is_empty());
    let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
    assert!(names.contains(&"Cargo: Check"));
    assert!(names.contains(&"Cargo: Build"));
    assert!(names.contains(&"Cargo: Test"));
    assert!(names.contains(&"Cargo: Clippy"));
    assert!(names.contains(&"Cargo: Run"));

    let check_var = variants.iter().find(|v| v.name == "Cargo: Check").unwrap();
    assert_eq!(check_var.command, "cargo");
    assert_eq!(check_var.args, vec!["check"]);
}

#[test]
fn test_detect_build_variants_package_json_and_file_types() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("package.json"), "{}").unwrap();
    let py_file = dir.path().join("script.py");
    std::fs::write(&py_file, "print('hi')").unwrap();

    let variants = detect_build_variants(Some(dir.path()), Some(&py_file), &[]);
    let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
    assert!(names.contains(&"Python: Run File"));
    assert!(names.contains(&"npm: test"));
    assert!(names.contains(&"npm: build"));

    let py_var = variants
        .iter()
        .find(|v| v.name == "Python: Run File")
        .unwrap();
    assert_eq!(py_var.command, "python3");
    assert_eq!(py_var.args, vec![py_file.to_string_lossy().to_string()]);
}

#[test]
fn test_build_picker_filtering_and_navigation() {
    let variants = vec![
        BuildVariant {
            name: "Cargo: Check".into(),
            command: "cargo".into(),
            args: vec!["check".into()],
            file_regex: None,
        },
        BuildVariant {
            name: "Cargo: Test".into(),
            command: "cargo".into(),
            args: vec!["test".into()],
            file_regex: None,
        },
        BuildVariant {
            name: "Python: Run File".into(),
            command: "python3".into(),
            args: vec!["run.py".into()],
            file_regex: None,
        },
    ];

    let mut picker = BuildPickerState {
        variants,
        query: String::new(),
        selected: 0,
    };
    assert_eq!(picker.filtered().len(), 3);
    assert_eq!(picker.selected, 0);

    // Navigation
    let total = picker.filtered().len();
    picker.select_next(total);
    assert_eq!(picker.selected, 1);
    picker.select_next(total);
    assert_eq!(picker.selected, 2);
    picker.select_next(total); // clamped at 2
    assert_eq!(picker.selected, 2);
    picker.select_prev();
    assert_eq!(picker.selected, 1);
    picker.select_prev();
    assert_eq!(picker.selected, 0);
    picker.select_prev(); // clamped at 0
    assert_eq!(picker.selected, 0);

    // Filtering
    picker.query = "python".into();
    let filtered = picker.filtered();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "Python: Run File");
}

#[test]
fn test_parse_build_errors_standard_and_custom() {
    let output = "src/main.rs:14:5: error: cannot find value `x` in this scope\n";
    let locs = parse_build_errors(output, None);
    assert_eq!(locs.len(), 1);
    assert_eq!(locs[0].path, PathBuf::from("src/main.rs"));
    assert_eq!(locs[0].line, 14);
    assert_eq!(locs[0].column, Some(5));

    // Custom regex variant
    let custom_output = "ERR [app/core.py:42:10] SyntaxError";
    let variant = BuildVariant {
        name: "Custom".into(),
        command: "custom".into(),
        args: vec![],
        file_regex: Some(r"ERR \[([^:]+):([0-9]+):([0-9]+)\]".into()),
    };
    let locs2 = parse_build_errors(custom_output, Some(&variant));
    assert_eq!(locs2.len(), 1);
    assert_eq!(locs2[0].path, PathBuf::from("app/core.py"));
    assert_eq!(locs2[0].line, 42);
    assert_eq!(locs2[0].column, Some(10));
}

#[test]
fn test_build_picker_key_handling_lifecycle() {
    let mut app = create_test_app();
    let ctx = DummyContext;

    // Open Build With... modal picker
    app.build_with();
    assert!(app.ui.build.picker.is_some());

    // Type query
    let key_t = KeyEvent {
        key: Key::Character("t".into()),
        text: Some("t".into()),
        state: lux_platform::event::ElementState::Pressed,
        repeat: false,
    };
    let handled = handle_build_key(&mut app, &ctx, &key_t);
    assert!(handled);
    assert_eq!(app.ui.build.picker.as_ref().unwrap().query, "t");

    // Press Backspace
    let key_bs = KeyEvent {
        key: Key::Named(NamedKey::Backspace),
        text: None,
        state: lux_platform::event::ElementState::Pressed,
        repeat: false,
    };
    handle_build_key(&mut app, &ctx, &key_bs);
    assert_eq!(app.ui.build.picker.as_ref().unwrap().query, "");

    // Press Escape closes picker
    let key_esc = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        text: None,
        state: lux_platform::event::ElementState::Pressed,
        repeat: false,
    };
    handle_build_key(&mut app, &ctx, &key_esc);
    assert!(app.ui.build.picker.is_none());
}

#[test]
fn test_build_keymap_command_dispatch() {
    let mut app = create_test_app();
    let ctx = DummyContext;

    // Dispatch build_with command
    execute_keymap_command(&mut app, "build_with", Some(&ctx));
    assert!(app.ui.build.picker.is_some());

    // Cancel / dismiss via Escape
    let key_esc = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        text: None,
        state: lux_platform::event::ElementState::Pressed,
        repeat: false,
    };
    handle_build_key(&mut app, &ctx, &key_esc);
    assert!(app.ui.build.picker.is_none());

    // Dispatch cancel_build command
    execute_keymap_command(&mut app, "cancel_build", Some(&ctx));

    // Test error navigation commands
    execute_keymap_command(&mut app, "next_build_error", Some(&ctx));
    execute_keymap_command(&mut app, "prev_build_error", Some(&ctx));
}
