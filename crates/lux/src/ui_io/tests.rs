//! Each path returns before its dialog or disk access runs, and its result
//! lands only after the worker finishes and `poll` applies it.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use lux_platform::dialog::SaveDecision;
use lux_platform::{
    Clipboard, FileDialogOptions, FileDialogs, PlatformContext, PlatformError, PlatformWindow,
    WindowAttributes,
};

use crate::app::App;
use crate::document::tests::app;

#[derive(Default)]
pub(super) struct Dialogs {
    file: Option<PathBuf>,
    folder: Option<PathBuf>,
    save: Option<PathBuf>,
    threads: Mutex<Vec<ThreadId>>,
}

impl Dialogs {
    fn shown(&self) -> Option<PathBuf> {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        None
    }
}

impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        self.shown().or_else(|| self.file.clone())
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        self.shown().or_else(|| self.folder.clone())
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        self.shown().or_else(|| self.save.clone())
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
    fn confirm_trash(&self, _: &str) -> bool {
        false
    }
}

pub(super) struct Context {
    dialogs: Arc<Dialogs>,
    events: Receiver<()>,
    sender: Sender<()>,
}

impl Context {
    pub(super) fn new(dialogs: Dialogs) -> Self {
        let (sender, events) = mpsc::channel();
        Self {
            dialogs: Arc::new(dialogs),
            events,
            sender,
        }
    }

    /// Waits for `count` worker wakes, then applies what they finished.
    pub(super) fn settle(&self, app: &mut App, count: usize) {
        for _ in 0..count {
            self.events
                .recv_timeout(Duration::from_secs(10))
                .expect("worker must wake the event loop");
        }
        super::poll(app, self);
    }

    fn dialog_threads(&self) -> Vec<ThreadId> {
        self.dialogs.threads.lock().unwrap().clone()
    }
}

impl Clipboard for Context {
    fn read_text(&self) -> Result<String, PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Err(PlatformError::Clipboard("unused".into()))
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
        self.dialogs.clone()
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless ui_io test".into()))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        panic!("the UI thread must not show a blocking dialog");
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {
        panic!("ui_io must not use timer polling");
    }
}

#[test]
fn open_folder_shows_the_panel_on_a_worker_and_loads_the_folder_after() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs {
        folder: Some(dir.path().to_path_buf()),
        ..Default::default()
    });
    let mut app = app();
    let hold = app.ui.dialogs.jobs.hold();
    app.open_folder_dialog(&ctx);
    assert!(ctx.dialog_threads().is_empty(), "panel shown before return");
    assert_ne!(app.workspace_root(), Some(dir.path().to_path_buf()));
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 1);

    hold.release();
    ctx.settle(&mut app, 1);
    assert_eq!(app.workspace_root(), Some(dir.path().to_path_buf()));
    assert_ne!(ctx.dialog_threads(), vec![std::thread::current().id()]);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 0);
}

#[test]
fn open_project_reads_the_file_on_a_worker() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("demo.sublime-project");
    std::fs::write(
        &project,
        r#"{"folders": [{"path": "."}], "settings": {"tab_size": 3}}"#,
    )
    .unwrap();
    let ctx = Context::new(Dialogs {
        file: Some(project.clone()),
        ..Default::default()
    });
    let mut app = app();
    let hold = app.ui.dialogs.jobs.hold();
    app.open_project_dialog(&ctx);
    assert!(ctx.dialog_threads().is_empty());
    assert!(app.ui.project.workspace.is_none());

    hold.release();
    ctx.settle(&mut app, 1);
    assert_eq!(app.ui.project.active_project_path, Some(project));
    assert!(app.ui.project.workspace.is_some());
    assert_eq!(app.workspace_root(), Some(dir.path().to_path_buf()));
}

#[test]
fn cancelled_open_project_changes_nothing() {
    let ctx = Context::new(Dialogs::default());
    let mut app = app();
    app.open_project_dialog(&ctx);
    ctx.settle(&mut app, 1);
    assert!(app.ui.project.workspace.is_none());
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 0);
}

#[test]
fn save_project_as_writes_on_a_worker() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("saved.sublime-project");
    let ctx = Context::new(Dialogs {
        save: Some(target.clone()),
        ..Default::default()
    });
    let mut app = app();
    let hold = app.ui.dialogs.jobs.hold();
    app.save_project_as_dialog(&ctx);
    assert!(!target.exists(), "written before the call returned");
    assert!(app.ui.project.active_project_path.is_none());

    hold.release();
    ctx.settle(&mut app, 1);
    assert!(
        std::fs::read_to_string(&target)
            .unwrap()
            .contains("folders")
    );
    assert_eq!(app.ui.project.active_project_path, Some(target));
}

