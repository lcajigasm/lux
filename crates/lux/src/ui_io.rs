//! Native dialogs and small file reads and writes that the UI asks for.
//!
//! Each job runs on a worker thread. Its result goes into a channel owned by
//! the app, the worker wakes the event loop, and `poll` applies the result
//! on the UI thread from `background_event`. Nothing here blocks the UI
//! thread on a dialog or on the disk (CLAUDE.md rule 6).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

use lux_platform::PlatformContext;

use crate::app::App;

mod latest;

type Wake = Arc<dyn Fn() + Send + Sync>;
type Task = Box<dyn FnOnce() + Send>;

/// A finished job, applied on the UI thread.
pub(crate) enum Done {
    /// Open Folder picked a directory.
    Folder(PathBuf),
    /// Open Project picked and parsed a project file.
    /// `generation` is `ProjectState::generation` when the job started.
    ProjectOpened {
        path: PathBuf,
        workspace: lux_workspace::project::Workspace,
        generation: u64,
    },
    ProjectFailed {
        path: PathBuf,
        generation: u64,
    },
    /// A conflict-bar action finished its disk read or write.
    Conflict(crate::conflict::Resolved),
    /// Recent projects for the Quick Switch Project picker.
    ProjectList {
        projects: Vec<PathBuf>,
        generation: u64,
    },
    /// Save Project As wrote the project file.
    ProjectSaved(PathBuf),
    ProjectSaveFailed(PathBuf),
    /// A file is ready to open on the document worker.
    Open(PathBuf),
    /// The user settings file was written and its layer reloaded.
    SettingsSaved,
    /// A settings write left the file alone; the reason to show.
    SettingsFailed(String),
    /// An import from another editor finished; the report to show. Any
    /// imported values are already written and reloaded.
    Imported(String),
    MacroSaved {
        path: PathBuf,
        saved: crate::macros::Macro,
    },
    MacroFailed(String),
    /// Save Macro found a file of that name and did not replace it.
    MacroExists {
        name: String,
        dir: Option<PathBuf>,
    },
    /// Saved macros read from disk by the load numbered `generation`.
    MacrosLoaded {
        generation: u64,
        macros: Vec<crate::macros::Macro>,
    },
    /// The UI face was read from disk and can be registered.
    UiFont,
    /// An editor face read for the `editor.fontFamily` name it carries;
    /// `None` when not even the system face could be read.
    EditorFont(&'static str, Option<crate::font::editor::LoadedFont>),
    /// Coverage font faces read from disk.
    Fonts(Vec<crate::font::CoverageFace>),
    /// Faces the system picked for characters nothing else covers.
    FallbackFaces(Vec<crate::font::fallback::FallbackFace>),
    /// First document with an unsaved recovery snapshot, if any.
    Recovery(Option<PathBuf>),
    /// A Goto Anything preview read finished.
    Preview(Box<crate::goto::preview::Loaded>),
    /// `.sublime-build` files read from disk.
    BuildSystems(Box<crate::build::systems::Loaded>),
    /// Build marker files probed for `Cmd+B` / Build With.
    BuildDetected(Box<crate::build::request::Detected>),
}

/// Blocks workers while a test holds it, so a test can observe the UI
/// state between "the call returned" and "the worker ran".
#[derive(Default)]
struct Gate {
    closed: Mutex<bool>,
    opened: Condvar,
}

impl Gate {
    fn wait(&self) {
        let Ok(mut closed) = self.closed.lock() else {
            return;
        };
        while *closed {
            closed = match self.opened.wait(closed) {
                Ok(guard) => guard,
                Err(_) => return,
            };
        }
    }
}

pub(crate) struct Jobs {
    sender: Sender<Option<Done>>,
    receiver: Receiver<Option<Done>>,
    /// Shared with every job, so a job started before the event loop
    /// registered its waker (a project file on the command line) still
    /// wakes it.
    wake: Arc<Mutex<Option<Wake>>>,
    /// Settings writes are read-modify-write; one thread keeps them in order.
    serial: Option<Sender<Task>>,
    /// Goto Anything previews: only the newest request is read.
    latest: latest::Latest,
    in_flight: usize,
    gate: Arc<Gate>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            wake: Arc::default(),
            serial: None,
            latest: latest::Latest::default(),
            in_flight: 0,
            gate: Arc::new(Gate::default()),
        }
    }
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs")
            .field("in_flight", &self.in_flight)
            .finish_non_exhaustive()
    }
}

