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
//!
//! A narrow bar degrades instead of overlapping: before the query field
//! drops below [`MIN_FIELD`] points the bar hides Find All, then Prev,
//! then draws smaller toggles, then hides the toggles, then Replace All,
//! and finally the buttons. A hidden control has an empty rectangle, which is neither
//! painted nor hit. Every hidden control has a keyboard command.

use super::SearchBarLayout;

/// Side of a toggle and of the close button, in points.
pub(crate) const TOGGLE: f32 = 22.0;
/// Height of a field or button row, in points.
pub(crate) const ROW: f32 = 24.0;
/// Inset of the text inside a field, in points.
pub(crate) const TEXT_INSET: f32 = 6.0;
/// Narrowest query field, in points, the bar keeps before it drops a
/// control.
pub(crate) const MIN_FIELD: f32 = 120.0;
const EDGE: f32 = 8.0;
const GAP: f32 = 4.0;
const BUTTON_GAP: f32 = 6.0;
const FIND_W: f32 = 52.0;
const PREV_W: f32 = 52.0;
const FIND_ALL_W: f32 = 84.0;
const COMPACT_REPLACE_W: f32 = 60.0;
const REPLACE_ALL_W: f32 = 84.0;

/// Which controls a bar of a given width shows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Level {
    /// Toggle side and gap in points; `None` hides the toggles.
    toggles: Option<(f32, f32)>,
    /// Find-row buttons shown: 3 = Find, Prev, Find All; 0 = none. The
    /// replace row shows Replace whenever the find row shows any button.
    buttons: u8,
    /// Whether the replace row shows Replace All.
    replace_all: bool,
}

/// From the full bar to the barest one, in the order controls go.
const LEVELS: [Level; 7] = [
    Level::new(Some((TOGGLE, GAP)), 3, true),
    Level::new(Some((TOGGLE, GAP)), 2, true),
    Level::new(Some((TOGGLE, GAP)), 1, true),
    Level::new(Some((18.0, 2.0)), 1, true),
    Level::new(None, 1, true),
    Level::new(None, 1, false),
    Level::new(None, 0, false),
];

impl Level {
    const fn new(toggles: Option<(f32, f32)>, buttons: u8, replace_all: bool) -> Self {
        Level {
            toggles,
            buttons,
            replace_all,
        }
    }

    fn find_widths(self) -> &'static [f32] {
        match self.buttons {
            3 => &[FIND_W, PREV_W, FIND_ALL_W],
            2 => &[FIND_W, PREV_W],
            1 => &[FIND_W],
            _ => &[],
        }
    }

    /// Replace-row button widths. With all three find buttons the two
    /// replace buttons line up under them.
    fn replace_widths(self) -> &'static [f32] {
        match (self.buttons, self.replace_all) {
            (3, _) => &[FIND_W + BUTTON_GAP + PREV_W, FIND_ALL_W],
            (1 | 2, true) => &[COMPACT_REPLACE_W, REPLACE_ALL_W],
            (1 | 2, false) => &[COMPACT_REPLACE_W],
            _ => &[],
        }
    }

    /// Points the button column takes.
    fn column(self, replace_mode: bool) -> f32 {
        let span = |widths: &[f32]| {
            widths.iter().sum::<f32>() + BUTTON_GAP * widths.len().saturating_sub(1) as f32
        };
        let find = span(self.find_widths());
        if replace_mode {
            find.max(span(self.replace_widths()))
        } else {
            find
        }
    }

    /// Points the toggles take, with the gap before the field.
    fn toggles_span(self) -> f32 {
        self.toggles
            .map_or(0.0, |(side, gap)| 4.0 * side + 3.0 * gap + EDGE)
    }
}

/// Points left to the query field at `level` in a bar `width` points wide.
fn field_points(level: Level, width: f32, replace_mode: bool) -> f32 {
    let column = level.column(replace_mode);
    let column = if column > 0.0 { column + EDGE } else { 0.0 };
    width - EDGE - level.toggles_span() - column - EDGE - TOGGLE - EDGE
}

fn pick(width: f32, replace_mode: bool) -> Level {
    LEVELS
        .into_iter()
        .find(|&level| field_points(level, width, replace_mode) >= MIN_FIELD)
        .unwrap_or(LEVELS[LEVELS.len() - 1])
}

