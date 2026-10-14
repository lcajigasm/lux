//! Folding operations and state management for App.

use lux_core::folding::{FoldRange, indent_folding_ranges};
use lux_core::position::Position;

use crate::app::App;

impl App {
    /// Recomputes available foldable ranges from syntax tree or indentation fallback,
    /// updating `self.fold_state` while preserving currently active folds.
    pub(crate) fn refresh_foldable_ranges(&mut self) {
        let syntax_ranges = self
            .syntax
            .as_ref()
            .and_then(|s| s.document.as_ref())
            .map(|doc| {
                lux_syntax::folding::folding_ranges(doc)
                    .into_iter()
                    .map(|r| FoldRange::new(r.start_row, r.end_row))
                    .collect::<Vec<_>>()
            });

        let ranges = match syntax_ranges {
            Some(ranges) if !ranges.is_empty() => ranges,
            _ => indent_folding_ranges(&self.buffer),
        };

        self.fold_state.set_foldable_ranges(ranges);
    }

    /// Folds the block at the primary cursor.
    pub(crate) fn fold_at_cursor(&mut self) {
        self.refresh_foldable_ranges();
        let cursor_line = self.cursors.primary().head.line;
        if self.fold_state.fold_cursor(cursor_line) {
            self.clamp_cursors_outside_folds();
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    /// Unfolds the block at the primary cursor.
    pub(crate) fn unfold_at_cursor(&mut self) {
        self.refresh_foldable_ranges();
        let cursor_line = self.cursors.primary().head.line;
        if self.fold_state.unfold_cursor(cursor_line) {
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    /// Folds all top-level blocks in the document.
    pub(crate) fn fold_all(&mut self) {
        self.refresh_foldable_ranges();
        self.fold_state.fold_all();
        self.clamp_cursors_outside_folds();
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    /// Unfolds all folded blocks in the file.
    pub(crate) fn unfold_all(&mut self) {
        if self.fold_state.unfold_all() {
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    /// Folds all blocks at the specified nesting level (1 = top-level, 2 = 1 nested, etc.).
    pub(crate) fn fold_by_level(&mut self, level: usize) {
        self.refresh_foldable_ranges();
        self.fold_state.fold_by_level(level);
        self.clamp_cursors_outside_folds();
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    /// Toggles the fold state of the block starting at `line`.
    pub(crate) fn toggle_fold_at_line(&mut self, line: usize) {
        self.refresh_foldable_ranges();
        if self.fold_state.toggle_fold(line) {
            self.clamp_cursors_outside_folds();
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    /// Unfolds the block starting at `line`.
    pub(crate) fn unfold_at_line(&mut self, line: usize) {
        if self.fold_state.unfold(line) {
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    /// Ensures any cursor currently situated inside a hidden line is moved
    /// to the header line of the active fold.
    fn clamp_cursors_outside_folds(&mut self) {
        let mut selections = Vec::new();
        for &sel in self.cursors.cursors() {
            let pos = sel.head;
            if self.fold_state.is_line_hidden(pos.line) {
                // Find the fold header that hides this line
                let mut header_line = pos.line;
                for (&start, range) in self.fold_state.folded_ranges() {
                    if range.hides_line(pos.line) {
                        header_line = start;
                        break;
                    }
                }
                let line_byte = self.buffer.line_start_byte(header_line).unwrap_or(0);
                let line_start_pos = Position::from_byte(&self.buffer, line_byte);
                let new_pos = lux_core::movement::line_end(&self.buffer, line_start_pos);
                selections.push(lux_core::selection::Selection::cursor(new_pos));
            } else {
                selections.push(sel);
            }
        }
        if !selections.is_empty() {
            self.cursors = lux_core::multicursor::MultiCursor::from_cursors(selections);
            self.reset_goal_columns();
        }
    }
}
