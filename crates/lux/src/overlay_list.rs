//! One list model for every floating picker.
//!
//! The card is about 60% of the window, with a floor of six rows when the
//! window can hold them. The visible slice follows the selection, so a long
//! list stays virtualized. Wheel, arrows, Page Up/Down, Home, and End all
//! move that selection and keep it on screen.
//!
//! Trackpad pixels move one row per 16 logical pixels. A full row of pixels
//! made a retina trackpad look dead: the language picker also multiplied the
//! row height by the scale a second time.

use lux_platform::{Key, NamedKey, ScrollDelta};

/// Byte ranges of a row's title the query matched, as the ranking matcher
/// aligned them. The card lights exactly these glyphs.
pub(crate) type Hits = Vec<std::ops::Range<usize>>;

pub(crate) const HEIGHT_FRACTION: f32 = 0.60;
/// A one-line row: Sublime's picker density, 13 pt text in 28 pt.
pub(crate) const ROW_PT: f32 = 28.0;
/// A two-line row: 13 pt title over an 11 pt path.
pub(crate) const TWO_LINE_ROW_PT: f32 = 44.0;
pub(crate) const TITLE_PT: f32 = 13.0;
pub(crate) const SUBTITLE_PT: f32 = 11.0;
/// Gap between the title and path lines of a two-line row.
const LINE_GAP_PT: f32 = 4.0;
/// Every list, and the query in the input, start text at this inset.
pub(crate) const TEXT_INSET_PT: f32 = 22.0;
/// Right-aligned hints end this far inside the card, clear of the
/// selection fill and the scrollbar.
pub(crate) const DETAIL_RIGHT_PT: f32 = 28.0;
pub(crate) const MIN_ROWS: usize = 6;
const TRACK_PX: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ListFrame {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub input_height: f32,
    pub row_height: f32,
    pub rows_top: f32,
    pub radius: f32,
    pub visible: usize,
    pub origin: usize,
    /// Rows carry a second, smaller line (goto's file paths).
    pub two_line: bool,
}

/// Where one row's text sits, in the same space as the frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RowSlots {
    pub top: f32,
    pub text_left: f32,
    /// Top of the 13 pt title box.
    pub title_top: f32,
    /// Top of the 11 pt path box on a two-line row.
    pub subtitle_top: Option<f32>,
}

impl ListFrame {
    /// Text geometry of the row at `offset` inside the visible slice.
    /// `rows_top` is passed in so the open animation can shift paint
    /// while hit-testing keeps the resting frame.
    pub(crate) fn row_slots(&self, rows_top: f32, offset: usize, scale: f32) -> RowSlots {
        let top = rows_top + offset as f32 * self.row_height;
        let title = TITLE_PT * scale;
        let (title_top, subtitle_top) = if self.two_line {
            let block = title + (LINE_GAP_PT + SUBTITLE_PT) * scale;
            let title_top = top + (self.row_height - block) * 0.5;
            (title_top, Some(title_top + title + LINE_GAP_PT * scale))
        } else {
            (top + (self.row_height - title) * 0.5, None)
        };
        RowSlots {
            top,
            text_left: self.left + TEXT_INSET_PT * scale,
            title_top,
            subtitle_top,
        }
    }

