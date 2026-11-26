//! Caret motion by visual row under soft word wrap: Up/Down step between
//! the rows on screen and Home/End stop at a row's edges first, as in
//! Sublime Text. Only the lines a caret touches are shaped, with the
//! same wrap width the frame uses, so motion never waits for a redraw
//! and never wraps the whole document.

use lux_core::movement;
use lux_core::position::Position;

use crate::app::App;

/// Soft-wrap rows of one buffer line, as grapheme columns.
struct Rows {
    starts: Vec<usize>,
    len: usize,
}

impl Rows {
    fn row_of(&self, column: usize) -> usize {
        self.starts
            .partition_point(|&start| start <= column)
            .saturating_sub(1)
    }

    fn last(&self) -> usize {
        self.starts.len() - 1
    }

    /// First column of `row` and the last column a caret can take while
    /// still being drawn on it. The column where the next row starts is
    /// drawn at the start of that row, so it belongs to the next row.
    fn span(&self, row: usize) -> (usize, usize) {
        let start = self.starts[row];
        let end = self
            .starts
            .get(row + 1)
            .map_or(self.len, |next| next.saturating_sub(1).max(start));
        (start, end)
    }
}

impl App {
    fn wraps(&self) -> bool {
        self.editor_view.metrics().word_wrap
    }

    fn rows(&mut self, line: usize) -> Rows {
        let len = movement::position_at_line_column(&self.buffer, line, usize::MAX).column;
        if self.fold_state.is_line_folded(line) {
            return Rows {
                starts: vec![0],
                len,
            };
        }
        let line_start = self.buffer.line_start_byte(line).unwrap_or(0);
        let mut starts: Vec<usize> = self
            .editor_view
            .wrap_row_starts(&self.buffer, line, self.viewport.width)
            .into_iter()
            .map(|byte| Position::from_byte(&self.buffer, line_start + byte).column)
            .collect();
        starts.dedup();
        Rows { starts, len }
    }

    /// Where Up/Down moves a caret at `pos`, and the goal column to keep
    /// for the next press. Under wrap the goal is a line column whose
    /// offset from the caret's row start is the row-relative goal.
    pub(super) fn vertical_target(
        &mut self,
        pos: Position,
        goal: usize,
        down: bool,
    ) -> (Position, usize) {
        if !self.wraps() {
            return (self.buffer_line_target(pos, goal, down), goal);
        }
        let rows = self.rows(pos.line);
        let row = rows.row_of(pos.column);
        let offset = goal.saturating_sub(rows.starts[row]);
        let (line, rows, row) = if down && row < rows.last() {
            (pos.line, rows, row + 1)
        } else if !down && row > 0 {
            (pos.line, rows, row - 1)
        } else if down {
            let next = self
                .fold_state
                .next_visible_line(pos.line, self.buffer.len_lines());
            if next <= pos.line {
                return (pos, goal);
            }
            (next, self.rows(next), 0)
        } else {
            if pos.line == 0 {
                return (pos, goal);
            }
            let prev = self.fold_state.prev_visible_line(pos.line);
            let rows = self.rows(prev);
            let last = rows.last();
            (prev, rows, last)
        };
        let (start, end) = rows.span(row);
        let column = (start + offset).min(end);
        let target = movement::position_at_line_column(&self.buffer, line, column);
        (target, start + offset)
    }

    fn buffer_line_target(&self, pos: Position, goal: usize, down: bool) -> Position {
        if down {
            let target = movement::move_line_down(&self.buffer, pos, goal);
            if self.fold_state.is_line_hidden(target.line) {
                let visible = self
                    .fold_state
                    .next_visible_line(pos.line, self.buffer.len_lines());
                movement::position_at_line_column(&self.buffer, visible, goal)
            } else {
                target
            }
        } else {
            let target = movement::move_line_up(&self.buffer, pos, goal);
            if self.fold_state.is_line_hidden(target.line) {
                let visible = self.fold_state.prev_visible_line(pos.line);
                movement::position_at_line_column(&self.buffer, visible, goal)
            } else {
                target
            }
        }
    }

    /// Home: the start of a wrapped continuation row first, then the
    /// line's smart start.
    pub(super) fn row_start_target(&mut self, pos: Position) -> Position {
        if self.wraps() {
            let rows = self.rows(pos.line);
            let row = rows.row_of(pos.column);
            if row > 0 && pos.column != rows.starts[row] {
                return movement::position_at_line_column(&self.buffer, pos.line, rows.starts[row]);
            }
        }
        movement::line_start(&self.buffer, pos)
    }

    /// End: the end of a wrapped row that is not the line's last, then
    /// the line's end.
    pub(super) fn row_end_target(&mut self, pos: Position) -> Position {
        if self.wraps() {
            let rows = self.rows(pos.line);
            let row = rows.row_of(pos.column);
            let (_, end) = rows.span(row);
            if row < rows.last() && pos.column < end {
                return movement::position_at_line_column(&self.buffer, pos.line, end);
            }
        }
        movement::line_end(&self.buffer, pos)
    }
}