/// Places every control of the bar in `bar` (`[x, y, width, height]`,
/// physical pixels). `scale` turns points into pixels.
pub(crate) fn layout(bar: [f32; 4], scale: f32, replace_mode: bool) -> SearchBarLayout {
    let [bx, by, bw, _] = bar;
    let p = |points: f32| points * scale;
    let level = pick(bw / scale, replace_mode);
    let row1 = by + p(5.0);
    let row2 = by + p(34.0);
    let mut out = SearchBarLayout {
        bar_rect: bar,
        ..SearchBarLayout::default()
    };

    let close_x = bx + bw - p(EDGE) - p(TOGGLE);
    if close_x >= bx {
        out.close_btn_rect = [close_x, row1 + p(ROW - TOGGLE) * 0.5, p(TOGGLE), p(TOGGLE)];
    }

    let mut field_x = bx + p(EDGE);
    if let Some((side, gap)) = level.toggles {
        let toggle = |row: f32, slot: usize| {
            let x = bx + p(EDGE) + slot as f32 * p(side + gap);
            [x, row + p(ROW - side) * 0.5, p(side), p(side)]
        };
        out.regex_btn_rect = toggle(row1, 0);
        out.case_btn_rect = toggle(row1, 1);
        out.word_btn_rect = toggle(row1, 2);
        out.selection_btn_rect = toggle(row1, 3);
        if replace_mode {
            out.preserve_case_btn_rect = toggle(row2, 0);
        }
        field_x += p(level.toggles_span());
    }

    let column = p(level.column(replace_mode));
    let column_x = close_x - p(EDGE) - column;
    let field_end = if column > 0.0 {
        column_x - p(EDGE)
    } else {
        close_x - p(EDGE)
    };
    let field_w = field_end - field_x;
    if field_w > 0.0 {
        out.find_input_rect = [field_x, row1, field_w, p(ROW)];
        if replace_mode {
            out.replace_input_rect = [field_x, row2, field_w, p(ROW)];
        }
    }

    let place = |row: f32, widths: &[f32]| {
        let mut x = column_x;
        widths
            .iter()
            .map(|&w| {
                let rect = [x, row, p(w), p(ROW)];
                x += p(w + BUTTON_GAP);
                rect
            })
            .collect::<Vec<_>>()
    };
    let slots = [
        &mut out.next_btn_rect,
        &mut out.prev_btn_rect,
        &mut out.find_all_btn_rect,
    ];
    for (slot, rect) in slots.into_iter().zip(place(row1, level.find_widths())) {
        *slot = rect;
    }
    if replace_mode {
        let slots = [&mut out.replace_btn_rect, &mut out.replace_all_btn_rect];
        for (slot, rect) in slots.into_iter().zip(place(row2, level.replace_widths())) {
            *slot = rect;
        }
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

    fn visible(l: &SearchBarLayout) -> Vec<[f32; 4]> {
        controls(l).into_iter().filter(|r| r[2] > 0.0).collect()
    }

    fn assert_apart(rects: &[[f32; 4]]) {
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

    #[test]
    fn controls_do_not_overlap() {
        let l = layout([220.0, 600.0, 780.0, 64.0], 1.0, true);
        assert_eq!(visible(&l).len(), controls(&l).len(), "all shown at 780 pt");
        assert_apart(&controls(&l));
    }

    #[test]
    fn narrow_bars_drop_controls_instead_of_overlapping() {
        for scale in [1.0, 2.0] {
            for replace in [false, true] {
                for points in [300.0, 400.0, 780.0] {
                    let bar = [100.0 * scale, 500.0 * scale, points * scale, 64.0 * scale];
                    let l = layout(bar, scale, replace);
                    let shown = visible(&l);
                    assert_apart(&shown);
                    for rect in &shown {
                        assert!(inside(*rect, bar), "{rect:?} outside {bar:?}");
                    }
                    let field = l.find_input_rect[2];
                    assert!(field >= MIN_FIELD * scale, "{points} pt: field {field}");
                    assert!(l.close_btn_rect[2] > 0.0, "close stays");
                    assert!(l.next_btn_rect[2] > 0.0, "Find stays at {points} pt");
                    if replace {
                        assert!(l.replace_btn_rect[2] > 0.0);
                        if points >= 400.0 {
                            assert!(l.replace_all_btn_rect[2] > 0.0);
                        }
                        assert_eq!(l.replace_input_rect[2], field);
                    }
                }
                let at =
                    |points: f32| layout([0.0, 0.0, points * scale, 64.0 * scale], scale, replace);
                let wide = at(780.0);
                assert!(wide.find_all_btn_rect[2] > 0.0 && wide.prev_btn_rect[2] > 0.0);
                let narrow = at(300.0);
                assert_eq!(narrow.find_all_btn_rect[2], 0.0, "Find All goes first");
                assert_eq!(narrow.prev_btn_rect[2], 0.0, "then Prev");
            }
        }
    }

    #[test]
    fn every_width_keeps_controls_apart_and_drops_them_in_order() {
        for replace in [false, true] {
            let mut width = 40.0;
            while width <= 1200.0 {
                let bar = [0.0, 0.0, width, 64.0];
                let l = layout(bar, 1.0, replace);
                let shown = visible(&l);
                assert_apart(&shown);
                for rect in &shown {
                    assert!(inside(*rect, bar), "{width}: {rect:?} outside");
                }
                if l.find_all_btn_rect[2] > 0.0 {
                    assert!(l.prev_btn_rect[2] > 0.0, "{width}: Prev before Find All");
                }
                if l.prev_btn_rect[2] > 0.0 {
                    assert!(l.regex_btn_rect[2] > 0.0, "{width}: toggles before Prev");
                }
                if l.regex_btn_rect[2] > 0.0 {
                    assert!(l.next_btn_rect[2] > 0.0, "{width}: Find before toggles");
                }
                if replace && l.regex_btn_rect[2] > 0.0 {
                    assert!(l.replace_all_btn_rect[2] > 0.0, "{width}: toggles first");
                }
                if l.replace_all_btn_rect[2] > 0.0 {
                    assert!(l.replace_btn_rect[2] > 0.0, "{width}: Replace All first");
                }
                width += 1.0;
            }
        }
    }

    #[test]
    fn a_hidden_control_is_not_hit() {
        let l = layout([0.0, 0.0, 300.0, 64.0], 1.0, false);
        assert_eq!(l.find_all_btn_rect, [0.0; 4]);
        assert!(!crate::search::click::rect_contains(
            l.find_all_btn_rect,
            0.0,
            0.0
        ));
        let [x, y, w, h] = l.next_btn_rect;
        assert!(crate::search::click::rect_contains(
            l.next_btn_rect,
            x + w / 2.0,
            y + h / 2.0
        ));
    }
}
