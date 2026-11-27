use super::*;
use lux_platform::{FileDialogs, PlatformContext, PlatformEventHandler};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    fn confirm_trash(&self, _: &str) -> bool {
        false
    }
}

static DUMMY_DIALOGS: DummyDialogs = DummyDialogs;

/// A context whose background waker is a real event: every worker wake
/// lands on `wakes`, and the test answers each with `background_event`,
/// the same call the platform loop makes.
struct DummyContext {
    waker: Arc<dyn Fn() + Send + Sync>,
    wakes: Mutex<Receiver<()>>,
}

impl DummyContext {
    fn new() -> Self {
        let (tx, rx) = channel();
        let tx = Mutex::new(tx);
        Self {
            waker: Arc::new(move || {
                let _ = tx.lock().unwrap().send(());
            }),
            wakes: Mutex::new(rx),
        }
    }
}

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
        Arc::clone(&self.waker)
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

fn test_app_with_root(root: PathBuf, ctx: &DummyContext) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    app.ui.sidebar = crate::sidebar::SidebarState::open_folder(root);
    app.ui.session.register_waker(ctx.background_waker());
    app
}

/// Answers worker wakes with `background_event` until `done` holds. No key
/// is pressed and nothing blocks on the search itself.
fn drive(app: &mut App, ctx: &DummyContext, done: impl Fn(&App) -> bool) {
    while !done(app) {
        ctx.wakes
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(30))
            .expect("a background worker wakes the loop");
        // `background_event` also drains the process-wide git mailbox.
        let _mailbox = crate::git::MAILBOX_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        app.background_event(ctx);
    }
}

fn search(app: &mut App, ctx: &DummyContext, query: &str, where_query: &str) {
    crate::project_search::open_project_find(app);
    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text(query);
        state.where_query = crate::field::QueryField::from_text(where_query);
    }
    crate::project_search::execute_find_in_files(app, Some(ctx));
    drive(app, ctx, |app| !crate::find_stream::is_running(app));
}

fn find_results_text(app: &App) -> String {
    app.find_results_buffer().expect("Find Results tab").text()
}

fn active_title(app: &App) -> Option<String> {
    app.documents
        .tabs
        .tabs()
        .iter()
        .find(|t| Some(t.id) == app.documents.active_tab)
        .map(|t| t.title.clone())
}

#[test]
fn find_in_files_streams_into_find_results_tab_on_wakes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    std::fs::write(
        root.join("alpha.rs"),
        "fn alpha() {\n    let target = 42;\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("beta.rs"),
        "fn beta() {\n    let other = target;\n}\n",
    )
    .unwrap();
    std::fs::write(root.join("gamma.rs"), "fn gamma() {}\n").unwrap();

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);

    crate::project_search::open_project_find(&mut app);
    assert!(app.ui.project_search.is_some());
    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text("target");
    }
    crate::project_search::execute_find_in_files(&mut app, Some(&ctx));

    // The tab exists with its header before any result has been drained.
    assert!(app.ui.project_search.is_none());
    assert!(app.is_find_results_active());
    let header = app.buffer.text();
    let mut lines = header.lines();
    assert_eq!(lines.next(), Some("Searching files for \"target\""));
    let root_line = lines.next().unwrap();
    assert!(root_line.starts_with("Searching in "), "{root_line}");
    assert_eq!(lines.next(), Some(""));

    drive(&mut app, &ctx, |app| !crate::find_stream::is_running(app));
    let content = app.buffer.text();
    assert!(content.contains("alpha.rs:"));
    assert!(content.contains("beta.rs:"));
    assert!(
        content.ends_with("\n2 matches across 2 files (3 files searched)\n"),
        "{content}"
    );
    assert_eq!(app.ui.find_results_nav.matches.len(), 2);
}

#[test]
fn find_in_files_with_where_glob_filters() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    std::fs::write(root.join("code.rs"), "const VAL: &str = \"needle\";\n").unwrap();
    std::fs::write(root.join("notes.txt"), "found needle in text\n").unwrap();

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);
    search(&mut app, &ctx, "needle", "*.rs, -*.txt");

    assert!(app.is_find_results_active());
    let content = app.buffer.text();
    assert!(content.contains("code.rs:"));
    assert!(!content.contains("notes.txt:"));
    assert!(content.contains("1 match across 1 file (1 file searched)"));
    assert_eq!(app.ui.find_results_nav.matches.len(), 1);
}

