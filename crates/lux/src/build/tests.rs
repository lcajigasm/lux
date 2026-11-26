//! `.sublime-build` files end to end: loaded on a worker from the user
//! config folder and the project, listed for the active buffer by
//! `selector`, and run with their variables expanded.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use lux_core::buffer::Buffer;
use lux_platform::dialog::SaveDecision;
use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{
    Clipboard, FileDialogOptions, FileDialogs, Key, NamedKey, PlatformContext, PlatformError,
    PlatformWindow, WindowAttributes,
};

use super::{BuildVariant, parse_build_errors, select, systems};
use crate::app::App;
use crate::document::DocumentEntry;

struct NoDialogs;
impl FileDialogs for NoDialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
    fn confirm_trash(&self, _: &str) -> bool {
        false
    }
}

static NO_DIALOGS: NoDialogs = NoDialogs;

struct Context {
    events: Receiver<()>,
    sender: Sender<()>,
}

impl Context {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        Context { events, sender }
    }

    /// Applies worker and watcher results as their wakes arrive until
    /// `done` holds. Each wait is on a wake, never a timer.
    fn settle_until(&self, app: &mut App, mut done: impl FnMut(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !done(app) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "timed out waiting for build systems");
            let _ = self.events.recv_timeout(left);
            crate::ui_io::poll(app, self);
            crate::session::poll_file_watcher(app);
        }
    }
}

impl Clipboard for Context {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl PlatformContext for Context {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        let sender = self.sender.clone();
        Arc::new(move || {
            let _ = sender.send(());
        })
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(NoDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &NO_DIALOGS
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless build test".into()))
    }
    fn request_wakeup(&self, _: Instant) {}
    fn exit(&self) {}
}

/// An app whose user config lives in `config` and whose active tab is
/// `file`, with every wake going to `ctx`.
fn app_with(config: &Path, file: &Path, ctx: &Context) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.ui.settings.settings_path = config.join("settings.json");
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    app.ui.session.register_waker(ctx.background_waker());
    app.documents.path = Some(file.to_path_buf());
    let tab = app.documents.tabs.open(file.display().to_string());
    let buffer = Buffer::from_bytes(b"print('hi')\n").unwrap();
    app.documents
        .entries
        .push(DocumentEntry::new(tab, Some(file.to_path_buf()), buffer));
    app.documents.active_tab = Some(tab);
    app
}

fn load(app: &mut App, ctx: &Context) {
    systems::reload(app);
    let generation = app.ui.build.systems.generation();
    ctx.settle_until(app, |app| {
        app.ui.build.systems.loaded_generation() == generation
    });
}

fn names(variants: &[BuildVariant]) -> Vec<&str> {
    variants.iter().map(|v| v.name.as_str()).collect()
}

/// Waits for the running build's output to contain `needle`.
fn wait_for_output(app: &App, needle: &str) -> String {
    let task = app.ui.build.task_id.expect("a build is running");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let text = app
            .ui
            .term
            .tasks
            .get(task)
            .map(|p| p.snapshot())
            .unwrap_or_default();
        if text.contains(needle) || Instant::now() > deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn press(app: &mut App, ctx: &Context, key: Key) {
    let event = KeyEvent {
        key,
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    };
    super::handle_build_key(app, ctx, &event);
}

const ECHO_BUILD: &str = r#"{
    // A user build file, JSON with comments like Sublime's.
    "cmd": ["echo", "built:${file_name}:${file_base_name}:${file_extension}"],
    "selector": "source.python",
    "file_regex": "^at ([^:]+):([0-9]+)",
    "variants": [
        { "name": "Shell", "shell_cmd": "echo shell:$file_base_name && echo env:\\$LUX_BUILD_TEST",
          "env": { "LUX_BUILD_TEST": "from-env" } },
    ],
}"#;

