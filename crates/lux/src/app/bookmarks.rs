//! Bookmark operations and state management for App.

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;

use crate::app::App;

impl App {
    /// Toggles a bookmark at the line of each active cursor.
    pub(crate) fn toggle_bookmark(&mut self) {
        self.bookmarks.toggle_at_cursors(&self.cursors);
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    /// Toggles a bookmark at `line`.
    pub(crate) fn toggle_bookmark_at_line(&mut self, line: usize) {
        self.bookmarks.toggle(line);
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    /// Navigates to the next bookmark in the buffer, cycling to the beginning if needed.
    pub(crate) fn next_bookmark(&mut self) {
        let cur_line = self.cursors.primary().head.line;
        if let Some(target_line) = self.bookmarks.next_bookmark(cur_line) {
            self.jump_to_bookmarked_line(target_line);
        }
    }

    /// Navigates to the previous bookmark in the buffer, cycling to the end if needed.
    pub(crate) fn prev_bookmark(&mut self) {
        let cur_line = self.cursors.primary().head.line;
        if let Some(target_line) = self.bookmarks.prev_bookmark(cur_line) {
            self.jump_to_bookmarked_line(target_line);
        }
    }

    /// Clears all bookmarks in the current document.
    pub(crate) fn clear_bookmarks(&mut self) {
        if self.bookmarks.clear() {
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    /// Places a cursor on every bookmarked line in the buffer.
    pub(crate) fn select_all_bookmarks(&mut self) {
        if let Some(mc) = self.bookmarks.select_all(&self.buffer) {
            self.cursors = mc;
            self.reset_goal_columns();
            self.reveal_primary_cursor();
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }

    fn jump_to_bookmarked_line(&mut self, target_line: usize) {
        if self.fold_state.is_line_hidden(target_line) {
            self.fold_state.unfold_cursor(target_line);
        }

        let line_byte = self.buffer.line_start_byte(target_line).unwrap_or(0);
        let pos = Position::from_byte(&self.buffer, line_byte);
        self.cursors = MultiCursor::new(Selection::cursor(pos));
        self.reset_goal_columns();
        self.reveal_primary_cursor();
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }
}
