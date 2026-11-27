//! Highlight rectangles for a text range on the visible lines.
//!
//! Selections and find matches share this geometry, so a match box sits
//! exactly where a selection of the same range would. Positions come from
//! the shaped clusters, so tabs, wide characters and soft-wrap rows are
//! placed as the text is drawn.

use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_render::LineLayout;

use super::wrap::place_byte;
use super::{EditorViewFrame, SelectionRect, VisibleLine};

impl EditorViewFrame {
    /// Rectangles covering `start..end` on the lines of this frame, in
    /// document pixels (before scroll and the text origin are applied).
    /// Lines outside the frame produce nothing.
    pub fn range_rects(
        &self,
        buffer: &Buffer,
        start: Position,
        end: Position,
        line_height: f32,
    ) -> Vec<SelectionRect> {
        let mut out = Vec::new();
        push_range_rects(&self.lines, buffer, start, end, line_height, &mut out);
        out
    }
}

/// Appends the rectangles of `start..end` (ordered) to `out`.
pub(super) fn push_range_rects(
    lines: &[VisibleLine],
    buffer: &Buffer,
    start: Position,
    end: Position,
    line_height: f32,
    out: &mut Vec<SelectionRect>,
) {
    let first = lines.partition_point(|line| line.index < start.line);
    for line in &lines[first..] {
        if line.index > end.line {
            break;
        }
        let Some(line_start) = buffer.line_start_byte(line.index) else {
            continue;
        };
        let (x_start, row_start) = if line.index == start.line {
            place_byte(&line.layout, start.byte - line_start)
        } else {
            (0.0, 0)
        };
        let (x_end, row_end) = if line.index == end.line {
            place_byte(&line.layout, end.byte - line_start)
        } else {
            (line.layout.width(), last_row(&line.layout))
        };
        let rect = |row: u32, x_start: f32, x_end: f32| SelectionRect {
            line: line.index,
            visual_row: line.visual_index,
            x_start,
            x_end,
            y: line.top + row as f32 * line_height,
        };
        if row_end <= row_start {
            out.push(rect(row_start, x_start, x_end));
            continue;
        }
        // A range that crosses soft-wrap rows: the tail of the first row,
        // whole middle rows, and the head of the last row.
        out.push(rect(row_start, x_start, row_width(&line.layout, row_start)));
        for row in row_start + 1..row_end {
            out.push(rect(row, 0.0, row_width(&line.layout, row)));
        }
        if x_end > 0.0 {
            out.push(rect(row_end, 0.0, x_end));
        }
    }
}

fn last_row(layout: &LineLayout) -> u32 {
    layout.clusters().last().map_or(0, |cluster| cluster.row)
}

fn row_width(layout: &LineLayout, row: u32) -> f32 {
    layout
        .clusters()
        .iter()
        .filter(|cluster| cluster.row == row)
        .map(|cluster| cluster.x + cluster.advance)
        .fold(0.0, f32::max)
}