#[test]
fn a_user_sublime_build_is_listed_and_runs_its_expanded_command() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir(config.path().join("builds")).unwrap();
    std::fs::write(config.path().join("builds/Echo.sublime-build"), ECHO_BUILD).unwrap();
    let file = work.path().join("main.py");
    std::fs::write(&file, "print('hi')\n").unwrap();
    let ctx = Context::new();
    let mut app = app_with(config.path(), &file, &ctx);

    load(&mut app, &ctx);
    let loaded: Vec<&str> = app
        .ui
        .build
        .systems
        .systems
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(loaded, ["Echo"]);

    // Build With lists the system and its variant first.
    assert!(app.build_with());
    let listed = app.ui.build.picker.as_ref().unwrap().variants.clone();
    assert_eq!(&names(&listed)[..2], ["Echo", "Echo - Shell"]);

    // Cmd+B picks it automatically by selector and runs it expanded, in
    // the file's folder (Sublime's default `working_dir`).
    app.ui.build.picker = None;
    assert!(app.build(None));
    let ran = app.ui.build.active_variant.clone().unwrap();
    assert_eq!(ran.name, "Echo");
    assert_eq!(ran.args, ["built:main.py:main:py"]);
    assert_eq!(ran.working_dir.as_deref(), Some(work.path()));
    let output = wait_for_output(&app, "built:main.py:main:py");
    assert!(output.contains("built:main.py:main:py"), "{output}");

    // Build With -> the `shell_cmd` variant runs through the shell with
    // its `env`, and becomes what Cmd+B runs next.
    assert!(app.build_with());
    for c in "shell".chars() {
        press(&mut app, &ctx, Key::Character(c.to_string()));
    }
    press(&mut app, &ctx, Key::Named(NamedKey::Enter));
    assert_eq!(
        app.ui.build.active_variant.as_ref().unwrap().name,
        "Echo - Shell"
    );
    let output = wait_for_output(&app, "env:from-env");
    assert!(output.contains("shell:main"), "{output}");
    assert!(output.contains("env:from-env"), "{output}");
    assert_eq!(select::automatic(&app).unwrap().name, "Echo - Shell");
}

#[test]
fn a_selector_that_does_not_match_leaves_the_system_out() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir(config.path().join("builds")).unwrap();
    std::fs::write(config.path().join("builds/Echo.sublime-build"), ECHO_BUILD).unwrap();
    std::fs::write(
        config.path().join("builds/Any.sublime-build"),
        r#"{ "cmd": ["echo", "any"] }"#,
    )
    .unwrap();
    let file = work.path().join("lib.rs");
    let ctx = Context::new();
    let mut app = app_with(config.path(), &file, &ctx);
    load(&mut app, &ctx);

    let listed = select::candidates(&app);
    assert!(!names(&listed).contains(&"Echo"), "{:?}", names(&listed));
    assert!(names(&listed).contains(&"Any"));
    assert!(names(&listed).contains(&"Rust: Compile File"));

    // A variant picked for Python does not follow the user into Rust.
    app.ui.build.chosen = Some(BuildVariant {
        name: "Echo".into(),
        command: "echo".into(),
        selector: Some("source.python".into()),
        ..BuildVariant::default()
    });
    assert_eq!(select::automatic(&app).unwrap().name, "Any");
}

#[test]
fn editing_the_user_build_folder_reloads_through_its_watcher() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let file = work.path().join("main.py");
    let ctx = Context::new();
    let mut app = app_with(config.path(), &file, &ctx);
    load(&mut app, &ctx);
    assert!(app.ui.build.systems.systems.is_empty());
    assert!(
        config.path().join("builds").is_dir(),
        "created on first load"
    );

    std::fs::write(config.path().join("builds/Echo.sublime-build"), ECHO_BUILD).unwrap();
    ctx.settle_until(&mut app, |app| {
        app.ui
            .build
            .systems
            .systems
            .iter()
            .any(|s| s.name == "Echo")
    });
    assert_eq!(select::automatic(&app).unwrap().name, "Echo");
}