#[test]
fn thousands_of_files_all_land_in_find_results() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let files = 5000;
    let mut expected = 0;
    for i in 0..files {
        let dir = root.join(format!("d{}", i % 50));
        std::fs::create_dir_all(&dir).unwrap();
        let body = match i % 4 {
            0 => "needle one\nhay\nneedle two\n",
            1 => "needle\n",
            _ => "hay only\n",
        };
        expected += body.matches("needle").count();
        std::fs::write(dir.join(format!("f{i}.txt")), body).unwrap();
    }
    let matching_files = files / 2;

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);
    search(&mut app, &ctx, "needle", "");

    assert_eq!(app.ui.find_results_nav.matches.len(), expected);
    let content = find_results_text(&app);
    let buffer = lux_core::buffer::Buffer::from_bytes(content.as_bytes()).unwrap();
    let results_root =
        crate::find_results::header::root(&buffer.line_content(1).unwrap()).expect("run root");
    let parsed = crate::find_results::parse_matches_from_buffer(&buffer, &results_root);
    assert_eq!(parsed.len(), expected);
    assert!(
        parsed.iter().all(|m| m.path.is_file()),
        "relative headers resolve to the searched files"
    );
    assert_eq!(
        content.matches(&results_root.display().to_string()).count(),
        1,
        "only the root line names the folder; headers are relative to it"
    );
    let distinct: std::collections::HashSet<_> = parsed.iter().map(|m| &m.path).collect();
    assert_eq!(distinct.len(), matching_files, "each file has one block");
    assert!(
        content.ends_with(&format!(
            "\n{expected} matches across {matching_files} files ({files} files searched)\n"
        )),
        "{}",
        &content[content.len().saturating_sub(200)..]
    );
}

#[test]
fn a_new_search_cancels_the_previous_run() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    for i in 0..500 {
        std::fs::write(root.join(format!("old{i}.txt")), "first_query\n").unwrap();
    }
    std::fs::write(root.join("new.txt"), "second_query\n").unwrap();

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);
    crate::project_search::open_project_find(&mut app);
    if let Some(state) = app.ui.project_search.as_mut() {
        state.find_query = crate::field::QueryField::from_text("first_query");
    }
    crate::project_search::execute_find_in_files(&mut app, Some(&ctx));
    assert!(crate::find_stream::is_running(&app));

    search(&mut app, &ctx, "second_query", "");
    let content = find_results_text(&app);
    assert!(content.starts_with("Searching files for \"second_query\""));
    assert!(!content.contains("first_query"), "{content}");
    assert!(content.contains("1 match across 1 file (501 files searched)"));
    assert_eq!(app.ui.find_results_nav.matches.len(), 1);
}

#[test]
fn find_panel_live_list_fills_on_wakes_without_keys() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    for i in 0..40 {
        std::fs::write(root.join(format!("f{i}.txt")), "panel_hit\n").unwrap();
    }
    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);
    crate::project_search::open_project_find(&mut app);
    let workspace = app
        .ui
        .sidebar
        .as_ref()
        .unwrap()
        .workspace_root
        .clone()
        .unwrap();
    let state = app.ui.project_search.as_mut().unwrap();
    state.find_query = crate::field::QueryField::from_text("panel_hit");
    state.start_search(&workspace);
    drive(&mut app, &ctx, |app| {
        app.ui
            .project_search
            .as_ref()
            .is_some_and(|s| s.stream.is_none())
    });
    assert_eq!(app.ui.project_search.as_ref().unwrap().matches.len(), 40);
}

fn line_index_containing(app: &App, needle: &str) -> usize {
    (0..app.buffer.len_lines())
        .find(|&i| {
            app.buffer
                .line_content(i)
                .unwrap_or_default()
                .contains(needle)
        })
        .expect("line in Find Results")
}

#[test]
fn enter_or_double_click_on_result_line_opens_file_off_the_ui_thread() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    std::fs::write(root.join("main.rs"), "line 1\nline 2: target\nline 3\n").unwrap();

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);
    search(&mut app, &ctx, "target", "");
    assert!(app.is_find_results_active());

    let match_line = line_index_containing(&app, "line 2: target");
    assert!(crate::find_results::open_find_result_at_line(
        &mut app,
        Some(&ctx),
        match_line
    ));
    // The document worker reads the file; the tab appears on its wake.
    assert!(crate::document::open_queue::busy(&app));
    drive(&mut app, &ctx, |app| {
        active_title(app).as_deref() == Some("main.rs") && !crate::document::open_queue::busy(app)
    });
    assert_eq!(app.cursors.primary().head.line, 1);
}

#[test]
fn enter_or_double_click_on_file_header_line_jumps_to_line_1() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    std::fs::write(
        root.join("header_test.rs"),
        "first line\nsecond line with query\n",
    )
    .unwrap();

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);
    search(&mut app, &ctx, "query", "");

    let header_line = line_index_containing(&app, "header_test.rs:");
    assert!(crate::find_results::open_find_result_at_line(
        &mut app,
        Some(&ctx),
        header_line
    ));
    drive(&mut app, &ctx, |app| {
        active_title(app).as_deref() == Some("header_test.rs")
            && !crate::document::open_queue::busy(app)
    });
    assert_eq!(app.cursors.primary().head.line, 0);
}

