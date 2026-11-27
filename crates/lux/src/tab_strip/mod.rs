//! Tab strip behavior that is not a single tab operation: overflow
//! geometry, keeping the active tab in view, the drag gesture with its
//! insertion marker, and the `▼` list of open tabs (ST-TAB-01, ST-TAB-02).

mod drag;
pub(crate) mod list;
pub(crate) mod strips;
pub(crate) mod title;

#[cfg(test)]
pub(crate) use drag::drop_slot;
pub(crate) use drag::{drag_tab, drop_target, press_tab, release_drag};
pub(crate) use strips::Strip;

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod hover_tests;
#[cfg(test)]
mod inset_tests;
#[cfg(test)]
pub(crate) mod tests;

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey};
use lux_ui::tabs::TabId;

use crate::app::{App, TAB_CLOSE_BTN_WIDTH};

/// Width of the `▼` button at the right end of an overflowing strip.
pub(crate) const OVERFLOW_BUTTON_WIDTH: f32 = 28.0;

/// Strip state that lives beside the tab order itself.
#[derive(Default)]
pub(crate) struct StripState {
    pub(crate) drag: Option<TabDrag>,
    pub(crate) list: Option<list::TabListState>,
    /// The active tab the strip last scrolled into view. A different
    /// active tab is revealed on the next frame; the same one is left
    /// wherever the wheel put it.
    revealed: Option<TabId>,
    /// Goto Anything's transient preview tab (ST-NAV-01).
    pub(crate) preview: crate::goto::preview::PreviewTab,
    /// Physical pixels the window controls take at the row's leading end
    /// (macOS traffic lights; zero in fullscreen and on other platforms).
    pub(crate) controls_inset: f32,
    /// The tab under the pointer. Set on pointer moves, and re-derived
    /// from the last pointer position before each frame, so a wheel
    /// scroll, a close, or a reorder never leaves it naming a tab that
    /// moved away. Never timed.
    pub(crate) hover: Option<Hover>,
}

/// The tab under the pointer: in which strip, and whether the pointer is
/// on its close mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hover {
    pub(crate) group: Option<usize>,
    pub(crate) tab: TabId,
    pub(crate) on_close: bool,
}

/// A press on a tab, and the drag it becomes once the pointer moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TabDrag {
    pub(crate) id: TabId,
    /// The strip the tab was pressed in.
    pub(crate) group: Option<usize>,
    pub(crate) pointer_y: f32,
    pub(crate) press_x: f32,
    pub(crate) pointer_x: f32,
    /// Pointer x minus the tab's left edge at the press.
    pub(crate) grab_offset: f32,
    pub(crate) moving: bool,
}

/// Where the tabs sit in the window, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StripLayout {
    pub(crate) left: f32,
    /// Width the tabs may use: the strip minus the `▼` button when shown.
    pub(crate) visible_width: f32,
    pub(crate) tab_width: f32,
    pub(crate) count: usize,
    /// `[x, width]` of the `▼` button, only when the tabs overflow.
    pub(crate) button: Option<[f32; 2]>,
}

impl StripLayout {
    pub(crate) fn new(
        left: f32,
        strip_width: f32,
        tab_width: f32,
        count: usize,
        scale: f32,
    ) -> Self {
        let overflow = count as f32 * tab_width > strip_width;
        let button_w = OVERFLOW_BUTTON_WIDTH * scale;
        let (visible_width, button) = if overflow {
            let visible = (strip_width - button_w).max(0.0);
            (visible, Some([left + visible, button_w]))
        } else {
            (strip_width, None)
        };
        Self {
            left,
            visible_width,
            tab_width,
            count,
            button,
        }
    }

    pub(crate) fn max_scroll(&self) -> f32 {
        (self.count as f32 * self.tab_width - self.visible_width).max(0.0)
    }

    pub(crate) fn right(&self) -> f32 {
        self.left + self.visible_width
    }

    /// Gap between tabs the pointer is closest to: 0 is before the first
    /// tab, `count` after the last.
    pub(crate) fn drop_slot(&self, scroll: f32, pointer_x: f32) -> usize {
        let local = (pointer_x - self.left + scroll) / self.tab_width.max(1.0);
        (local.round().max(0.0) as usize).min(self.count)
    }

    pub(crate) fn on_button(&self, x: f32) -> bool {
        self.button
            .is_some_and(|[left, width]| x >= left && x < left + width)
    }
}

/// `(left, width)` of the single strip, after the sidebar and window
/// controls.
pub(crate) fn span(app: &App) -> (f32, f32) {
    crate::title_bar::strip_span(
        app.sidebar_width(),
        app.viewport.width,
        app.documents.strip.controls_inset,
    )
}

/// The focused strip's layout (the only strip without a split).
pub(crate) fn layout(app: &App) -> StripLayout {
    if let Some(strip) = strips::focused(app) {
        return strip.layout;
    }
    let (left, width) = span(app);
    StripLayout::new(
        left,
        width,
        app.tab_width(),
        app.documents.tabs.tabs().len(),
        app.scale_factor,
    )
}

/// What a press in a tab row lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowHit {
    Overflow,
    /// Tab index in its strip, and whether the press is on its close mark.
    Tab(usize, bool),
    /// No tab and no button: title-bar space.
    Empty,
}

/// What `(x, y)` lands on, if it is in a tab row at all: the strip (none
/// for the window's title row outside every strip) and the hit.
pub(crate) fn hit(app: &App, x: f32, y: f32) -> Option<(Option<Strip>, RowHit)> {
    if let Some(strip) = strips::at(app, x, y) {
        let hit = if strip.layout.on_button(x) {
            RowHit::Overflow
        } else if let Some((index, on_close)) = tab_in(app, &strip, x) {
            RowHit::Tab(index, on_close)
        } else {
            RowHit::Empty
        };
        return Some((Some(strip), hit));
    }
    (y >= 0.0 && y < app.tab_bar_height()).then_some((None, RowHit::Empty))
}

