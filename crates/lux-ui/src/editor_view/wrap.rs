//! Vertical slots for soft-wrapped lines.
//!
//! Only lines that intersect the viewport are shaped. The slot of a line
//! above the viewport is the number of rows its character count needs, so
//! scrolling does not reshape the whole buffer.

use lux_core::buffer::Buffer;
use lux_core::folding::FoldState;
use lux_render::LineLayout;

use super::Viewport;

/// Horizontal position and soft-wrap row of `byte_in_line`.
///
/// `LineLayout::width` omits trailing whitespace, so a caret after a tab
/// or trailing spaces uses the last shaped cluster instead.
pub(super) fn place_byte(layout: &LineLayout, byte_in_line: usize) -> (f32, u32) {
    let clusters = layout.clusters();
    for cluster in &clusters {
        if cluster.byte_range.contains(&byte_in_line) {
            let span = (cluster.byte_range.end - cluster.byte_range.start).max(1) as f32;
            let offset = (byte_in_line - cluster.byte_range.start) as f32;
            return (cluster.x + cluster.advance * (offset / span), cluster.row);
        }
    }
    let row = clusters.last().map(|cluster| cluster.row).unwrap_or(0);
    let x = clusters
        .last()
        .map(|cluster| cluster.x + cluster.advance)
        .unwrap_or(0.0);
    (x, row)
}

/// The prefix of `text` that fits in `count` Unicode scalars. A long
/// unwrapped line only needs the columns that intersect the viewport;
/// shaping the rest blocks the UI thread.
pub(super) fn prefix_chars(text: &str, count: usize) -> &str {
    if count == 0 {
        return "";
    }
    match text.char_indices().nth(count) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

pub(super) fn visible_slots(
    buffer: &Buffer,
    folds: Option<&FoldState>,
    viewport: &Viewport,
    line_height: f32,
    chars_per_line: usize,
) -> Vec<(usize, f32)> {
    let total_lines = buffer.len_lines();
    let total_visual = folds.map_or(total_lines, |fold| fold.total_visual_lines(total_lines));
    let chars_per_line = chars_per_line.max(1);
    let line_height = line_height.max(1.0);
    let bottom = viewport.scroll_y + viewport.height + line_height;
    let mut y = 0.0;
    let mut out = Vec::new();
    for visual in 0..total_visual {
        let buffer_line = folds.map_or(visual, |fold| {
            fold.visual_to_buffer_line(visual, total_lines)
        });
        let folded = folds.is_some_and(|fold| fold.is_line_folded(buffer_line));
        let rows = if folded {
            1
        } else {
            let cols = buffer
                .line_content(buffer_line)
                .map(|text| text.chars().count())
                .unwrap_or(1)
                .max(1);
            cols.div_ceil(chars_per_line).max(1)
        };
        let height = rows as f32 * line_height;
        if y + height > viewport.scroll_y && y < bottom {
            out.push((visual, y));
        }
        y += height;
        if y >= bottom && !out.is_empty() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_view::Viewport;

    #[test]
    fn a_long_line_takes_several_rows_and_a_short_line_takes_one() {
        let mut text = "a".repeat(400);
        text.push('\n');
        text.push_str("short\n");
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let viewport = Viewport {
            width: 800.0,
            height: 400.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        let slots = visible_slots(&buffer, None, &viewport, 20.0, 80);
        assert_eq!(slots[0], (0, 0.0));
        assert_eq!(slots[1].0, 1);
        assert_eq!(slots[1].1, 5.0 * 20.0);
    }

    #[test]
    fn prefix_chars_stops_on_a_scalar_boundary() {
        assert_eq!(prefix_chars("abcdef", 3), "abc");
        assert_eq!(prefix_chars("ééé", 2), "éé");
        assert_eq!(prefix_chars("ab", 8), "ab");
        assert_eq!(prefix_chars("ab", 0), "");
    }

    #[test]
    fn one_unindented_line_is_a_single_slot() {
        let buffer = Buffer::from_bytes(b"hello\n").unwrap();
        let viewport = Viewport {
            width: 800.0,
            height: 200.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        let slots = visible_slots(&buffer, None, &viewport, 22.0, 80);
        assert_eq!(slots[0], (0, 0.0));
        assert!(slots[1].1 - slots[0].1 <= 22.0 + f32::EPSILON);
    }
}