#[test]
fn open_settings_json_creates_the_file_on_a_worker_then_opens_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let ctx = Context::new(Dialogs::default());
    let mut app = app();
    app.ui.settings = crate::settings::SettingsState::new(path.clone());
    std::fs::remove_file(&path).unwrap();
    let hold = app.ui.dialogs.jobs.hold();
    crate::settings::open_settings_json(&mut app, &ctx);
    assert!(!path.exists(), "created before the call returned");
    assert!(app.documents.path.is_none());

    hold.release();
    ctx.settle(&mut app, 1);
    assert!(path.exists());
    // The document worker reads the file and lands on the next wake.
    ctx.events.recv_timeout(Duration::from_secs(10)).unwrap();
    app.finish_document_operation(&ctx);
    assert_eq!(app.documents.path, Some(path));
    assert!(app.buffer.text().contains("editor.fontSize"));
}

#[test]
fn confirming_a_theme_writes_settings_on_a_worker_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let ctx = Context::new(Dialogs::default());
    let mut app = app();
    app.ui.settings = crate::settings::SettingsState::new(path.clone());
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    let before = std::fs::read_to_string(&path).unwrap();
    let hold = app.ui.dialogs.jobs.hold();
    app.confirm_theme("Monokai");
    app.confirm_theme("Mariana");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    let mariana = lux_ui::Theme::MARIANA.name;
    assert_eq!(app.chrome.theme.name, mariana, "the theme applies at once");

    hold.release();
    ctx.settle(&mut app, 2);
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        written.contains(r#""workbench.colorTheme": "Mariana""#),
        "{written}"
    );
    let resolved = app.ui.settings.config.lock().unwrap().resolved().clone();
    assert_eq!(resolved["workbench.colorTheme"], "Mariana");
    assert_eq!(app.chrome.theme.name, mariana);
}

/// Points the macro folders of this test thread at an empty temporary
/// folder, so no test reads or writes the real `~/.lux/macros`.
struct MacroFolder(tempfile::TempDir);

impl MacroFolder {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        crate::macros::recording::set_test_dir(Some(dir.path().to_path_buf()));
        Self(dir)
    }

    fn path(&self) -> &std::path::Path {
        self.0.path()
    }
}

impl Drop for MacroFolder {
    fn drop(&mut self) {
        crate::macros::recording::set_test_dir(None);
    }
}

fn recorded_app(text: &str) -> App {
    let mut app = app();
    app.ui.macros.start_recording();
    app.ui
        .macros
        .record_command(crate::macros::MacroCommand::InsertText { text: text.into() });
    app.ui.macros.stop_recording();
    app
}

#[test]
fn saving_a_macro_writes_on_a_worker() {
    let dir = MacroFolder::new();
    let ctx = Context::new(Dialogs::default());
    let mut app = recorded_app("x");
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    let hold = app.ui.dialogs.jobs.hold();
    crate::macros::dispatch::save_macro_as(&mut app, "demo");
    let file = dir.path().join("demo.sublime-macro");
    assert!(!file.exists(), "written before the call returned");
    assert!(app.ui.macros.saved_macros.is_empty());

    hold.release();
    ctx.settle(&mut app, 1);
    assert!(file.exists());
    assert!(app.ui.macros.saved("demo").is_some());
    assert_eq!(
        app.documents.message.as_deref(),
        Some("Saved macro to demo.sublime-macro")
    );
    // The save also re-reads the macro folders on a worker.
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 1, "reload after save");
    app.ui.macros.set_saved(Vec::new());
    ctx.settle(&mut app, 1);
    assert!(
        app.ui.macros.saved("demo").is_some(),
        "reload found the file"
    );
}

#[test]
fn saving_over_an_existing_macro_asks_first() {
    let dir = MacroFolder::new();
    let file = dir.path().join("demo.sublime-macro");
    std::fs::write(&file, "[]").unwrap();
    let ctx = Context::new(Dialogs::default());
    let mut app = recorded_app("new");
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());

    crate::macros::dispatch::save_macro_as(&mut app, "demo");
    ctx.settle(&mut app, 1);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "[]",
        "not replaced"
    );
    let palette = app.ui.command_palette.as_ref().expect("asks first");
    assert!(matches!(
        &palette.prompt,
        Some(crate::palette::PalettePrompt::ReplaceMacro { name, .. }) if name == "demo"
    ));
    assert_eq!(palette.query.as_str(), "demo");
    let message = app.documents.message.clone().unwrap_or_default();
    assert!(message.contains("already exists"), "{message}");

    // Enter on the same name replaces the file.
    let prompt = palette.prompt.clone().unwrap();
    assert!(prompt.submit(&mut app, "demo"));
    app.ui.command_palette = None;
    ctx.settle(&mut app, 1);
    assert!(std::fs::read_to_string(&file).unwrap().contains("new"));
    assert!(app.ui.command_palette.is_none());
}

