use super::*;
use lux_platform::{FileDialogs, PlatformContext};
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

fn test_app_with_root(root: PathBuf) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root);
    app
}

#[test]
fn find_in_files_opens_modal_and_executes_to_find_results_tab() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let file1 = root.join("alpha.rs");
    let file2 = root.join("beta.rs");

    std::fs::write(&file1, "fn alpha() {\n    let target = 42;\n}\n").unwrap();
    std::fs::write(&file2, "fn beta() {\n    let other = target;\n}\n").unwrap();

    let mut app = test_app_with_root(root);
    let ctx = DummyContext;

    crate::project_search::open_project_find(&mut app);
    assert!(app.ui.project_search.is_some());

    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text("target");
    }

    crate::project_search::execute_find_in_files(&mut app, Some(&ctx));

    assert!(app.ui.project_search.is_none());
    assert!(app.is_find_results_active());

    let content = app.buffer.text();
    assert!(content.contains("Searching 2 files for \"target\""));
    assert!(content.contains("alpha.rs:"));
    assert!(content.contains("beta.rs:"));
    assert!(content.contains("2 matches across 2 files"));
    assert_eq!(app.ui.find_results_nav.matches.len(), 2);
}

#[test]
fn find_in_files_with_where_glob_filters() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let rs_file = root.join("code.rs");
    let txt_file = root.join("notes.txt");

    std::fs::write(&rs_file, "const VAL: &str = \"needle\";\n").unwrap();
    std::fs::write(&txt_file, "found needle in text\n").unwrap();

    let mut app = test_app_with_root(root);
    let ctx = DummyContext;

    crate::project_search::open_project_find(&mut app);
    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text("needle");
        state.where_query = crate::field::QueryField::from_text("*.rs, -*.txt");
    }

    crate::project_search::execute_find_in_files(&mut app, Some(&ctx));

    assert!(app.is_find_results_active());
    let content = app.buffer.text();
    assert!(content.contains("code.rs:"));
    assert!(!content.contains("notes.txt:"));
    assert!(content.contains("1 match across 1 file"));
    assert_eq!(app.ui.find_results_nav.matches.len(), 1);
}

#[test]
fn enter_or_double_click_on_result_line_jumps_to_file_and_line() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let file = root.join("main.rs");
    std::fs::write(&file, "line 1\nline 2: target\nline 3\n").unwrap();

    let mut app = test_app_with_root(root);
    let ctx = DummyContext;

    crate::project_search::open_project_find(&mut app);
    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text("target");
    }
    crate::project_search::execute_find_in_files(&mut app, Some(&ctx));
    assert!(app.is_find_results_active());

    // In Find Results buffer, find line containing "line 2: target"
    let mut match_line_idx = None;
    for i in 0..app.buffer.len_lines() {
        if app
            .buffer
            .line_content(i)
            .unwrap_or_default()
            .contains("line 2: target")
        {
            match_line_idx = Some(i);
            break;
        }
    }
    let match_line = match_line_idx.expect("match line found in Find Results");

    let opened = crate::find_results::open_find_result_at_line(&mut app, Some(&ctx), match_line);
    assert!(opened);

    assert_eq!(
        app.documents
            .tabs
            .tabs()
            .iter()
            .find(|t| Some(t.id) == app.documents.active_tab)
            .map(|t| t.title.as_str()),
        Some("main.rs")
    );
    assert_eq!(app.cursors.primary().head.line, 1);
}

#[test]
fn enter_or_double_click_on_file_header_line_jumps_to_line_1() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let file = root.join("header_test.rs");
    std::fs::write(&file, "first line\nsecond line with query\n").unwrap();

    let mut app = test_app_with_root(root);
    let ctx = DummyContext;

    crate::project_search::open_project_find(&mut app);
    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text("query");
    }
    crate::project_search::execute_find_in_files(&mut app, Some(&ctx));

    // Find header line ending with ":"
    let mut header_line_idx = None;
    for i in 0..app.buffer.len_lines() {
        let line = app.buffer.line_content(i).unwrap_or_default();
        if line.contains("header_test.rs:") {
            header_line_idx = Some(i);
            break;
        }
    }
    let header_line = header_line_idx.expect("header line in Find Results");

    let opened = crate::find_results::open_find_result_at_line(&mut app, Some(&ctx), header_line);
    assert!(opened);
    assert_eq!(
        app.documents
            .tabs
            .tabs()
            .iter()
            .find(|t| Some(t.id) == app.documents.active_tab)
            .map(|t| t.title.as_str()),
        Some("header_test.rs")
    );
    assert_eq!(app.cursors.primary().head.line, 0);
}

#[test]
fn f4_and_shift_f4_navigate_sequentially_through_matches() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let file1 = root.join("first.rs");
    let file2 = root.join("second.rs");

    std::fs::write(&file1, "// item 1: target_token\n").unwrap();
    std::fs::write(&file2, "// item 2: target_token\n").unwrap();

    let mut app = test_app_with_root(root);
    let ctx = DummyContext;

    crate::project_search::open_project_find(&mut app);
    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text("target_token");
    }
    crate::project_search::execute_find_in_files(&mut app, Some(&ctx));

    // Initially at None before F4
    assert_eq!(app.ui.find_results_nav.active_index, None);

    // F4 (next_result) advances to match 0
    crate::find_results::next_result(&mut app, Some(&ctx));
    assert_eq!(app.ui.find_results_nav.active_index, Some(0));

    // F4 advances to match 1
    crate::find_results::next_result(&mut app, Some(&ctx));
    assert_eq!(app.ui.find_results_nav.active_index, Some(1));

    // F4 again wraps around to match 0
    crate::find_results::next_result(&mut app, Some(&ctx));
    assert_eq!(app.ui.find_results_nav.active_index, Some(0));

    // Shift+F4 (prev_result) goes back to match 1
    crate::find_results::prev_result(&mut app, Some(&ctx));
    assert_eq!(app.ui.find_results_nav.active_index, Some(1));
}

#[test]
fn f4_populates_nav_from_find_results_buffer_if_empty() {
    let mut app = test_app_with_root(PathBuf::from("."));
    let ctx = DummyContext;

    let results_text = "Searching 1 file for \"hello\"\n\n/tmp/test.rs:\n     5: hello world\n\n1 match across 1 file\n";
    crate::find_results::open_find_results_tab(&mut app, results_text);

    // Nav matches are empty initially
    app.ui.find_results_nav.matches.clear();

    crate::find_results::next_result(&mut app, Some(&ctx));
    assert_eq!(app.ui.find_results_nav.matches.len(), 1);
    assert_eq!(app.ui.find_results_nav.matches[0].line_number, 5);
}
