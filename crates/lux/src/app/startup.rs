//! Work that used to run before the first presented frame.
//!
//! Coverage fonts, the UI face, and the recovery-session scan are not
//! needed to paint an ASCII frame. They run once, after that frame has
//! been logged, so the cold-start clock does not include them.

use std::sync::atomic::{AtomicBool, Ordering};

use super::App;

impl App {
    pub(crate) fn after_first_frame(&mut self) {
        static DONE: AtomicBool = AtomicBool::new(false);
        if DONE.swap(true, Ordering::Relaxed) {
            return;
        }
        let started = std::time::Instant::now();
        crate::font::install_coverage_fallbacks(self.editor_view.text_layout_mut());
        crate::font::install_ui_font(self.editor_view.text_layout_mut());
        eprintln!(
            "Lux: phase coverage {:.1} ms",
            started.elapsed().as_secs_f64() * 1000.0
        );
        if self.documents.message.is_none()
            && let Some(first) = Self::recoverable_documents().into_iter().next()
        {
            self.documents.message = Some(format!(
                "Unsaved recovery session available: {}",
                first
                    .file_name()
                    .unwrap_or(first.as_os_str())
                    .to_string_lossy()
            ));
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }
}