#[test]
fn saved_macros_are_read_on_a_worker_at_startup() {
    let dir = MacroFolder::new();
    crate::macros::Macro::from_commands(vec![crate::macros::MacroCommand::InsertNewline])
        .save_to_file(&dir.path().join("boot.sublime-macro"))
        .unwrap();
    let ctx = Context::new(Dialogs::default());
    let mut app = app();
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    let hold = app.ui.dialogs.jobs.hold();
    crate::macros::library::start_loading(&mut app, None);
    assert_eq!(
        app.ui.dialogs.jobs.in_flight(),
        1,
        "macro folders read inline"
    );
    assert!(app.ui.macros.saved("boot").is_none());
    hold.release();
    ctx.settle(&mut app, 1);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 0);
    assert!(app.ui.macros.saved("boot").is_some());
    assert_eq!(app.ui.macros.saved_macros.len(), 1, "only the test folder");
}

#[test]
fn a_slow_macro_load_does_not_overwrite_a_newer_one() {
    let _dir = MacroFolder::new();
    let ctx = Context::new(Dialogs::default());
    let mut app = app();
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    let hold = app.ui.dialogs.jobs.hold();
    // Startup load, then the reload after a save.
    crate::macros::library::start_loading(&mut app, None);
    crate::macros::library::start_loading(&mut app, None);
    let named = |name: &str| {
        let mut m = crate::macros::Macro::from_commands(Vec::new());
        m.name = Some(name.into());
        vec![m]
    };
    // The newer load lands first, then the stale startup one.
    super::apply(
        &mut app,
        &ctx,
        super::Done::MacrosLoaded {
            generation: 2,
            macros: named("fresh"),
        },
    );
    super::apply(
        &mut app,
        &ctx,
        super::Done::MacrosLoaded {
            generation: 1,
            macros: named("stale"),
        },
    );
    assert!(app.ui.macros.saved("fresh").is_some());
    assert!(app.ui.macros.saved("stale").is_none(), "stale load dropped");
    hold.release();
    ctx.settle(&mut app, 2);
}

#[test]
fn coverage_fonts_and_recovery_scan_load_after_the_call_returns() {
    let _macros = MacroFolder::new();
    let ctx = Context::new(Dialogs::default());
    let mut app = app();
    let config = tempfile::tempdir().unwrap();
    app.ui.settings.settings_path = config.path().join("settings.json");
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    let hold = app.ui.dialogs.jobs.hold();
    app.start_deferred_loads();
    // Fonts, recovery scan, saved macros and `.sublime-build` files.
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 5, "reads ran inline");
    assert!(
        !config.path().join("builds").exists(),
        "no disk access inline"
    );

    hold.release();
    ctx.settle(&mut app, 5);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 0);
}

#[test]
fn a_new_editor_font_is_read_on_a_worker_and_swapped_in_after() {
    use lux_config::layering::{Layer, LayeredConfig};

    let ctx = Context::new(Dialogs::default());
    let mut app = app();
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.json");
    app.ui.settings = crate::settings::SettingsState::new(path.clone());
    let startup = app.editor_view.family().to_string();
    let startup_key = app.font.key.value();
    let set_family = |app: &mut App, json: &str| {
        std::fs::write(&path, json).unwrap();
        LayeredConfig::reload_layer_shared(&app.ui.settings.config, Layer::User).unwrap();
        app.apply_user_settings();
    };

    let hold = app.ui.dialogs.jobs.hold();
    set_family(&mut app, r#"{"editor.fontFamily": "JetBrains Mono"}"#);
    // The worker is parked before it reads anything; the UI thread returned.
    assert_eq!(app.editor_view.family(), startup);
    assert_eq!(app.font.key.value(), startup_key);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 1);
    // Applying again while the read runs does not start a second one.
    app.apply_user_settings();
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 1);

    hold.release();
    ctx.settle(&mut app, 1);
    assert_eq!(app.editor_view.family(), "JetBrains Mono");
    assert_eq!(app.atlas_state.fonts.applied(), "JetBrains Mono");
    assert_ne!(app.font.key.value(), startup_key);
    assert_ne!(app.font.charmap().map('a'), 0);

    // Back to the startup face: already registered, no worker.
    set_family(&mut app, r#"{"editor.fontFamily": "system"}"#);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 0);
    assert_eq!(app.editor_view.family(), startup);
    assert_eq!(app.font.key.value(), startup_key);

    // An unknown family keeps the system face and reads nothing.
    set_family(&mut app, r#"{"editor.fontFamily": "No Such Mono"}"#);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 0);
    assert_eq!(app.editor_view.family(), startup);
}