#[test]
fn project_build_files_and_project_build_systems_are_listed() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(
        work.path().join("Here.sublime-build"),
        r#"{ "shell_cmd": "echo ${folder}", "working_dir": "${project_path}" }"#,
    )
    .unwrap();
    std::fs::write(
        work.path().join("app.sublime-project"),
        r#"{
            "folders": [{ "path": "." }],
            "build_systems": [{
                "name": "Proj",
                "cmd": ["make"],
                "working_dir": "${folder}/sub",
                "variants": [{ "name": "Clean", "cmd": ["make", "clean"] }]
            }]
        }"#,
    )
    .unwrap();
    let file = work.path().join("notes.txt");
    let ctx = Context::new();
    let mut app = app_with(config.path(), &file, &ctx);
    assert!(app.open_project_file(&work.path().join("app.sublime-project"), Some(&ctx)));
    load(&mut app, &ctx);

    let listed = select::candidates(&app);
    assert_eq!(&names(&listed)[..3], ["Proj", "Proj - Clean", "Here"]);
    assert_eq!(listed[1].args, ["clean"]);
    assert_eq!(listed[1].working_dir, Some(work.path().join("sub")));
    assert_eq!(listed[2].working_dir.as_deref(), Some(work.path()));
    assert_eq!(
        listed[2].args.last().unwrap(),
        &format!("echo {}", work.path().display())
    );
}

#[test]
fn line_regex_takes_the_file_from_the_last_file_regex_match() {
    let variant = BuildVariant {
        file_regex: Some(r"^In file (\S+)$".into()),
        line_regex: Some(r"^\s+line (\d+), col (\d+)".into()),
        ..BuildVariant::default()
    };
    let output =
        "In file src/a.py\n  line 3, col 7: bad\nIn file src/b.py\n  line 9, col 1: worse\n";
    let found = parse_build_errors(output, Some(&variant));
    let found: Vec<(String, u32, Option<u32>)> = found
        .into_iter()
        .map(|l| (l.path.display().to_string(), l.line, l.column))
        .collect();
    assert_eq!(
        found,
        [
            ("src/a.py".to_string(), 3, Some(7)),
            ("src/b.py".to_string(), 9, Some(1))
        ]
    );
}

#[test]
fn sublime_scopes_follow_sublime_default_syntaxes() {
    use lux_syntax::LanguageId;
    assert_eq!(select::sublime_scope(LanguageId::Rust), "source.rust");
    assert_eq!(select::sublime_scope(LanguageId::Python), "source.python");
    assert_eq!(select::sublime_scope(LanguageId::Cpp), "source.c++");
    assert_eq!(
        select::sublime_scope(LanguageId::Markdown),
        "text.html.markdown"
    );
}

#[test]
fn cmd_b_then_f4_jumps_to_the_error_relative_to_the_working_dir() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir(config.path().join("builds")).unwrap();
    std::fs::write(
        config.path().join("builds/Lint.sublime-build"),
        r#"{
            "shell_cmd": "echo checking; echo 'at main.py:2:5 boom'",
            "file_regex": "^at ([^:]+):([0-9]+):([0-9]+)",
            "selector": "source.python"
        }"#,
    )
    .unwrap();
    let file = work.path().join("main.py");
    std::fs::write(&file, "print('hi')\nprint('bye')\n").unwrap();
    let ctx = Context::new();
    let mut app = app_with(config.path(), &file, &ctx);
    app.buffer = Buffer::from_bytes(b"print('hi')\nprint('bye')\n").unwrap();
    load(&mut app, &ctx);

    assert!(app.build(None));
    wait_for_output(&app, "boom");
    assert!(app.next_build_error(Some(&ctx)));
    assert_eq!(app.ui.build.current_error_idx, Some(0));
    let error = &app.ui.build.detected_errors[0];
    assert_eq!((error.line, error.column), (2, Some(5)));
    // `main.py` resolved against the build's working dir: the open tab.
    assert_eq!(app.documents.path.as_deref(), Some(file.as_path()));
    let head = app.cursors.primary().head;
    assert_eq!((head.line, head.column), (1, 4));
}