impl Jobs {
    /// Registers the event loop's waker. The first registration wakes the
    /// loop once when jobs are in flight, so results that landed before
    /// there was a waker are applied.
    pub(crate) fn set_wake(&mut self, wake: Wake) {
        let had = match self.wake.lock() {
            Ok(mut slot) => slot.replace(Arc::clone(&wake)).is_some(),
            Err(_) => true,
        };
        if !had && self.in_flight > 0 {
            wake();
        }
    }

    /// The event-loop waker registered at startup, for other workers.
    pub(crate) fn waker(&self) -> Option<Wake> {
        self.wake.lock().ok().and_then(|slot| slot.clone())
    }

    /// Jobs started and not yet applied by `poll`.
    #[cfg(test)]
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight
    }

    fn task(&self, work: impl FnOnce() -> Option<Done> + Send + 'static) -> Task {
        let sender = self.sender.clone();
        let wake = self.wake.clone();
        let gate = self.gate.clone();
        Box::new(move || {
            gate.wait();
            let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or(None);
            let wake = wake.lock().ok().and_then(|slot| slot.clone());
            if sender.send(done).is_ok()
                && let Some(wake) = wake
            {
                wake();
            }
        })
    }

    #[cfg(test)]
    pub(crate) fn hold(&self) -> Hold {
        if let Ok(mut closed) = self.gate.closed.lock() {
            *closed = true;
        }
        Hold(self.gate.clone())
    }
}

#[cfg(test)]
pub(crate) struct Hold(Arc<Gate>);

#[cfg(test)]
impl Hold {
    pub(crate) fn release(self) {
        if let Ok(mut closed) = self.0.closed.lock() {
            *closed = false;
        }
        self.0.opened.notify_all();
    }
}

fn note_wake(app: &mut App, ctx: Option<&dyn PlatformContext>) {
    if let Some(ctx) = ctx {
        app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    }
}

/// Runs `work` on its own thread. Dialog jobs (Open Folder, Open Project,
/// Save Project As) use this, so a sheet left open blocks nothing else.
pub(crate) fn spawn(
    app: &mut App,
    ctx: Option<&dyn PlatformContext>,
    work: impl FnOnce() -> Option<Done> + Send + 'static,
) {
    note_wake(app, ctx);
    let task = app.ui.dialogs.jobs.task(work);
    let spawned = std::thread::Builder::new()
        .name("lux-ui-io".into())
        .spawn(task);
    match spawned {
        Ok(_) => app.ui.dialogs.jobs.in_flight += 1,
        Err(error) => {
            app.documents.message = Some(format!("Cannot start file operation: {error}"));
        }
    }
}

/// Runs `work` after every earlier serial job has finished. Never for a
/// dialog: a sheet waits on the user and would hold every later job (font
/// loads, settings writes) behind it. Dialogs go through [`spawn`], one
/// thread each.
pub(crate) fn spawn_serial(
    app: &mut App,
    ctx: Option<&dyn PlatformContext>,
    work: impl FnOnce() -> Option<Done> + Send + 'static,
) {
    note_wake(app, ctx);
    let task = app.ui.dialogs.jobs.task(work);
    let jobs = &mut app.ui.dialogs.jobs;
    if jobs.serial.is_none() {
        let (tx, rx) = mpsc::channel::<Task>();
        let spawned = std::thread::Builder::new()
            .name("lux-ui-io-serial".into())
            .spawn(move || {
                for task in rx {
                    task();
                }
            });
        if let Err(error) = spawned {
            app.documents.message = Some(format!("Cannot start file operation: {error}"));
            return;
        }
        jobs.serial = Some(tx);
    }
    if jobs.serial.as_ref().is_some_and(|tx| tx.send(task).is_ok()) {
        jobs.in_flight += 1;
    } else {
        jobs.serial = None;
        app.documents.message = Some("Cannot start file operation".into());
    }
}

