//! Tab strip behavior that is not a single tab operation: overflow
//! geometry, keeping the active tab in view, the drag gesture with its
//! insertion marker, and the `▼` list of open tabs (ST-TAB-01, ST-TAB-02).

pub(crate) mod list;

#[cfg(test)]
mod inset_tests;
#[cfg(test)]
mod tests;

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey};
use lux_ui::tabs::TabId;

use crate::app::{App, TAB_CLOSE_BTN_WIDTH};

/// Width of the `▼` button at the right end of an overflowing strip.
pub(crate) const OVERFLOW_BUTTON_WIDTH: f32 = 28.0;
/// Pointer travel before a press on a tab becomes a drag.
const DRAG_THRESHOLD: f32 = 6.0;

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
}

/// A press on a tab, and the drag it becomes once the pointer moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TabDrag {
    pub(crate) id: TabId,
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

/// `(left, width)` of the strip, after the sidebar and window controls.
pub(crate) fn span(app: &App) -> (f32, f32) {
    crate::title_bar::strip_span(
        app.sidebar_width(),
        app.viewport.width,
        app.documents.strip.controls_inset,
    )
}

pub(crate) fn layout(app: &App) -> StripLayout {
    let (left, width) = span(app);
    StripLayout::new(
        left,
        width,
        app.tab_width(),
        app.documents.tabs.tabs().len(),
        app.scale_factor,
    )
}

/// What a press at `mouse_x` in the tab row lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowHit {
    Overflow,
    /// Tab index, and whether the press is on its close mark.
    Tab(usize, bool),
    /// No tab and no button: title-bar space.
    Empty,
}

pub(crate) fn row_hit(app: &App, mouse_x: f32) -> RowHit {
    if layout(app).on_button(mouse_x) {
        RowHit::Overflow
    } else if let Some((index, on_close)) = tab_at(app, mouse_x) {
        RowHit::Tab(index, on_close)
    } else {
        RowHit::Empty
    }
}

/// The tab under `mouse_x`, and whether the pointer is on its close mark.
pub(crate) fn tab_at(app: &App, mouse_x: f32) -> Option<(usize, bool)> {
    let strip = layout(app);
    if strip.count == 0 || mouse_x < strip.left || mouse_x >= strip.right() {
        return None;
    }
    let width = strip.tab_width.max(1.0);
    let local = mouse_x - strip.left + app.documents.tabs.scroll();
    let index = (local / width) as usize;
    if index >= strip.count {
        return None;
    }
    let close_w = TAB_CLOSE_BTN_WIDTH * app.scale_factor;
    let on_close = local - index as f32 * width >= width - close_w;
    Some((index, on_close))
}

pub(crate) fn scroll_tabs(app: &mut App, delta_x: f32) {
    let max_scroll = layout(app).max_scroll();
    let next = (app.documents.tabs.scroll() - delta_x).clamp(0.0, max_scroll);
    app.documents.tabs.set_scroll(next);
    app.scheduler.invalidate_all();
}

/// Scrolls the strip the least distance that shows all of tab `id`.
pub(crate) fn reveal(app: &mut App, id: TabId) {
    let strip = layout(app);
    let Some(index) = app.documents.tabs.tabs().iter().position(|t| t.id == id) else {
        return;
    };
    let left = index as f32 * strip.tab_width;
    let right = left + strip.tab_width;
    let mut scroll = app.documents.tabs.scroll();
    if left < scroll {
        scroll = left;
    } else if right > scroll + strip.visible_width {
        scroll = right - strip.visible_width;
    }
    app.documents
        .tabs
        .set_scroll(scroll.clamp(0.0, strip.max_scroll()));
}

/// Run before each frame is painted. Every path that activates a tab
/// (shortcuts, Goto Anything, opening a file, closing a neighbor, the
/// `▼` list) ends here, so none of them has to remember to scroll. Also
/// clamps a scroll left past the end by closed tabs or a wider window.
pub(crate) fn follow_active(app: &mut App) {
    let active = app.documents.active_tab;
    if active != app.documents.strip.revealed {
        if let Some(id) = active {
            reveal(app, id);
        }
        app.documents.strip.revealed = active;
    }
    let max_scroll = layout(app).max_scroll();
    if app.documents.tabs.scroll() > max_scroll {
        app.documents.tabs.set_scroll(max_scroll);
    }
}

/// Left press on a tab body: remember it as a possible drag.
pub(crate) fn press_tab(app: &mut App, id: TabId, index: usize, mouse_x: f32) {
    let strip = layout(app);
    let tab_left = strip.left + index as f32 * strip.tab_width - app.documents.tabs.scroll();
    app.documents.strip.drag = Some(TabDrag {
        id,
        press_x: mouse_x,
        pointer_x: mouse_x,
        grab_offset: mouse_x - tab_left,
        moving: false,
    });
}

/// Pointer moved with a tab pressed. The order does not change until
/// release; the strip only redraws the floating tab and the marker.
pub(crate) fn drag_tab(app: &mut App) -> bool {
    let mouse_x = app.mouse.position.x as f32;
    let Some(drag) = app.documents.strip.drag.as_mut() else {
        return false;
    };
    if !drag.moving && (mouse_x - drag.press_x).abs() < DRAG_THRESHOLD {
        return false;
    }
    drag.moving = true;
    drag.pointer_x = mouse_x;
    app.scheduler.invalidate_all();
    true
}

/// Insertion gap of the current drag, if the pointer has moved.
pub(crate) fn drop_slot(app: &App) -> Option<usize> {
    let drag = app.documents.strip.drag.filter(|d| d.moving)?;
    Some(layout(app).drop_slot(app.documents.tabs.scroll(), drag.pointer_x))
}

/// Button released: a moving drag commits at its marker.
pub(crate) fn release_drag(app: &mut App) {
    let slot = drop_slot(app);
    let Some(drag) = app.documents.strip.drag.take() else {
        return;
    };
    let Some(slot) = slot else {
        return;
    };
    let Some(from) = app
        .documents
        .tabs
        .tabs()
        .iter()
        .position(|t| t.id == drag.id)
    else {
        return;
    };
    let to = if slot > from { slot - 1 } else { slot };
    if to != from {
        app.reorder_tab(drag.id, to);
    } else {
        app.scheduler.invalidate_all();
    }
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
