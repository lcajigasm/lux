//! Which tab strips the window has and what each one holds. Without a
//! split there is one strip over the editor holding every tab. In a split
//! each group has its own strip over its own column with its own tabs,
//! scroll, and `▼` button; only the strip at the window's leading corner
//! steps around the window controls.

use lux_ui::tabs::TabId;

use super::StripLayout;
use crate::app::{App, TAB_MAX_WIDTH, TAB_MIN_WIDTH};

/// One strip, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Strip {
    /// The split group it belongs to; `None` without a split.
    pub(crate) group: Option<usize>,
    pub(crate) top: f32,
    pub(crate) height: f32,
    /// Full width from `layout.left`, the `▼` button included.
    pub(crate) width: f32,
    pub(crate) layout: StripLayout,
}

impl Strip {
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        y >= self.top
            && y < self.top + self.height
            && x >= self.layout.left
            && x < self.layout.left + self.width
    }
}

fn split_on(app: &App) -> bool {
    app.ui.split.is_active() && !app.ui.split.panes.is_empty()
}

/// Tab width for `count` tabs in `width`: shared evenly, within the
/// minimum and maximum.
pub(crate) fn tab_width(width: f32, count: usize, scale: f32) -> f32 {
    (width / count.max(1) as f32).clamp(TAB_MIN_WIDTH * scale, TAB_MAX_WIDTH * scale)
}

fn strip(app: &App, group: Option<usize>, [x, top, w, h]: [f32; 4]) -> Strip {
    // Only a strip in the window's tab row meets the window controls.
    let left = if top < 0.5 {
        x.max(app.documents.strip.controls_inset)
    } else {
        x
    };
    let width = (x + w - left).max(0.0);
    let count = count(app, group);
    let scale = app.scale_factor;
    Strip {
        group,
        top,
        height: h,
        width,
        layout: StripLayout::new(left, width, tab_width(width, count, scale), count, scale),
    }
}

/// Every strip in the window, left to right and top to bottom.
pub(crate) fn all(app: &App) -> Vec<Strip> {
    let height = app.tab_bar_height();
    if height <= 0.0 {
        return Vec::new();
    }
    if !split_on(app) {
        let (left, width) = super::span(app);
        return vec![strip(app, None, [left, 0.0, width, height])];
    }
    crate::split::groups::rects(app)
        .iter()
        .take(app.ui.split.panes.len())
        .enumerate()
        .map(|(index, group)| strip(app, Some(index), group.strip))
        .collect()
}

/// The strip under `(x, y)`.
pub(crate) fn at(app: &App, x: f32, y: f32) -> Option<Strip> {
    all(app).into_iter().find(|s| s.contains(x, y))
}

/// The focused group's strip, or the only one.
pub(crate) fn focused(app: &App) -> Option<Strip> {
    let strips = all(app);
    let group = split_on(app).then_some(app.ui.split.active_pane);
    strips
        .iter()
        .find(|s| s.group == group)
        .or(strips.first())
        .copied()
}

/// A strip's tabs, in its order. Allocates: per-event paths use
/// [`count`], [`tab_at_index`], and [`position`] instead.
pub(crate) fn order(app: &App, group: Option<usize>) -> Vec<TabId> {
    match group.and_then(|g| app.ui.split.panes.get(g)) {
        Some(pane) => pane.tabs.clone(),
        None => app.documents.tabs.tabs().iter().map(|t| t.id).collect(),
    }
}

/// How many tabs a strip holds, without copying them.
pub(crate) fn count(app: &App, group: Option<usize>) -> usize {
    match group.and_then(|g| app.ui.split.panes.get(g)) {
        Some(pane) => pane.tabs.len(),
        None => app.documents.tabs.tabs().len(),
    }
}

/// The tab at `index` in a strip's order.
pub(crate) fn tab_at_index(app: &App, group: Option<usize>, index: usize) -> Option<TabId> {
    match group.and_then(|g| app.ui.split.panes.get(g)) {
        Some(pane) => pane.tabs.get(index).copied(),
        None => app.documents.tabs.tabs().get(index).map(|t| t.id),
    }
}

/// Where `id` is in a strip's order.
pub(crate) fn position(app: &App, group: Option<usize>, id: TabId) -> Option<usize> {
    match group.and_then(|g| app.ui.split.panes.get(g)) {
        Some(pane) => pane.tabs.iter().position(|t| *t == id),
        None => app.documents.tabs.position(id),
    }
}

/// The tab a strip shows as selected.
pub(crate) fn shown(app: &App, group: Option<usize>) -> Option<TabId> {
    match group.and_then(|g| app.ui.split.panes.get(g)) {
        Some(pane) => Some(pane.tab_id),
        None => app.documents.active_tab,
    }
}

pub(crate) fn scroll(app: &App, group: Option<usize>) -> f32 {
    match group.and_then(|g| app.ui.split.panes.get(g)) {
        Some(pane) => pane.strip_scroll,
        None => app.documents.tabs.scroll(),
    }
}

pub(crate) fn set_scroll(app: &mut App, group: Option<usize>, value: f32) {
    match group.and_then(|g| app.ui.split.panes.get_mut(g)) {
        Some(pane) => pane.strip_scroll = value,
        None => app.documents.tabs.set_scroll(value),
    }
}

pub(crate) fn revealed(app: &App, group: Option<usize>) -> Option<TabId> {
    match group.and_then(|g| app.ui.split.panes.get(g)) {
        Some(pane) => pane.revealed,
        None => app.documents.strip.revealed,
    }
}

pub(crate) fn set_revealed(app: &mut App, group: Option<usize>, tab: Option<TabId>) {
    match group.and_then(|g| app.ui.split.panes.get_mut(g)) {
        Some(pane) => pane.revealed = tab,
        None => app.documents.strip.revealed = tab,
    }
}

/// What the renderer needs to paint one strip.
#[derive(Debug, Clone)]
pub(crate) struct StripPaint {
    pub(crate) strip: Strip,
    pub(crate) ids: Vec<TabId>,
    pub(crate) shown: Option<TabId>,
    pub(crate) scroll: f32,
    /// The focused group's strip (always, without a split).
    pub(crate) focused: bool,
    /// Where a moving drag would drop, when it would drop here.
    pub(crate) drop_slot: Option<usize>,
}

/// Every strip with what it shows, for one frame.
pub(crate) fn paint_list(app: &App) -> Vec<StripPaint> {
    let target = super::drop_target(app);
    let active = split_on(app).then_some(app.ui.split.active_pane);
    all(app)
        .into_iter()
        .map(|strip| StripPaint {
            strip,
            ids: order(app, strip.group),
            shown: shown(app, strip.group),
            scroll: scroll(app, strip.group),
            focused: strip.group == active,
            drop_slot: target
                .filter(|(t, _)| t.group == strip.group)
                .map(|(_, slot)| slot),
        })
        .collect()
}
