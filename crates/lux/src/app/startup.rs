//! Work that used to run before the first presented frame.
//!
//! Coverage fonts, the UI face, and the recovery-session scan are not
//! needed to paint an ASCII frame. They start once, after that frame has
//! been logged, on workers: coverage faces are tens to hundreds of MB, so
//! reading them on the UI thread would stall input. The faces are installed
//! when the worker hands them back.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use lux_platform::PlatformContext;

use super::App;
use crate::ui_io::{self, Done};

impl App {
    pub(crate) fn register_wakers(&mut self, ctx: &dyn PlatformContext) {
        crate::menu::register_waker(ctx.background_waker());
        crate::cli::register_waker(ctx.background_waker());
        crate::git::register_waker(ctx.background_waker());
        self.ui.session.register_waker(ctx.background_waker());
        self.ui.dialogs.jobs.set_wake(ctx.background_waker());
    }

    pub(crate) fn after_first_frame(&mut self) {
        static DONE: AtomicBool = AtomicBool::new(false);
        if DONE.swap(true, Ordering::Relaxed) {
            return;
        }
        self.start_deferred_loads();
    }

    /// Starts the coverage-font read and the recovery scan on workers.
    pub(crate) fn start_deferred_loads(&mut self) {
        ui_io::spawn(self, None, || {
            let started = std::time::Instant::now();
            let faces = crate::font::read_coverage_faces();
            crate::font::warm_ui_font();
            eprintln!(
                "Lux: phase coverage {:.1} ms (worker)",
                started.elapsed().as_secs_f64() * 1000.0
            );
            Some(Done::Fonts(faces))
        });
        ui_io::spawn(self, None, || {
            Some(Done::Recovery(
                Self::recoverable_documents().into_iter().next(),
            ))
        });
    }
}

pub(crate) fn note_recovery(app: &mut App, first: Option<PathBuf>) {
    if app.documents.message.is_some() {
        return;
    }
    let Some(first) = first else {
        return;
    };
    app.documents.message = Some(format!(
        "Unsaved recovery session available: {}",
        first
            .file_name()
            .unwrap_or(first.as_os_str())
            .to_string_lossy()
    ));
}
