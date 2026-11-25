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

type Wake = Arc<dyn Fn() + Send + Sync>;
type Task = Box<dyn FnOnce() + Send>;

/// A finished job, applied on the UI thread.
pub(crate) enum Done {
    /// Open Folder picked a directory.
    Folder(PathBuf),
    /// Open Project picked and parsed a project file.
    ProjectOpened {
        path: PathBuf,
        workspace: lux_workspace::project::Workspace,
    },
    ProjectFailed(PathBuf),
    /// Save Project As wrote the project file.
    ProjectSaved(PathBuf),
    ProjectSaveFailed(PathBuf),
    /// A file is ready to open on the document worker.
    Open(PathBuf),
    /// The user settings file was written and its layer reloaded.
    SettingsSaved,
    MacroSaved {
        path: PathBuf,
        saved: crate::macros::Macro,
    },
    MacroFailed(String),
    /// The UI face was read from disk and can be registered.
    UiFont,
    /// Coverage font faces read from disk.
    Fonts(Vec<crate::font::CoverageFace>),
    /// First document with an unsaved recovery snapshot, if any.
    Recovery(Option<PathBuf>),
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
    wake: Option<Wake>,
    /// Settings writes are read-modify-write; one thread keeps them in order.
    serial: Option<Sender<Task>>,
    in_flight: usize,
    gate: Arc<Gate>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            wake: None,
            serial: None,
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
    pub(crate) fn set_wake(&mut self, wake: Wake) {
        self.wake = Some(wake);
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

/// Runs `work` on its own thread.
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

/// Runs `work` after every earlier serial job has finished.
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
        Done::ProjectOpened { path, workspace } => app.apply_project(&path, workspace),
        Done::ProjectFailed(path) => {
            app.ui.announcement = Some(format!("Failed to open project: {}", path.display()));
        }
        Done::ProjectSaved(path) => app.note_project_saved(&path),
        Done::ProjectSaveFailed(path) => {
            app.ui.announcement = Some(format!("Failed to save project to {}", path.display()));
        }
        Done::Open(path) => crate::document::open_queue::enqueue(app, ctx, path),
        Done::SettingsSaved => app.apply_user_settings(),
        Done::MacroSaved { path, saved } => {
            crate::macros::dispatch::note_macro_saved(app, &path, saved)
        }
        Done::MacroFailed(error) => {
            app.documents.message = Some(format!("Failed to save macro: {error}"));
        }
        Done::Fonts(faces) => {
            let layout = app.editor_view.text_layout_mut();
            crate::font::install_coverage_faces(layout, &faces);
        }
        Done::UiFont => crate::font::install_ui_font(app.editor_view.text_layout_mut()),
        Done::Recovery(first) => crate::app::startup::note_recovery(app, first),
    }
}

#[cfg(test)]
mod tests;
