//! A running build's life after it starts: redraws when it prints, the
//! `[Finished in X.Xs]` / `[Cancelled]` line when its child exits, and
//! Cancel Build.
//!
//! The build's PTY reports output and the child's exit from its own
//! threads (`lux_term::PtyNotify`). They send a `BuildEvent` and wake the
//! event loop; `poll`, called from `background_event`, applies it. Nothing
//! here polls the child (CLAUDE.md rule 5).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use lux_term::{ExitStatus, PtyNotify, TaskId};

use crate::app::App;

type Wake = Arc<dyn Fn() + Send + Sync>;

pub(crate) enum BuildEvent {
    /// The build printed something since the last `poll`.
    Output,
    /// The build's child exited with `code`, and its output is in the grid.
    Exited { task: TaskId, code: u32 },
}

/// The channel the build's PTY threads report through.
pub(crate) struct BuildEvents {
    tx: Sender<BuildEvent>,
    rx: Receiver<BuildEvent>,
    /// Set when an `Output` event is queued and not yet applied, so a
    /// chatty build sends one wake per frame, not one per chunk.
    output_pending: Arc<AtomicBool>,
}

impl Default for BuildEvents {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            output_pending: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl std::fmt::Debug for BuildEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildEvents").finish_non_exhaustive()
    }
}

impl BuildEvents {
    /// Callbacks for a new build task: they send events and call `wake`.
    pub(crate) fn notify(&self, wake: Option<Wake>) -> impl FnOnce(TaskId) -> PtyNotify + use<> {
        let tx = self.tx.clone();
        let pending = Arc::clone(&self.output_pending);
        move |task| {
            let output_tx = tx.clone();
            let output_wake = wake.clone();
            PtyNotify {
                output: Some(Arc::new(move || {
                    if !pending.swap(true, Ordering::AcqRel)
                        && output_tx.send(BuildEvent::Output).is_ok()
                        && let Some(wake) = &output_wake
                    {
                        wake();
                    }
                })),
                exit: Some(Box::new(move |status: &ExitStatus| {
                    let code = status.exit_code();
                    if tx.send(BuildEvent::Exited { task, code }).is_ok()
                        && let Some(wake) = &wake
                    {
                        wake();
                    }
                })),
            }
        }
    }
}

/// Sublime's closing line for a build that ran to the end.
pub(crate) fn finished_line(elapsed: Duration, code: u32) -> String {
    let secs = elapsed.as_secs_f64();
    if code == 0 {
        format!("[Finished in {secs:.1}s]")
    } else {
        format!("[Finished in {secs:.1}s with exit code {code}]")
    }
}

pub(crate) const CANCELLED_LINE: &str = "[Cancelled]";

/// Applies what the build's PTY threads reported. Called after a wake.
pub(crate) fn poll(app: &mut App) {
    let mut changed = false;
    app.ui
        .build
        .events
        .output_pending
        .store(false, Ordering::Release);
    while let Ok(event) = app.ui.build.events.rx.try_recv() {
        changed = true;
        if let BuildEvent::Exited { task, code } = event {
            note_exit(app, task, code);
        }
    }
    if changed {
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
}

fn note_exit(app: &mut App, task: TaskId, code: u32) {
    let build = &mut app.ui.build;
    if build.task_id != Some(task) || !build.is_running {
        return;
    }
    let line = if build.cancelling {
        CANCELLED_LINE.to_string()
    } else {
        let elapsed = build.started.map(|s| s.elapsed()).unwrap_or_default();
        finished_line(elapsed, code)
    };
    build.is_running = false;
    build.cancelling = false;
    build.started = None;
    if let Some(pty) = app.ui.term.tasks.get(task) {
        pty.append_line(&line);
    }
    app.sync_build_errors();
    app.ui.announcement = Some(format!("Build {line}"));
}

impl App {
    /// Cancel Build: asks the build's child to stop and returns at once.
    /// The output stays; `[Cancelled]` is appended when the child is gone.
    pub(crate) fn cancel_build(&mut self) -> bool {
        let build = &mut self.ui.build;
        let Some(task) = build.task_id else {
            return false;
        };
        if !build.is_running || build.cancelling {
            return false;
        }
        let Some(pty) = self.ui.term.tasks.get_mut(task) else {
            return false;
        };
        pty.request_kill();
        build.cancelling = true;
        self.ui.announcement = Some("Cancelling build".into());
        self.scheduler.invalidate_all();
        true
    }

    /// Marks a build task as started now.
    pub(crate) fn note_build_started(&mut self, task: TaskId) {
        let build = &mut self.ui.build;
        build.task_id = Some(task);
        build.is_running = true;
        build.cancelling = false;
        build.started = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_closing_line_reads_like_sublime() {
        let t = Duration::from_millis(1234);
        assert_eq!(finished_line(t, 0), "[Finished in 1.2s]");
        assert_eq!(
            finished_line(t, 101),
            "[Finished in 1.2s with exit code 101]"
        );
    }
}
