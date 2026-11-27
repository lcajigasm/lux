//! Indent guides: where each visible line draws them, and which one marks
//! the block that holds the primary caret.

use lux_core::buffer::Indent;
use lux_render::LineLayout;

/// Indent-guide x-positions for one line: one guide at the start of each
/// full indent level of leading whitespace, using `Buffer::indent()`'s
/// detected width. Level `n` sits at column `n * width`, so the deepest
/// guide is one indent left of the first glyph and never touches it.
///
/// Every leading space or tab counts as one column, matching
/// `lux_core::Position::column`'s grapheme-count semantics, which don't
/// expand tabs either. Upgrade with real tab expansion once `lux-config`
/// defines a visual tab-width setting.
pub(super) fn indent_guide_x(layout: &LineLayout, line_text: &str, indent: Indent) -> Vec<f32> {
    let width = indent.width.max(1) as usize;
    let leading = line_text
        .bytes()
        .take_while(|&b| b == b' ' || b == b'\t')
        .count();
    let levels = leading / width;
    (0..levels)
        .map(|level| super::wrap::place_byte(layout, level * width).0)
        .collect()
}

/// One visible line as the active-guide search sees it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GuideLine {
    /// Number of guides the line draws.
    pub levels: usize,
    /// Whitespace-only lines continue the block around them.
    pub blank: bool,
}

/// For each visible line, the index of the guide that belongs to the
/// caret's block, if that line draws it. The caret's line at level `L`
/// (one guide per level) makes guide `L - 1` active on every neighbor
/// that stays at level `L` or deeper, or is blank. The walk stops at the
/// first shallower line, so the opener and closer of the block stay
/// plain. Only visible lines are read, so the cost is bounded by the
/// viewport and a caret off screen marks nothing.
pub fn active_guides(lines: &[GuideLine], caret: Option<usize>) -> Vec<Option<usize>> {
    let mut active = vec![None; lines.len()];
    let Some(caret) = caret.filter(|&c| c < lines.len()) else {
        return active;
    };
    let level = caret_level(lines, caret);
    if level == 0 {
        return active;
    }
    let guide = level - 1;
    let inside = |index: &usize| lines[*index].blank || lines[*index].levels >= level;
    let above = (0..caret).rev().take_while(inside);
    let below = (caret + 1..lines.len()).take_while(inside);
    for index in std::iter::once(caret).chain(above).chain(below) {
        if lines[index].levels > guide {
            active[index] = Some(guide);
        }
    }
    active
}

/// A caret on a blank line takes the nearest non-blank level below it,
/// then above it, the way the next typed line would be indented.
fn caret_level(lines: &[GuideLine], caret: usize) -> usize {
    if !lines[caret].blank {
        return lines[caret].levels;
    }
    lines[caret + 1..]
        .iter()
        .chain(lines[..caret].iter().rev())
        .find(|line| !line.blank)
        .map_or(0, |line| line.levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(levels: usize) -> GuideLine {
        GuideLine {
            levels,
            blank: false,
        }
    }

    const BLANK: GuideLine = GuideLine {
        levels: 0,
        blank: true,
    };

    #[test]
    fn the_caret_block_marks_its_own_guide_through_blank_lines() {
        // fn a() {        0
        //     let x;      1  <- caret
        //                 blank
        //     if x {      1
        //         y();    2
        //     }           1
        // }               0
        let lines = [line(0), line(1), BLANK, line(1), line(2), line(1), line(0)];
        let active = active_guides(&lines, Some(1));
        assert_eq!(
            active,
            vec![None, Some(0), None, Some(0), Some(0), Some(0), None]
        );
    }

    #[test]
    fn a_nested_caret_marks_only_the_inner_guide() {
        let lines = [line(1), line(2), line(2), line(1)];
        let active = active_guides(&lines, Some(2));
        assert_eq!(active, vec![None, Some(1), Some(1), None]);
    }

    #[test]
    fn a_top_level_or_missing_caret_marks_nothing() {
        let lines = [line(0), line(1)];
        assert_eq!(active_guides(&lines, Some(0)), vec![None, None]);
        assert_eq!(active_guides(&lines, None), vec![None, None]);
        assert_eq!(active_guides(&lines, Some(9)), vec![None, None]);
    }

    #[test]
    fn a_caret_on_a_blank_line_uses_the_next_line_level() {
        let lines = [line(0), BLANK, line(1), line(0)];
        assert_eq!(
            active_guides(&lines, Some(1)),
            vec![None, None, Some(0), None]
        );
    }
}
