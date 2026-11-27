//! Dragging a tab: within its strip it reorders; onto another group's
//! strip it moves the tab to that group at the marker (ST-TAB-02).

use lux_ui::tabs::TabId;

use super::{Strip, TabDrag, strips};
use crate::app::App;

/// Pointer travel before a press on a tab becomes a drag.
const DRAG_THRESHOLD: f32 = 6.0;

/// Left press on a tab body: remember it as a possible drag. The strip is
/// the one under the pointer, or the focused one.
pub(crate) fn press_tab(app: &mut App, id: TabId, index: usize, mouse_x: f32) {
    let mouse_y = app.mouse.position.y as f32;
    let Some(strip) = strips::at(app, mouse_x, mouse_y).or_else(|| strips::focused(app)) else {
        return;
    };
    let layout = strip.layout;
    let tab_left = layout.left + index as f32 * layout.tab_width - strips::scroll(app, strip.group);
    app.documents.strip.drag = Some(TabDrag {
        id,
        group: strip.group,
        pointer_y: mouse_y,
        press_x: mouse_x,
        pointer_x: mouse_x,
        grab_offset: mouse_x - tab_left,
        moving: false,
    });
}

/// Pointer moved with a tab pressed. Nothing moves until release; the
/// strips only redraw the floating tab and the marker.
pub(crate) fn drag_tab(app: &mut App) -> bool {
    let mouse_x = app.mouse.position.x as f32;
    let mouse_y = app.mouse.position.y as f32;
    let Some(drag) = app.documents.strip.drag.as_mut() else {
        return false;
    };
    if !drag.moving && (mouse_x - drag.press_x).abs() < DRAG_THRESHOLD {
        return false;
    }
    drag.moving = true;
    drag.pointer_x = mouse_x;
    drag.pointer_y = mouse_y;
    app.scheduler.invalidate_all();
    true
}

/// The strip a moving drag would drop into, and the gap in it: the strip
/// under the pointer, or the strip it came from when the pointer is
/// outside every strip.
pub(crate) fn drop_target(app: &App) -> Option<(Strip, usize)> {
    let drag = app.documents.strip.drag.filter(|d| d.moving)?;
    let all = strips::all(app);
    let strip = all
        .iter()
        .find(|s| s.contains(drag.pointer_x, drag.pointer_y))
        .or_else(|| all.iter().find(|s| s.group == drag.group))?;
    let slot = strip
        .layout
        .drop_slot(strips::scroll(app, strip.group), drag.pointer_x);
    Some((*strip, slot))
}

/// Insertion gap of the current drag, if the pointer has moved.
pub(crate) fn drop_slot(app: &App) -> Option<usize> {
    drop_target(app).map(|(_, slot)| slot)
}

/// Button released: a moving drag commits at its marker.
pub(crate) fn release_drag(app: &mut App) {
    let target = drop_target(app);
    let Some(drag) = app.documents.strip.drag.take() else {
        return;
    };
    let Some((strip, slot)) = target else {
        return;
    };
    match (drag.group, strip.group) {
        (Some(from), Some(to)) => {
            crate::split::tabs::move_tab(app, drag.id, from, to, Some(slot));
            crate::session::note_change(app);
        }
        _ => reorder(app, drag.id, slot),
    }
    app.scheduler.invalidate_all();
}

fn reorder(app: &mut App, id: TabId, slot: usize) {
    let Some(from) = app.documents.tabs.tabs().iter().position(|t| t.id == id) else {
        return;
    };
    let to = if slot > from { slot - 1 } else { slot };
    if to != from {
        app.reorder_tab(id, to);
    }
}
