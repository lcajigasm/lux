//! From window points and assistive-technology positions back to buffer
//! positions, and from a position to its x on screen.

use accesskit::{TextPosition, TextSelection};
use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_render::LineLayout;

use super::{EditorView, Viewport, x_for_byte};

impl EditorView {
    /// Maps a physical window point to the nearest valid text position.
    /// Only the pointed-at line is shaped, so mouse motion retains the
    /// editor's visible-work performance bound even for huge documents.
    pub fn hit_test(
        &mut self,
        buffer: &Buffer,
        viewport: &Viewport,
        x: f32,
        y: f32,
        left_padding: f32,
        folds: Option<&lux_core::folding::FoldState>,
    ) -> Position {
        let total_buffer_lines = buffer.len_lines();
        let total_visual_lines = folds.map_or(total_buffer_lines, |f| {
            f.total_visual_lines(total_buffer_lines)
        });
        let last_visual = total_visual_lines.saturating_sub(1);
        let visual_line = (((y.max(0.0) + viewport.scroll_y) / self.metrics.line_height.max(1.0))
            .floor() as usize)
            .min(last_visual);
        let line = folds.map_or(visual_line, |f| {
            f.visual_to_buffer_line(visual_line, total_buffer_lines)
        });
        let content = buffer.line_content(line).unwrap_or_default();
        let digits = total_buffer_lines.max(1).to_string().len();
        let gutter_width = self.reference_width(&"0".repeat(digits));
        let text_x = (x + viewport.scroll_x - gutter_width - left_padding).max(0.0);
        let style = Self::line_style(&self.family, self.metrics.font_size, self.metrics.ligatures);
        let layout = self.text_layout.layout_line(&style, &content, &[]);
        let byte_in_line = byte_for_x(&layout, text_x);
        let line_start = buffer.line_start_byte(line).unwrap_or(0);
        Position::from_byte(buffer, line_start + byte_in_line)
    }

    /// Horizontal position of a buffer position from the window's left
    /// edge, before applying horizontal scroll.
    pub fn x_for_position(
        &mut self,
        buffer: &Buffer,
        position: Position,
        left_padding: f32,
    ) -> f32 {
        let content = buffer.line_content(position.line).unwrap_or_default();
        let digits = buffer.len_lines().max(1).to_string().len();
        let gutter_width = self.reference_width(&"0".repeat(digits));
        let style = Self::line_style(&self.family, self.metrics.font_size, self.metrics.ligatures);
        let layout = self.text_layout.layout_line(&style, &content, &[]);
        let line_start = buffer.line_start_byte(position.line).unwrap_or(0);
        gutter_width + left_padding + x_for_byte(&layout, position.byte - line_start)
    }

    /// Converts AccessKit's line-node/cluster-index text model back to a
    /// core selection so assistive technology can move the real caret.
    pub fn selection_from_accessibility(
        &mut self,
        buffer: &Buffer,
        selection: TextSelection,
    ) -> Option<lux_core::selection::Selection> {
        let anchor = self.position_from_accessibility(buffer, selection.anchor)?;
        let focus = self.position_from_accessibility(buffer, selection.focus)?;
        Some(lux_core::selection::Selection::new(anchor, focus))
    }

    fn position_from_accessibility(
        &mut self,
        buffer: &Buffer,
        position: TextPosition,
    ) -> Option<Position> {
        let line = usize::try_from(position.node.0).ok()?.checked_sub(1)?;
        let content = buffer.line_content(line)?;
        let style = Self::line_style(&self.family, self.metrics.font_size, false);
        let layout = self.text_layout.layout_line(&style, &content, &[]);
        let clusters = layout.clusters();
        let byte_in_line = clusters
            .get(position.character_index)
            .map_or(content.len(), |cluster| cluster.byte_range.start);
        Some(Position::from_byte(
            buffer,
            buffer.line_start_byte(line)? + byte_in_line,
        ))
    }
}

/// Chooses the source boundary nearest a horizontal point. Cluster
/// boundaries keep the result out of the middle of a shaped grapheme or
/// programming ligature.
fn byte_for_x(layout: &LineLayout, x: f32) -> usize {
    let clusters = layout.clusters();
    for cluster in &clusters {
        if x < cluster.x + cluster.advance / 2.0 {
            return cluster.byte_range.start;
        }
        if x < cluster.x + cluster.advance {
            return cluster.byte_range.end;
        }
    }
    clusters.last().map_or(0, |cluster| cluster.byte_range.end)
}
