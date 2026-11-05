use super::*;
use lux_core::buffer::Buffer;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, FileDialogs, Key, NamedKey, PlatformContext};
use std::path::PathBuf;
use std::sync::Arc;

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<PathBuf> {
        vec![]
    }
    fn pick_folder(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
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

fn test_app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

fn send_key(app: &mut App, event: &KeyEvent) {
    let ctx = DummyContext;
    if app.ui.goto_anything.is_some() && crate::goto::handle_goto_key(app, &ctx, event) {
        return;
    }
    app.handle_key(event);
}

fn type_str(app: &mut App, s: &str) {
    for ch in s.chars() {
        let str_val = ch.to_string();
        send_key(
            app,
            &KeyEvent {
                key: Key::Character(str_val.clone()),
                text: Some(str_val),
                state: ElementState::Pressed,
                repeat: false,
                physical: None,
            },
        );
    }
}

fn press_key(app: &mut App, key: NamedKey) {
    send_key(
        app,
        &KeyEvent {
            key: Key::Named(key),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );
}

#[test]
fn goto_colon_line_jumps_live_to_line() {
    let text = (1..=100).map(|i| format!("line {i}\n")).collect::<String>();
    let mut app = test_app(&text);
    assert_eq!(app.cursors.primary().head.line, 0);

    // 1. Open Goto Anything and type :50 -> jumps live to line 50
    app.open_goto_anything("");
    assert!(app.ui.goto_anything.is_some());

    type_str(&mut app, ":50");
    assert_eq!(app.cursors.primary().head.line, 49);

    // 2. Type :75:4 -> jumps to line 75 column 4 (0-based line 74 col 3)
    press_key(&mut app, NamedKey::Backspace);
    press_key(&mut app, NamedKey::Backspace);
    press_key(&mut app, NamedKey::Backspace);
    type_str(&mut app, ":75:4");
    assert_eq!(app.cursors.primary().head.line, 74);
    assert_eq!(app.cursors.primary().head.column, 3);

    // 3. Press Enter confirms and closes Goto Anything
    press_key(&mut app, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.cursors.primary().head.line, 74);
}

#[test]
fn goto_at_symbol_lists_functions_and_types() {
    let text = r#"
fn compute_sum(a: i32, b: i32) -> i32 {
    a + b
}

struct Engine {
    speed: u32,
}

trait Steerable {
    fn steer(&self);
}
"#;
    let mut app = test_app(text);
    app.set_language_mode(Some(lux_syntax::LanguageId::Rust));

    // Open Goto Symbol mode with "@"
    app.open_goto_anything("@");
    assert!(app.ui.goto_anything.is_some());

    let state = app.ui.goto_anything.as_ref().unwrap();
    let names: Vec<&str> = state.items.iter().map(|it| it.label.as_str()).collect();
    assert!(names.contains(&"compute_sum"));
    assert!(names.contains(&"Engine"));
    assert!(names.contains(&"Steerable"));

    // Type "Engine" to filter
    type_str(&mut app, "Engine");
    let state = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].label, "Engine");

    // Press Enter to jump to Engine
    press_key(&mut app, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.cursors.primary().head.line, 5);
}

#[test]
fn goto_hash_term_searches_in_document() {
    let text = "first row\nsecond row target here\nthird row\nfourth row target again\n";
    let mut app = test_app(text);

    // Open Goto Word mode with "#"
    app.open_goto_anything("#");
    assert!(app.ui.goto_anything.is_some());

    type_str(&mut app, "target");
    let state = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(state.items.len(), 2);
    assert!(state.items[0].label.contains("target"));

    // Press Enter to jump to first occurrence (line 2, 0-based line 1)
    press_key(&mut app, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.cursors.primary().head.line, 1);
}

