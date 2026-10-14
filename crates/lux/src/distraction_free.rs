//! Distraction Free Mode implementation matching Sublime Text 3.

use crate::app::App;

/// Visual and window state saved prior to entering distraction-free mode.
#[derive(Debug, Clone)]
pub(crate) struct DistractionFreeSnapshot {
    pub(crate) was_fullscreen: bool,
    pub(crate) was_sidebar_visible: bool,
    pub(crate) was_minimap_enabled: bool,
}

/// State active while in distraction-free mode.
#[derive(Debug, Clone)]
pub(crate) struct DistractionFreeState {
    pub(crate) snapshot: DistractionFreeSnapshot,
    pub(crate) column_width_chars: usize,
}

impl DistractionFreeState {
    pub(crate) fn new(snapshot: DistractionFreeSnapshot, column_width_chars: usize) -> Self {
        Self {
            snapshot,
            column_width_chars,
        }
    }
}

impl App {
    /// Whether the editor is currently in distraction-free mode.
    pub(crate) fn is_distraction_free(&self) -> bool {
        self.ui.distraction_free.is_some()
    }

    /// Column width in characters for the centered distraction-free document text.
    pub(crate) fn distraction_free_column_chars(&self) -> usize {
        if let Some(df) = &self.ui.distraction_free {
            return df.column_width_chars;
        }
        let config = self.ui.settings.config.lock().unwrap();
        let r = config.resolved();
        r["editor.distractionFreeColumnWidth"]
            .as_u64()
            .or_else(|| {
                r["editor.distractionFreeColumnWidth"]
                    .as_f64()
                    .map(|n| n as u64)
            })
            .map_or(80, |n| n as usize)
    }

    /// Horizontal offset (in physical pixels) where the editor text starts.
    ///
    /// In normal mode, this is after the sidebar and line-number gutter.
    /// In distraction-free mode, this is horizontally centered within the window.
    pub(crate) fn text_left(&mut self) -> f32 {
        if self.is_distraction_free() {
            let col_chars = self.distraction_free_column_chars();
            let char_w = self.editor_view.reference_width("0");
            let content_w = char_w * col_chars as f32;
            let window_w = self.viewport.width;
            ((window_w - content_w) / 2.0).max(24.0 * self.scale_factor)
        } else {
            let digits = self.buffer.len_lines().max(1).to_string().len();
            let gutter_w = self.editor_view.reference_width(&"0".repeat(digits));
            let gutter_total_w = gutter_w + 16.0 * self.scale_factor;
            self.sidebar_width() + gutter_total_w + 6.0 * self.scale_factor
        }
    }

    /// Toggles distraction-free mode.
    pub(crate) fn toggle_distraction_free(&mut self) {
        if self.is_distraction_free() {
            self.exit_distraction_free();
        } else {
            self.enter_distraction_free();
        }
    }

    /// Enters distraction-free mode:
    /// - Sets the window to borderless fullscreen.
    /// - Hides tab bar, status bar, sidebar, minimap, and gutter.
    /// - Centers text content in a fixed-width column.
    pub(crate) fn enter_distraction_free(&mut self) {
        if self.is_distraction_free() {
            return;
        }

        let was_fullscreen = self
            .gpu_window
            .as_ref()
            .is_some_and(|w| w.window.is_fullscreen());
        let was_sidebar_visible = self.ui.sidebar.as_ref().is_some_and(|s| s.visible);
        let was_minimap_enabled = self.ui.minimap.is_enabled();

        let snapshot = DistractionFreeSnapshot {
            was_fullscreen,
            was_sidebar_visible,
            was_minimap_enabled,
        };

        let col_chars = {
            let config = self.ui.settings.config.lock().unwrap();
            let r = config.resolved();
            r["editor.distractionFreeColumnWidth"]
                .as_u64()
                .or_else(|| {
                    r["editor.distractionFreeColumnWidth"]
                        .as_f64()
                        .map(|n| n as u64)
                })
                .map_or(80, |n| n as usize)
        };

        self.ui.distraction_free = Some(DistractionFreeState::new(snapshot, col_chars));

        if let Some(gpu_window) = &self.gpu_window {
            gpu_window.window.set_fullscreen(true);
        }

        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }

    /// Exits distraction-free mode, restoring the exact previous window and chrome state.
    pub(crate) fn exit_distraction_free(&mut self) {
        let Some(df) = self.ui.distraction_free.take() else {
            return;
        };

        if let Some(sidebar) = &mut self.ui.sidebar {
            sidebar.visible = df.snapshot.was_sidebar_visible;
        }

        if df.snapshot.was_minimap_enabled != self.ui.minimap.is_enabled() {
            self.ui.minimap.toggle();
        }

        if let Some(gpu_window) = &self.gpu_window
            && !df.snapshot.was_fullscreen
        {
            gpu_window.window.set_fullscreen(false);
        }

        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }
}