    /// The list index under `(x, y)`, from the same rows paint uses.
    pub(crate) fn row_at(&self, total: usize, x: f32, y: f32) -> Option<usize> {
        let bottom = self.rows_top + self.visible as f32 * self.row_height;
        if x < self.left
            || x > self.left + self.width
            || y < self.rows_top
            || y >= bottom
            || self.row_height <= 0.0
        {
            return None;
        }
        let index = self.origin + ((y - self.rows_top) / self.row_height) as usize;
        (index < total).then_some(index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    Step(isize),
    Page(isize),
    Start,
    End,
}

pub(crate) fn centered_frame(
    viewport_width: f32,
    window_height: f32,
    tab_bar: f32,
    status_bar: f32,
    scale: f32,
    total: usize,
    selected: usize,
) -> ListFrame {
    centered_frame_rows(
        viewport_width,
        window_height,
        tab_bar,
        status_bar,
        scale,
        total,
        selected,
        false,
    )
}

/// The centered card with one- or two-line rows.
#[allow(clippy::too_many_arguments)]
pub(crate) fn centered_frame_rows(
    viewport_width: f32,
    window_height: f32,
    tab_bar: f32,
    status_bar: f32,
    scale: f32,
    total: usize,
    selected: usize,
    two_line: bool,
) -> ListFrame {
    let mut box_ = crate::render::panel::selector_box(viewport_width, tab_bar, scale);
    if two_line {
        box_.row_height = TWO_LINE_ROW_PT * scale;
    }
    let available = (window_height - tab_bar - status_bar - 16.0 * scale).max(box_.row_height);
    // A short window wins over the four-row floor: `clamp` would panic when
    // the floor is taller than the space, so cap after the floor instead.
    let target = (window_height * HEIGHT_FRACTION)
        .max(box_.row_height * 4.0)
        .min(available);
    frame_from(
        box_.left,
        box_.top,
        box_.width,
        box_.input_height,
        box_.row_height,
        box_.radius,
        target,
        scale,
        total,
        selected,
        two_line,
    )
}

/// Completion stays next to the caret. Its cap is lower so the caret line
/// remains visible; the row math is the same card.
pub(crate) fn anchored_frame(
    x: f32,
    y: f32,
    width: f32,
    max_height: f32,
    scale: f32,
    total: usize,
    selected: usize,
) -> ListFrame {
    let row = 36.0 * scale;
    frame_from(
        x,
        y,
        width,
        8.0 * scale,
        row,
        8.0 * scale,
        max_height,
        scale,
        total,
        selected,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn frame_from(
    left: f32,
    top: f32,
    width: f32,
    input_height: f32,
    row_height: f32,
    radius: f32,
    target_height: f32,
    scale: f32,
    total: usize,
    selected: usize,
    two_line: bool,
) -> ListFrame {
    let chrome = input_height + 8.0 * scale;
    let max_rows = ((target_height - chrome) / row_height).floor().max(1.0) as usize;
    let floor = MIN_ROWS.min(max_rows);
    let shown = if total == 0 {
        floor
    } else {
        total.min(max_rows).max(1)
    };
    let height = chrome + shown as f32 * row_height;
    ListFrame {
        left,
        top,
        width,
        height,
        input_height,
        row_height,
        rows_top: top + input_height + 2.0 * scale,
        radius,
        visible: shown,
        origin: origin(selected, shown, total),
        two_line,
    }
}

pub(crate) fn origin(selected: usize, visible: usize, total: usize) -> usize {
    if visible == 0 || total == 0 {
        return 0;
    }
    let selected = selected.min(total - 1);
    let max_origin = total.saturating_sub(visible);
    if selected < visible {
        0
    } else {
        (selected + 1 - visible).min(max_origin)
    }
}

pub(crate) fn nudge(selected: &mut usize, total: usize, visible: usize, key: &Key) -> bool {
    let Some(motion) = motion_for(key) else {
        return false;
    };
    *selected = apply(*selected, total, visible, motion);
    true
}

pub(crate) fn visible_rows(
    window_height: f32,
    tab_bar: f32,
    status_bar: f32,
    scale: f32,
    total: usize,
) -> usize {
    visible_rows_for(window_height, tab_bar, status_bar, scale, total, false)
}

/// Page size of a card whose rows may be two lines tall.
pub(crate) fn visible_rows_for(
    window_height: f32,
    tab_bar: f32,
    status_bar: f32,
    scale: f32,
    total: usize,
    two_line: bool,
) -> usize {
    centered_frame_rows(
        1000.0,
        window_height,
        tab_bar,
        status_bar,
        scale,
        total,
        0,
        two_line,
    )
    .visible
}

pub(crate) fn motion_for(key: &Key) -> Option<Motion> {
    let Key::Named(named) = key else {
        return None;
    };
    Some(match named {
        NamedKey::ArrowDown => Motion::Step(1),
        NamedKey::ArrowUp => Motion::Step(-1),
        NamedKey::PageDown => Motion::Page(1),
        NamedKey::PageUp => Motion::Page(-1),
        NamedKey::Home => Motion::Start,
        NamedKey::End => Motion::End,
        _ => return None,
    })
}

pub(crate) fn apply(selected: usize, total: usize, visible: usize, motion: Motion) -> usize {
    if total == 0 {
        return 0;
    }
    let last = total - 1;
    let page = visible.max(1) as isize;
    match motion {
        Motion::Step(delta) | Motion::Page(delta) => {
            let step = if matches!(motion, Motion::Page(_)) {
                delta.saturating_mul(page)
            } else {
                delta
            };
            selected.saturating_add_signed(step).min(last)
        }
        Motion::Start => 0,
        Motion::End => last,
    }
}

/// One row per wheel line, or per 16 logical pixels of trackpad travel.
pub(crate) fn wheel_steps(slot: u8, delta: ScrollDelta, scale: f32) -> isize {
    crate::overlay_wheel::wheel_steps(slot, delta, TRACK_PX * scale.max(1.0))
}

pub(crate) fn window_height(viewport_height: f32, tab_bar: f32, status_bar: f32) -> f32 {
    viewport_height + tab_bar + status_bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tall_window_shows_more_than_eight_rows() {
        let frame = centered_frame(1600.0, 1200.0, 32.0, 24.0, 1.0, 40, 0);
        assert!(frame.visible > 8, "visible {}", frame.visible);
        assert!(frame.height <= 1200.0 * HEIGHT_FRACTION + 1.0);
        assert!(frame.height >= ROW_PT * 6.0);
    }

    #[test]
    fn a_tiny_window_degrades_to_fewer_rows_without_panicking() {
        for scale in [1.0, 2.0] {
            for status_bar in [0.0, 24.0 * scale] {
                for height in (0..=400).step_by(4) {
                    let frame = centered_frame(
                        800.0,
                        height as f32,
                        32.0 * scale,
                        status_bar,
                        scale,
                        40,
                        39,
                    );
                    assert!(frame.visible >= 1 && frame.visible <= 40);
                    assert!(frame.height.is_finite());
                    assert!(frame.origin + frame.visible <= 40);
                }
            }
        }
        let short = centered_frame(800.0, 200.0, 32.0, 24.0, 1.0, 40, 0);
        assert!(short.visible < MIN_ROWS, "visible {}", short.visible);
    }

    #[test]
    fn the_selection_stays_inside_the_visible_slice() {
        let total = 40;
        let visible = 10;
        let selected = apply(0, total, visible, Motion::End);
        assert_eq!(selected, 39);
        let start = origin(selected, visible, total);
        assert!(selected >= start && selected < start + visible);
        assert_eq!(start, 30);
        let paged = apply(0, total, visible, Motion::Page(1));
        assert_eq!(paged, 10);
        assert_eq!(apply(paged, total, visible, Motion::Start), 0);
    }

    #[test]
    fn home_and_end_are_recognized() {
        assert_eq!(motion_for(&Key::Named(NamedKey::End)), Some(Motion::End));
        assert_eq!(motion_for(&Key::Named(NamedKey::Home)), Some(Motion::Start));
        assert!(motion_for(&Key::Named(NamedKey::Escape)).is_none());
    }

    #[test]
    fn rows_are_28pt_and_two_line_rows_44pt_at_every_scale() {
        for scale in [1.0, 2.0] {
            let one = centered_frame(
                1000.0 * scale,
                700.0 * scale,
                32.0 * scale,
                0.0,
                scale,
                40,
                0,
            );
            let two = centered_frame_rows(
                1000.0 * scale,
                700.0 * scale,
                32.0 * scale,
                0.0,
                scale,
                40,
                0,
                true,
            );
            assert!((one.row_height - ROW_PT * scale).abs() < 0.01);
            assert!((two.row_height - TWO_LINE_ROW_PT * scale).abs() < 0.01);
            assert!(!one.two_line && two.two_line);
            assert_eq!(one.left, two.left, "same card, same column");
            assert!(one.visible > two.visible);
        }
    }

    #[test]
    fn every_list_starts_text_at_the_same_inset() {
        let scale = 2.0;
        let one = centered_frame(2000.0, 1400.0, 64.0, 0.0, scale, 40, 0);
        let two = centered_frame_rows(2000.0, 1400.0, 64.0, 0.0, scale, 40, 0, true);
        let near = anchored_frame(300.0, 200.0, 600.0, 500.0, scale, 10, 0);
        for frame in [one, two, near] {
            let slots = frame.row_slots(frame.rows_top, 0, scale);
            assert_eq!(slots.text_left - frame.left, TEXT_INSET_PT * scale);
        }
    }

    #[test]
    fn two_line_rows_stack_inside_their_row_and_hit_test_like_paint() {
        let scale = 2.0;
        let total = 30;
        let frame = centered_frame_rows(2000.0, 1400.0, 64.0, 0.0, scale, total, 25, true);
        assert!(frame.origin > 0, "the selection scrolled the slice");
        let x = frame.left + frame.width * 0.5;
        for offset in 0..frame.visible {
            let slots = frame.row_slots(frame.rows_top, offset, scale);
            let subtitle = slots.subtitle_top.expect("two-line row");
            let bottom = slots.top + frame.row_height;
            assert!(slots.title_top >= slots.top);
            assert!(subtitle >= slots.title_top + TITLE_PT * scale);
            assert!(subtitle + SUBTITLE_PT * scale <= bottom);
            let want = Some(frame.origin + offset);
            for y in [slots.top + 0.5, slots.title_top, subtitle, bottom - 0.5] {
                assert_eq!(frame.row_at(total, x, y), want, "offset {offset} y {y}");
            }
        }
        let below = frame.rows_top + frame.visible as f32 * frame.row_height;
        assert_eq!(frame.row_at(total, x, below), None);
        assert_eq!(
            frame.row_at(total, frame.left - 1.0, frame.rows_top + 1.0),
            None
        );
    }
}
