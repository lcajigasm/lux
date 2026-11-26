//! Find bar layout, Sublime Text style. The painter and the click handler
//! both use [`layout`], so a hit always lands on the control drawn there.
//!
//! ```text
//! [.*][Aa]["w"][In]  [ query ............. 1 of 6 ]  [Find][Prev][Find All]  ×
//! [AB]               [ replace ...................]  [ Replace  ][Replace All]
//! ```
//!
//! Toggles are square icon buttons on the left, the fields fill the
//! remaining width, and the replace row reuses the find row's columns.

use super::SearchBarLayout;

/// Side of a toggle and of the close button, in points.
pub(crate) const TOGGLE: f32 = 22.0;
/// Height of a field or button row, in points.
pub(crate) const ROW: f32 = 24.0;
/// Inset of the text inside a field, in points.
pub(crate) const TEXT_INSET: f32 = 6.0;
const EDGE: f32 = 8.0;
const GAP: f32 = 4.0;
const BUTTON_GAP: f32 = 6.0;
const FIND_W: f32 = 52.0;
const PREV_W: f32 = 52.0;
const FIND_ALL_W: f32 = 84.0;
const MIN_FIELD: f32 = 60.0;

/// Places every control of the bar in `bar` (`[x, y, width, height]`,
/// physical pixels). `scale` turns points into pixels.
pub(crate) fn layout(bar: [f32; 4], scale: f32, replace_mode: bool) -> SearchBarLayout {
    let [bx, by, bw, _] = bar;
    let p = |points: f32| points * scale;
    let row1 = by + p(5.0);
    let row2 = by + p(34.0);
    let toggle = |row: f32, slot: usize| {
        let x = bx + p(EDGE) + slot as f32 * p(TOGGLE + GAP);
        [x, row + p(ROW - TOGGLE) * 0.5, p(TOGGLE), p(TOGGLE)]
    };
    let field_x = bx + p(EDGE) + 4.0 * p(TOGGLE) + 3.0 * p(GAP) + p(EDGE);

    let close_x = bx + bw - p(EDGE) - p(TOGGLE);
    let find_all_x = close_x - p(EDGE) - p(FIND_ALL_W);
    let prev_x = find_all_x - p(BUTTON_GAP) - p(PREV_W);
    let find_x = prev_x - p(BUTTON_GAP) - p(FIND_W);
    let field_w = (find_x - p(EDGE) - field_x).max(p(MIN_FIELD));

    let mut out = SearchBarLayout {
        bar_rect: bar,
        regex_btn_rect: toggle(row1, 0),
        case_btn_rect: toggle(row1, 1),
        word_btn_rect: toggle(row1, 2),
        selection_btn_rect: toggle(row1, 3),
        find_input_rect: [field_x, row1, field_w, p(ROW)],
        next_btn_rect: [find_x, row1, p(FIND_W), p(ROW)],
        prev_btn_rect: [prev_x, row1, p(PREV_W), p(ROW)],
        find_all_btn_rect: [find_all_x, row1, p(FIND_ALL_W), p(ROW)],
        close_btn_rect: toggle(row1, 0),
        ..SearchBarLayout::default()
    };
    out.close_btn_rect[0] = close_x;
    if replace_mode {
        out.preserve_case_btn_rect = toggle(row2, 0);
        out.replace_input_rect = [field_x, row2, field_w, p(ROW)];
        out.replace_btn_rect = [find_x, row2, prev_x + p(PREV_W) - find_x, p(ROW)];
        out.replace_all_btn_rect = [find_all_x, row2, p(FIND_ALL_W), p(ROW)];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_layout::{ChromeSizes, compute};

    fn inside(inner: [f32; 4], outer: [f32; 4]) -> bool {
        inner[0] >= outer[0]
            && inner[1] >= outer[1]
            && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01
            && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01
    }

    fn controls(l: &SearchBarLayout) -> Vec<[f32; 4]> {
        vec![
            l.regex_btn_rect,
            l.case_btn_rect,
            l.word_btn_rect,
            l.selection_btn_rect,
            l.find_input_rect,
            l.next_btn_rect,
            l.prev_btn_rect,
            l.find_all_btn_rect,
            l.close_btn_rect,
            l.preserve_case_btn_rect,
            l.replace_input_rect,
            l.replace_btn_rect,
            l.replace_all_btn_rect,
        ]
    }

    #[test]
    fn every_control_sits_in_the_editor_column_at_both_scales_with_and_without_sidebar() {
        for scale in [1.0, 2.0] {
            for sidebar in [0.0, 220.0 * scale] {
                let bar_h = 64.0 * scale;
                let frame = compute(&ChromeSizes {
                    window_width: 1000.0 * scale,
                    window_height: 700.0 * scale,
                    tab_bar: 32.0 * scale,
                    status_bar: 24.0 * scale,
                    search_bar: bar_h,
                    dock: 0.0,
                    sidebar,
                    minimap: 90.0 * scale,
                });
                let bar = frame.search_bar.expect("bar");
                assert_eq!(bar[0], sidebar, "the bar starts at the sidebar edge");
                assert_eq!(bar[0] + bar[2], 1000.0 * scale, "and spans the minimap");
                assert_eq!(bar[1], frame.editor[1] + frame.editor[3]);
                let l = layout(bar, scale, true);
                for rect in controls(&l) {
                    assert!(inside(rect, bar), "{rect:?} outside {bar:?}");
                    assert!(rect[0] >= sidebar, "{rect:?} reaches into the sidebar");
                }
                assert_eq!(l.regex_btn_rect[2], 22.0 * scale);
                assert_eq!(l.find_input_rect[0], l.replace_input_rect[0]);
                assert_eq!(l.find_input_rect[2], l.replace_input_rect[2]);
                assert_eq!(l.find_all_btn_rect[0], l.replace_all_btn_rect[0]);
                let field_end = l.find_input_rect[0] + l.find_input_rect[2];
                assert_eq!(field_end + 8.0 * scale, l.next_btn_rect[0], "field fills");
                assert!(l.next_btn_rect[0] < l.prev_btn_rect[0]);
                assert!(l.prev_btn_rect[0] < l.find_all_btn_rect[0]);
                let close_end = l.close_btn_rect[0] + l.close_btn_rect[2];
                assert_eq!(close_end, bar[0] + bar[2] - 8.0 * scale);
            }
        }
    }

    #[test]
    fn controls_do_not_overlap() {
        let l = layout([220.0, 600.0, 780.0, 64.0], 1.0, true);
        let rects = controls(&l);
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let apart = a[0] + a[2] <= b[0]
                    || b[0] + b[2] <= a[0]
                    || a[1] + a[3] <= b[1]
                    || b[1] + b[3] <= a[1];
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }
}
