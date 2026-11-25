//! Keeps the gutter and the tree's git state current while the user works.
//!
//! Two long-lived workers, each fed by a channel and blocked on it while
//! idle — nothing here wakes on a clock to look at the outside world:
//!
//! - The gutter worker takes a copy of the edited rope (shared structure,
//!   no text copy on the UI thread) and waits until edits pause for
//!   [`GUTTER_DEBOUNCE`] (or [`GUTTER_MAX_WAIT`] passes while they keep
//!   coming), then diffs only the newest copy against `HEAD`.
//! - The status worker runs on file-watcher batches, saves and git
//!   operations. Requests that arrive while a walk runs collapse into one
//!   more walk; the watcher has already debounced the filesystem side.
//!
//! Each worker runs one job at a time, so at most one gutter diff and one
//! status walk are ever in flight.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use lux_core::buffer::Buffer;

use super::job::{self, Event};
use super::snapshot;
use crate::app::App;

/// Quiet time after the last edit before the gutter is recomputed.
pub(crate) const GUTTER_DEBOUNCE: Duration = Duration::from_millis(250);
/// Longest a stream of edits can hold the gutter back.
const GUTTER_MAX_WAIT: Duration = Duration::from_secs(1);

struct GutterJob {
    generation: u64,
    path: PathBuf,
    buffer: Buffer,
}

struct StatusJob {
    epoch: u64,
    start: PathBuf,
}

fn gutter_queue() -> &'static Mutex<Sender<GutterJob>> {
    static QUEUE: OnceLock<Mutex<Sender<GutterJob>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(start_worker("lux-git-gutter", gutter_worker)))
}

fn status_queue() -> &'static Mutex<Sender<StatusJob>> {
    static QUEUE: OnceLock<Mutex<Sender<StatusJob>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(start_worker("lux-git-status", status_worker)))
}

fn start_worker<T: Send + 'static>(name: &str, run: fn(Receiver<T>)) -> Sender<T> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name(name.into())
        .spawn(move || run(rx));
    tx
}

fn gutter_worker(rx: Receiver<GutterJob>) {
    while let Ok(mut job) = rx.recv() {
        let first = Instant::now();
        loop {
            let now = Instant::now();
            let deadline = (now + GUTTER_DEBOUNCE).min(first + GUTTER_MAX_WAIT);
            match rx.recv_timeout(deadline.saturating_duration_since(now)) {
                Ok(newer) => job = newer,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        let text = job.buffer.text();
        let shot = std::panic::catch_unwind(|| snapshot::file_shot(Some(&job.path), &text));
        if let Ok(shot) = shot {
            job::deliver(Event::File {
                generation: job.generation,
                shot,
            });
        }
    }
}

fn status_worker(rx: Receiver<StatusJob>) {
    while let Ok(mut job) = rx.recv() {
        while let Ok(newer) = rx.try_recv() {
            job = newer;
        }
        let shot = std::panic::catch_unwind(|| snapshot::status_of(&job.start));
        if let Ok(Ok(shot)) = shot {
            job::deliver(Event::Status {
                epoch: job.epoch,
                shot,
            });
        }
    }
}

/// Called after input. Queues a gutter diff when the active document's
/// content changed since the last request. The comparison is two integers.
pub(crate) fn note_edit(app: &mut App) {
    let key = (app.documents.active_tab, app.history.state_id());
    if app.ui.git.edit_key == Some(key) {
        return;
    }
    app.ui.git.edit_key = Some(key);
    let (Some(path), Some(root)) = (&app.documents.path, &app.ui.git.repo_root) else {
        return;
    };
    // A huge file is diffed once when it opens; re-diffing it per pause
    // would keep a worker busy for the whole editing session.
    if !path.starts_with(root) || app.buffer.is_huge_file_mode() {
        return;
    }
    let path = path.clone();
    let job = GutterJob {
        generation: job::next_generation(app),
        path,
        buffer: app.buffer.clone(),
    };
    if let Ok(queue) = gutter_queue().lock() {
        let _ = queue.send(job);
    }
}

/// Called after input, beside [`note_edit`]. Reads the tree's git state
/// once when a folder opens or the open folder changes; after that,
/// watcher batches, saves and git operations keep it current. The
/// comparison is one path and one integer.
pub(crate) fn note_folder(app: &mut App) {
    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.workspace_root.clone());
    let Some(root) = root else {
        app.ui.git.status_for = None;
        return;
    };
    let key = (root, app.ui.git.folder_epoch);
    if app.ui.git.status_for.as_ref() == Some(&key) {
        return;
    }
    app.ui.git.status_for = Some(key);
    refresh_status(app);
}

/// Re-reads the tree's git state without announcing anything. Does
/// nothing when no folder or repository file is open.
pub(crate) fn refresh_status(app: &mut App) {
    let start = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.workspace_root.clone())
        .or_else(|| app.ui.git.repo_root.clone());
    let Some(start) = start else {
        return;
    };
    let job = StatusJob {
        epoch: app.ui.git.folder_epoch,
        start,
    };
    if let Ok(queue) = status_queue().lock() {
        let _ = queue.send(job);
    }
}

/// A watcher batch touched these paths. Paths inside an ignored directory
/// (build output, mostly) cannot change what the tree shows.
pub(crate) fn note_disk_changes<'a>(app: &mut App, paths: impl Iterator<Item = &'a PathBuf>) {
    if app.ui.sidebar.is_none() {
        return;
    }
    let ignored = |path: &PathBuf| {
        let Some(root) = &app.ui.git.repo_root else {
            return false;
        };
        let Ok(rel) = path.strip_prefix(root) else {
            return false;
        };
        app.ui.git.ignored.iter().any(|dir| rel.starts_with(dir))
    };
    let mut relevant = false;
    for path in paths {
        if !ignored(path) {
            relevant = true;
            break;
        }
    }
    if relevant {
        refresh_status(app);
    }
}