/// Runs `work` on the one latest-wins worker. A job still waiting there
/// when the next one arrives is dropped without running, so a burst of
/// requests costs one thread and one run, not one of each per request.
pub(crate) fn spawn_latest(
    app: &mut App,
    ctx: Option<&dyn PlatformContext>,
    work: impl FnOnce() -> Option<Done> + Send + 'static,
) {
    note_wake(app, ctx);
    let jobs = &mut app.ui.dialogs.jobs;
    let task = jobs.task(work);
    match jobs.latest.post(task, &jobs.gate) {
        Ok(true) => jobs.in_flight += 1,
        Ok(false) => {}
        Err(error) => {
            app.documents.message = Some(format!("Cannot start file operation: {error}"));
        }
    }
}

/// Drops the latest-wins job that has not started yet, if any.
pub(crate) fn cancel_latest(app: &mut App) {
    let jobs = &mut app.ui.dialogs.jobs;
    if jobs.latest.clear() {
        jobs.in_flight = jobs.in_flight.saturating_sub(1);
    }
}

/// Waits for every job in flight and applies it, for tests that need a
/// worker's result before they go on.
#[cfg(test)]
pub(crate) fn finish_all(app: &mut App, ctx: &dyn PlatformContext) {
    while app.ui.dialogs.jobs.in_flight > 0 {
        let done = app
            .ui
            .dialogs
            .jobs
            .receiver
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("a worker must finish");
        app.ui.dialogs.jobs.in_flight -= 1;
        if let Some(done) = done {
            apply(app, ctx, done);
        }
    }
    app.scheduler.invalidate_all();
}

/// Applies every finished job. Called when a worker wakes the event loop.
pub(crate) fn poll(app: &mut App, ctx: &dyn PlatformContext) {
    let mut applied = false;
    while let Ok(done) = app.ui.dialogs.jobs.receiver.try_recv() {
        app.ui.dialogs.jobs.in_flight = app.ui.dialogs.jobs.in_flight.saturating_sub(1);
        applied = true;
        if let Some(done) = done {
            apply(app, ctx, done);
        }
    }
    if applied {
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
}

fn apply(app: &mut App, ctx: &dyn PlatformContext, done: Done) {
    match done {
        Done::Folder(path) => app.open_workspace(path),
        Done::ProjectOpened {
            path,
            workspace,
            generation,
        } => app.project_landed(&path, generation, Some(workspace)),
        Done::ProjectFailed { path, generation } => app.project_landed(&path, generation, None),
        Done::ProjectList {
            projects,
            generation,
        } => app.show_project_list(projects, generation),
        Done::Conflict(resolved) => crate::conflict::apply_resolved(app, resolved),
        Done::ProjectSaved(path) => app.note_project_saved(&path),
        Done::ProjectSaveFailed(path) => {
            app.ui.announcement = Some(format!("Failed to save project to {}", path.display()));
        }
        Done::Open(path) => crate::document::open_queue::enqueue(app, ctx, path),
        Done::SettingsSaved => app.apply_user_settings(),
        Done::SettingsFailed(error) => {
            app.documents.message = Some(format!("Settings not saved: {error}"));
        }
        Done::Imported(report) => {
            app.apply_user_settings();
            app.ui.announcement = Some(report);
        }
        Done::MacroSaved { path, saved } => {
            crate::macros::dispatch::note_macro_saved(app, &path, saved)
        }
        Done::MacroFailed(error) => {
            app.documents.message = Some(format!("Failed to save macro: {error}"));
        }
        Done::MacroExists { name, dir } => {
            crate::macros::dispatch::note_macro_exists(app, name, dir);
        }
        Done::MacrosLoaded { generation, macros } => {
            app.ui.macros.finish_load(generation, macros);
        }
        Done::Fonts(faces) => {
            let layout = app.editor_view.text_layout_mut();
            crate::font::install_coverage_faces(layout, &faces);
        }
        Done::FallbackFaces(faces) => {
            crate::font::fallback::install(app.editor_view.text_layout_mut(), faces);
        }
        Done::UiFont => crate::font::install_ui_font(app.editor_view.text_layout_mut()),
        Done::EditorFont(requested, loaded) => app.install_editor_font(requested, loaded),
        Done::BuildSystems(loaded) => crate::build::systems::apply_loaded(app, *loaded),
        Done::BuildDetected(detected) => crate::build::request::finish(app, ctx, *detected),
        Done::Recovery(first) => crate::app::startup::note_recovery(app, first),
        Done::Preview(loaded) => crate::goto::preview::apply(app, *loaded),
    }
}

#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod tests;