/// The hit at `mouse_x` in the window's tab row.
#[cfg(test)]
pub(crate) fn row_hit(app: &App, mouse_x: f32) -> RowHit {
    hit(app, mouse_x, 1.0).map_or(RowHit::Empty, |(_, hit)| hit)
}

/// The tab under `mouse_x` in the window's tab row, and whether the
/// pointer is on its close mark.
#[cfg(test)]
pub(crate) fn tab_at(app: &App, mouse_x: f32) -> Option<(usize, bool)> {
    let strip = strips::at(app, mouse_x, 1.0)?;
    tab_in(app, &strip, mouse_x)
}

/// The tab of `strip` under `mouse_x`, and whether it is on the close mark.
pub(crate) fn tab_in(app: &App, strip: &Strip, mouse_x: f32) -> Option<(usize, bool)> {
    let layout = strip.layout;
    if layout.count == 0 || mouse_x < layout.left || mouse_x >= layout.right() {
        return None;
    }
    let width = layout.tab_width.max(1.0);
    let local = mouse_x - layout.left + strips::scroll(app, strip.group);
    let index = (local / width) as usize;
    if index >= layout.count {
        return None;
    }
    let close_w = TAB_CLOSE_BTN_WIDTH * app.scale_factor;
    let on_close = local - index as f32 * width >= width - close_w;
    Some((index, on_close))
}

/// The tab id at `index` in `strip`.
pub(crate) fn tab_id(app: &App, strip: &Strip, index: usize) -> Option<TabId> {
    strips::tab_at_index(app, strip.group, index)
}

/// The wheel over a strip scrolls that strip.
pub(crate) fn scroll_tabs(app: &mut App, delta_x: f32) {
    let (x, y) = (app.mouse.position.x as f32, app.mouse.position.y as f32);
    let Some(strip) = strips::at(app, x, y).or_else(|| strips::focused(app)) else {
        return;
    };
    let max_scroll = strip.layout.max_scroll();
    let next = (strips::scroll(app, strip.group) - delta_x).clamp(0.0, max_scroll);
    strips::set_scroll(app, strip.group, next);
    app.scheduler.invalidate_all();
}

/// Scrolls `strip` the least distance that shows all of tab `id`.
fn reveal_in(app: &mut App, strip: &Strip, id: TabId) {
    let Some(index) = strips::position(app, strip.group, id) else {
        return;
    };
    let layout = strip.layout;
    let left = index as f32 * layout.tab_width;
    let right = left + layout.tab_width;
    let mut scroll = strips::scroll(app, strip.group);
    if left < scroll {
        scroll = left;
    } else if right > scroll + layout.visible_width {
        scroll = right - layout.visible_width;
    }
    strips::set_scroll(app, strip.group, scroll.clamp(0.0, layout.max_scroll()));
}

/// Run before each frame is painted. Every path that changes the tab a
/// strip shows (shortcuts, Goto Anything, opening a file, closing a
/// neighbor, the `▼` list, a click in another group) ends here, so none
/// of them has to remember to scroll. Also clamps a scroll left past the
/// end by closed tabs or a wider window.
pub(crate) fn follow_active(app: &mut App) {
    crate::split::tabs::reconcile(app);
    for strip in strips::all(app) {
        let shown = strips::shown(app, strip.group);
        if shown != strips::revealed(app, strip.group) {
            if let Some(id) = shown {
                reveal_in(app, &strip, id);
            }
            strips::set_revealed(app, strip.group, shown);
        }
        let max_scroll = strip.layout.max_scroll();
        if strips::scroll(app, strip.group) > max_scroll {
            strips::set_scroll(app, strip.group, max_scroll);
        }
    }
    // The frame is about to be painted, so no invalidation is needed.
    app.documents.strip.hover = if app.mouse.placed {
        hover_at_pointer(app)
    } else {
        None
    };
}

/// A pointer move: updates the hovered tab and any tab drag. Returns
/// whether a strip needs a redraw.
pub(crate) fn pointer_moved(app: &mut App) -> bool {
    let hover_changed = update_hover(app);
    drag_tab(app) || hover_changed
}

fn update_hover(app: &mut App) -> bool {
    let hover = hover_at_pointer(app);
    set_hover(app, hover)
}

/// The tab under the last pointer position in the strips as they are
/// now, or `None` while the pointer is on no tab.
fn hover_at_pointer(app: &App) -> Option<Hover> {
    let (x, y) = (app.mouse.position.x as f32, app.mouse.position.y as f32);
    let strip = strips::at(app, x, y)?;
    let (index, on_close) = tab_in(app, &strip, x)?;
    Some(Hover {
        group: strip.group,
        tab: tab_id(app, &strip, index)?,
        on_close,
    })
}

/// The pointer left the window: nothing is hovered.
pub(crate) fn clear_hover(app: &mut App) -> bool {
    set_hover(app, None)
}

fn set_hover(app: &mut App, hover: Option<Hover>) -> bool {
    if app.documents.strip.hover == hover {
        return false;
    }
    app.documents.strip.hover = hover;
    app.scheduler.invalidate_all();
    true
}

/// Escape cancels a moving drag; the open `▼` list owns the keyboard.
pub(crate) fn handle_key(app: &mut App, event: &KeyEvent) -> bool {
    if matches!(event.key, Key::Named(NamedKey::Escape))
        && app.documents.strip.drag.is_some_and(|d| d.moving)
    {
        app.documents.strip.drag = None;
        app.scheduler.invalidate_all();
        return true;
    }
    list::handle_key(app, event)
}
