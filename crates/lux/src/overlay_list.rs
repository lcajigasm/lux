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

pub(crate) const HEIGHT_FRACTION: f32 = 0.60;
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
    let box_ = crate::render::panel::selector_box(viewport_width, tab_bar, scale);
    let available = (window_height - tab_bar - status_bar - 16.0 * scale).max(box_.row_height);
    let target = (window_height * HEIGHT_FRACTION).clamp(box_.row_height * 4.0, available);
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
    centered_frame(1000.0, window_height, tab_bar, status_bar, scale, total, 0).visible
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
        assert!(frame.height >= 36.0 * 6.0);
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
}
