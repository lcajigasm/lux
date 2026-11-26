//! Geometry inside the bottom dock, shared by its painter and its clicks.
//!
//! The dock rectangle itself comes from [`crate::frame_layout`]. Everything
//! here is relative to it, so the dock can move (sidebar shown or hidden,
//! find bar open) without the hit targets drifting from the paint.

/// Height of the tab strip at the top of the dock, in points.
pub(crate) const HEADER_HEIGHT: f32 = 26.0;

/// Top of the content area under the tab strip.
pub(crate) fn grid_top(dock: [f32; 4], scale: f32) -> f32 {
    dock[1] + HEADER_HEIGHT * scale
}

const HEADER_LEFT: f32 = 8.0;
const TAB_WIDTH: f32 = 90.0;
const TAB_GAP: f32 = 4.0;
const TAB_CLOSE: f32 = 20.0;
const PLUS_WIDTH: f32 = 28.0;
const TASKS_WIDTH: f32 = 60.0;
const PANEL_CLOSE: f32 = 28.0;

/// Slots of the header strip, `[x, y, w, h]` in physical pixels: one per
/// terminal session, then `+`, then Tasks; the panel's close button sits
/// on the right edge.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeaderSlots {
    pub tabs: Vec<[f32; 4]>,
    pub plus: [f32; 4],
    pub tasks: [f32; 4],
    pub close: [f32; 4],
}

pub(crate) fn header_slots(dock: [f32; 4], sessions: usize, scale: f32) -> HeaderSlots {
    let [x, y, w, _] = dock;
    let h = HEADER_HEIGHT * scale;
    let mut left = x + HEADER_LEFT * scale;
    let mut slot = |width: f32| {
        let rect = [left, y, width * scale, h];
        left += (width + TAB_GAP) * scale;
        rect
    };
    let tabs = (0..sessions).map(|_| slot(TAB_WIDTH)).collect();
    let plus = slot(PLUS_WIDTH);
    let tasks = slot(TASKS_WIDTH);
    HeaderSlots {
        tabs,
        plus,
        tasks,
        close: [x + w - PANEL_CLOSE * scale, y, PANEL_CLOSE * scale, h],
    }
}

/// What a click at `x` on the header strip lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeaderHit {
    Tab(usize),
    CloseTab(usize),
    NewTab,
    Tasks,
    ClosePanel,
    Nothing,
}

pub(crate) fn header_hit(slots: &HeaderSlots, scale: f32, x: f32) -> HeaderHit {
    let within = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2];
    for (index, tab) in slots.tabs.iter().enumerate() {
        if within(*tab) {
            return if x >= tab[0] + tab[2] - TAB_CLOSE * scale {
                HeaderHit::CloseTab(index)
            } else {
                HeaderHit::Tab(index)
            };
        }
    }
    if within(slots.plus) {
        HeaderHit::NewTab
    } else if within(slots.tasks) {
        HeaderHit::Tasks
    } else if x >= slots.close[0] {
        HeaderHit::ClosePanel
    } else {
        HeaderHit::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_clicks_land_on_the_slot_the_painter_draws() {
        let dock = [220.0, 400.0, 780.0, 260.0];
        for scale in [1.0_f32, 2.0] {
            let dock = dock.map(|v| v * scale);
            let slots = header_slots(dock, 2, scale);
            let center = |r: [f32; 4]| r[0] + r[2] * 0.4;
            assert_eq!(
                header_hit(&slots, scale, center(slots.tabs[0])),
                HeaderHit::Tab(0)
            );
            assert_eq!(
                header_hit(&slots, scale, center(slots.tabs[1])),
                HeaderHit::Tab(1)
            );
            let tab_end = slots.tabs[1][0] + slots.tabs[1][2] - scale;
            assert_eq!(header_hit(&slots, scale, tab_end), HeaderHit::CloseTab(1));
            assert_eq!(
                header_hit(&slots, scale, center(slots.plus)),
                HeaderHit::NewTab
            );
            assert_eq!(
                header_hit(&slots, scale, center(slots.tasks)),
                HeaderHit::Tasks
            );
            assert_eq!(
                header_hit(&slots, scale, dock[0] + dock[2] - scale),
                HeaderHit::ClosePanel
            );
            assert!(slots.tabs[0][0] >= dock[0], "inside the dock column");
            assert!(slots.plus[0] >= slots.tabs[1][0] + slots.tabs[1][2]);
            assert!(slots.tasks[0] >= slots.plus[0] + slots.plus[2]);
        }
    }

    #[test]
    fn content_starts_under_the_tab_strip_at_both_scales() {
        for scale in [1.0, 2.0] {
            let dock = [220.0 * scale, 400.0 * scale, 780.0 * scale, 200.0 * scale];
            assert_eq!(grid_top(dock, scale), dock[1] + 26.0 * scale);
        }
    }
}