#[test]
fn goto_file_colon_line_transient_preview_and_escape() {
    let lines_1 = (1..=50)
        .map(|i| format!("file1 line {i}\n"))
        .collect::<String>();
    let lines_2 = (1..=50)
        .map(|i| format!("file2 line {i}\n"))
        .collect::<String>();

    let mut app = test_app(&lines_1);
    let tab1_id = app
        .documents
        .active_tab
        .unwrap_or_else(|| app.documents.tabs.open("file1.rs"));
    app.documents.active_tab = Some(tab1_id);
    app.documents.path = Some(PathBuf::from("file1.rs"));

    // Open second document in another tab
    let tab2_id = app.documents.tabs.open("file2.rs");
    let buf2 = Buffer::from_bytes(lines_2.as_bytes()).unwrap();
    app.documents.entries.push(crate::document::DocumentEntry {
        tab_id: tab2_id,
        buffer_id: 0,
        path: Some(PathBuf::from("file2.rs")),
        buffer: buf2,
        cursors: MultiCursor::new(Selection::cursor(Position::document_start())),
        goal_columns: Vec::new(),
        history: lux_core::history::History::new(),
        saved_version: 0,
        disk_snapshot: None,
        language: None,
        scroll_x: 0.0,
        scroll_y: 0.0,
        snippet_session: None,
        fold_state: lux_core::folding::FoldState::new(),
        bookmarks: lux_core::bookmarks::BookmarkState::new(),
        markdown_preview: false,
    });

    // Make sure we are on tab1 at line 1
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.cursors.primary().head.line, 0);

    // Open Goto Anything and type file2.rs:30
    app.open_goto_anything("");
    type_str(&mut app, "file2.rs:30");

    // Live preview switched to file2 at line 30 (0-based 29)
    assert_eq!(app.documents.active_tab, Some(tab2_id));
    assert_eq!(app.cursors.primary().head.line, 29);

    // Press Escape -> restores file1 and original cursor at line 1
    press_key(&mut app, NamedKey::Escape);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.cursors.primary().head.line, 0);
}

#[test]
fn goto_symbol_in_project_opens_modal_immediately() {
    let mut app = test_app("fn local_func() {}\n");
    if cfg!(target_os = "macos") {
        app.modifiers.meta = true;
    } else {
        app.modifiers.control = true;
    }
    app.modifiers.shift = true;

    // Cmd+Shift+R on macOS, Ctrl+Shift+R on Windows and Linux.
    send_key(
        &mut app,
        &KeyEvent {
            key: Key::Character("r".to_string()),
            text: Some("r".to_string()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );

    assert!(app.ui.goto_anything.is_some());
    let goto = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(goto.mode, crate::goto::GotoMode::ProjectSymbols);
}

#[test]
fn goto_symbol_in_project_finds_symbols_across_open_tabs() {
    let mut app = test_app("pub fn first_tab_handler() {}\n");
    let tab1_id = app
        .documents
        .active_tab
        .unwrap_or_else(|| app.documents.tabs.open("tab1.rs"));
    app.documents.active_tab = Some(tab1_id);
    app.documents.path = Some(PathBuf::from("tab1.rs"));

    // Add tab 2 with another function
    let tab2_id = app.documents.tabs.open("tab2.rs");
    let tab2_code = "\n\npub fn second_tab_calculator(x: i32) -> i32 { x * 2 }\n";
    let buf2 = Buffer::from_bytes(tab2_code.as_bytes()).unwrap();
    app.documents.entries.push(crate::document::DocumentEntry {
        tab_id: tab2_id,
        buffer_id: 0,
        path: Some(PathBuf::from("tab2.rs")),
        buffer: buf2,
        cursors: MultiCursor::new(Selection::cursor(Position::document_start())),
        goal_columns: Vec::new(),
        history: lux_core::history::History::new(),
        saved_version: 0,
        disk_snapshot: None,
        language: None,
        scroll_x: 0.0,
        scroll_y: 0.0,
        snippet_session: None,
        fold_state: lux_core::folding::FoldState::new(),
        bookmarks: lux_core::bookmarks::BookmarkState::new(),
        markdown_preview: false,
    });

    // Open Goto Symbol in Project
    app.open_goto_project_symbols();
    assert!(app.ui.goto_anything.is_some());

    let state = app.ui.goto_anything.as_ref().unwrap();
    let names: Vec<&str> = state.items.iter().map(|it| it.label.as_str()).collect();
    assert!(names.contains(&"first_tab_handler"));
    assert!(names.contains(&"second_tab_calculator"));

    // Filter by typing "second"
    type_str(&mut app, "second");
    let state = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].label, "second_tab_calculator");

    // Press Enter to confirm jump
    press_key(&mut app, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());

    // Verified: switched to tab2 at line 3 (0-based line 2)
    assert_eq!(app.documents.active_tab, Some(tab2_id));
    assert_eq!(app.cursors.primary().head.line, 2);
}