#[test]
fn f4_and_shift_f4_navigate_sequentially_through_matches() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    std::fs::write(root.join("first.rs"), "// item 1: target_token\n").unwrap();
    std::fs::write(root.join("second.rs"), "// item 2: target_token\n").unwrap();

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(root, &ctx);
    search(&mut app, &ctx, "target_token", "");
    assert_eq!(app.ui.find_results_nav.active_index, None);

    let settle = |app: &mut App| {
        drive(app, &ctx, |app| !crate::document::open_queue::busy(app));
    };
    crate::find_results::next_result(&mut app, Some(&ctx));
    settle(&mut app);
    assert_eq!(app.ui.find_results_nav.active_index, Some(0));
    let first = app.ui.find_results_nav.matches[0].path.clone();
    assert_eq!(app.documents.path.as_ref(), Some(&first));

    crate::find_results::next_result(&mut app, Some(&ctx));
    settle(&mut app);
    assert_eq!(app.ui.find_results_nav.active_index, Some(1));
    let second = app.ui.find_results_nav.matches[1].path.clone();
    assert_eq!(app.documents.path.as_ref(), Some(&second));

    crate::find_results::next_result(&mut app, Some(&ctx));
    settle(&mut app);
    assert_eq!(app.ui.find_results_nav.active_index, Some(0));
    assert_eq!(app.documents.path.as_ref(), Some(&first));

    crate::find_results::prev_result(&mut app, Some(&ctx));
    settle(&mut app);
    assert_eq!(app.ui.find_results_nav.active_index, Some(1));
}

#[test]
fn f4_populates_nav_from_find_results_buffer_if_empty() {
    let ctx = DummyContext::new();
    let mut app = test_app_with_root(PathBuf::from("."), &ctx);

    let results_text = "Searching files for \"hello\"\n\n/tmp/test.rs:\n     5: hello world\n\n1 match across 1 file (1 file searched)\n";
    crate::find_results::open_find_results_tab(&mut app, results_text);
    app.ui.find_results_nav.matches.clear();

    crate::find_results::next_result(&mut app, Some(&ctx));
    assert_eq!(app.ui.find_results_nav.matches.len(), 1);
    assert_eq!(app.ui.find_results_nav.matches[0].line_number, 5);
}

/// A restored tab knows nothing but its text: F4 must resolve its relative
/// headers against the folder that was searched, not against whichever
/// folder happens to be first (or not yet loaded) after a restart.
#[test]
fn a_restored_tab_resolves_against_its_recorded_root_after_a_restart() {
    let searched = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    std::fs::write(searched.path().join("found.rs"), "x\nrestart_token\n").unwrap();
    std::fs::write(other.path().join("found.rs"), "decoy\n").unwrap();

    let ctx = DummyContext::new();
    let mut app = test_app_with_root(searched.path().to_path_buf(), &ctx);
    let other_root = std::fs::canonicalize(other.path()).unwrap();
    app.ui
        .sidebar
        .as_mut()
        .unwrap()
        .add_folder(other_root.clone());
    search(&mut app, &ctx, "restart_token", "");
    let text = find_results_text(&app);
    let expected = app.ui.find_results_nav.matches[0].path.clone();
    assert!(text.contains("\nfound.rs:\n"), "relative header: {text}");

    // After a restart the searched folder is gone from the project, and
    // in a second case the sidebar has not loaded yet at all.
    for sidebar_root in [Some(other_root), None] {
        let mut restored = test_app_with_root(PathBuf::from("."), &ctx);
        restored.ui.sidebar = sidebar_root.and_then(crate::sidebar::SidebarState::open_folder);
        crate::find_results::open_find_results_tab(&mut restored, &text);
        assert!(restored.ui.find_results_nav.matches.is_empty());
        crate::find_results::next_result(&mut restored, Some(&ctx));
        drive(&mut restored, &ctx, |app| {
            !crate::document::open_queue::busy(app)
        });
        assert_eq!(restored.ui.find_results_nav.matches[0].path, expected);
        assert_eq!(restored.documents.path.as_ref(), Some(&expected));
        assert_eq!(restored.cursors.primary().head.line, 1);
    }
}

/// With no folder open the search root is the working directory, written
/// as an absolute path so a later project cannot change what it means.
#[test]
fn the_root_without_a_folder_is_absolute() {
    let ctx = DummyContext::new();
    let mut app = test_app_with_root(PathBuf::from("."), &ctx);
    app.ui.sidebar = None;
    let root = crate::project_search::workspace_root(&app);
    assert!(root.is_absolute(), "{}", root.display());
}