#[test]
fn goto_symbol_in_project_indexes_workspace_and_jumps() {
    let dir = std::env::temp_dir().join(format!(
        "lux-goto-proj-test-{:?}-{}",
        std::thread::current().id(),
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);

    let f1 = dir.join("math.rs");
    let f2 = dir.join("geom.rs");
    std::fs::write(&f1, "pub fn compute_factorial(n: u64) -> u64 { n }\n").unwrap();
    std::fs::write(
        &f2,
        "\n\npub fn calculate_hypotenuse(a: f64, b: f64) -> f64 { a + b }\n",
    )
    .unwrap();

    let mut app = test_app("fn main() {}\n");
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(dir.clone());
    app.ui.session.init_watcher(&dir);

    // Open Goto Symbol in Project
    app.open_goto_project_symbols();
    assert!(app.ui.goto_anything.is_some());

    // Search for calculate_hypotenuse
    type_str(&mut app, "hypotenuse");
    let state = app.ui.goto_anything.as_ref().unwrap();
    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].label, "calculate_hypotenuse");

    // Press Enter to open and jump to file
    press_key(&mut app, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());

    // Check that geom.rs is opened and cursor is at line 3 (0-based 2)
    assert_eq!(app.cursors.primary().head.line, 2);
    let doc_path = app.documents.path.as_ref().unwrap();
    assert!(doc_path.ends_with("geom.rs"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn goto_symbol_in_project_escape_restores_snapshot() {
    let mut app = test_app("fn start() {}\n");
    let tab1_id = app
        .documents
        .active_tab
        .unwrap_or_else(|| app.documents.tabs.open("start.rs"));
    app.documents.active_tab = Some(tab1_id);
    app.documents.path = Some(PathBuf::from("start.rs"));

    let tab2_id = app.documents.tabs.open("other.rs");
    let other_code = "\n\n\npub fn other_function() {}\n";
    let buf2 = Buffer::from_bytes(other_code.as_bytes()).unwrap();
    app.documents.entries.push(crate::document::DocumentEntry {
        tab_id: tab2_id,
        buffer_id: 0,
        path: Some(PathBuf::from("other.rs")),
        buffer: buf2,
        cursors: MultiCursor::new(Selection::cursor(Position::document_start())),
        goal_columns: Vec::new(),
        history: lux_core::history::History::new(),
        saved_version: 0,
        disk_snapshot: None,
        language: None,
        scroll_x: 0.0,
        scroll_y: 0.0,
        snippet_session: None,
        fold_state: lux_core::folding::FoldState::new(),
        bookmarks: lux_core::bookmarks::BookmarkState::new(),
        markdown_preview: false,
    });

    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.cursors.primary().head.line, 0);

    // Open Goto Symbol in Project and search other_function
    app.open_goto_project_symbols();
    type_str(&mut app, "other_function");

    // Live preview switches to tab2
    assert_eq!(app.documents.active_tab, Some(tab2_id));
    assert_eq!(app.cursors.primary().head.line, 3);

    // Press Escape -> restores original tab and cursor
    press_key(&mut app, NamedKey::Escape);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.documents.active_tab, Some(tab1_id));
    assert_eq!(app.cursors.primary().head.line, 0);
}
