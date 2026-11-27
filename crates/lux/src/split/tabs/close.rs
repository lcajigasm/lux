//! Closing in a split (ST-SPLIT-05). As in Sublime Text, a group's tab is
//! a view of a document: closing it where another group also lists the
//! document only removes that view, whether by `×`, middle-click, or
//! Command+W / File › Close. The document closes, with the unsaved prompt
//! as usual, only when its last view closes.

use lux_ui::tabs::TabId;

use super::{enter_pane, remove_group, show_in, split_on};
use crate::app::App;

/// Whether `tab` is listed by a group other than `group`.
fn listed_elsewhere(app: &App, group: usize, tab: TabId) -> bool {
    app.ui
        .split
        .panes
        .iter()
        .enumerate()
        .any(|(i, p)| i != group && p.tabs.contains(&tab))
}

/// Command+W or File › Close on the editor's document. When another group
/// also lists it, only the focused group's view closes and this returns
/// true; otherwise the caller closes the document.
pub(crate) fn close_active_view(app: &mut App) -> bool {
    if !split_on(app) {
        return false;
    }
    let group = app.ui.split.active_pane;
    let Some(tab) = app.documents.active_tab else {
        return false;
    };
    if !listed_elsewhere(app, group, tab) {
        return false;
    }
    drop_view(app, group, tab);
    true
}

/// Tab `closed` was closed while not the editor's document. Each group
/// that showed it shows its neighbor in that group's strip, or closes.
pub(crate) fn forget(app: &mut App, closed: TabId) {
    if !split_on(app) {
        return;
    }
    let mut idx = app.ui.split.panes.len();
    while idx > 0 {
        idx -= 1;
        // A closed group may have taken the split down to one group.
        let Some(pane) = app.ui.split.panes.get_mut(idx) else {
            continue;
        };
        let Some(at) = pane.tabs.iter().position(|id| *id == closed) else {
            continue;
        };
        pane.tabs.remove(at);
        if pane.tab_id != closed {
            continue;
        }
        match neighbor(&pane.tabs, at) {
            Some(next) => show_in(app, idx, next),
            None => remove_group(app, idx),
        }
    }
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
}

pub(super) fn neighbor(tabs: &[TabId], removed_at: usize) -> Option<TabId> {
    tabs.get(removed_at.min(tabs.len().saturating_sub(1)))
        .copied()
}

/// The editor's document `closed` was just closed. Every other group lets
/// go of it first, so no group is left showing a closed document (and a
/// group that inherits tabs never brings it back). Then the focused group
/// shows its neighbor; a group left empty closes and the group that takes
/// its place gets focus. Returns false without a split, for the caller's
/// global fallback.
pub(crate) fn after_active_closed(app: &mut App, closed: TabId) -> bool {
    if !split_on(app) {
        return false;
    }
    let active = app.ui.split.active_pane.min(app.ui.split.panes.len() - 1);
    let at = app.ui.split.panes[active]
        .tabs
        .iter()
        .position(|id| *id == closed)
        .unwrap_or(0);
    app.ui.split.panes[active].tabs.retain(|id| *id != closed);
    app.documents.active_tab = None;
    // The focused group no longer lists `closed`, so this only touches
    // the others. Closing one of them keeps `active_pane` on this group.
    forget(app, closed);
    if !split_on(app) {
        if let Some(tab) = app.documents.tabs.active().map(|t| t.id) {
            app.switch_to_tab(tab);
        }
        return true;
    }
    let active = app.ui.split.active_pane;
    match neighbor(&app.ui.split.panes[active].tabs, at) {
        Some(next) => {
            app.ui.split.panes[active].tab_id = next;
            app.switch_to_tab(next);
        }
        None => {
            remove_group(app, active);
            if split_on(app) {
                let next = app.ui.split.active_pane.min(app.ui.split.panes.len() - 1);
                enter_pane(app, next);
            } else if let Some(tab) = app.documents.tabs.tabs().first().map(|t| t.id) {
                app.switch_to_tab(tab);
            }
        }
    }
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
    true
}

/// A click on tab `tab` in the strip of `group`: shows it there and
/// focuses that group.
pub(crate) fn select(app: &mut App, group: Option<usize>, tab: TabId) {
    crate::session::note_change(app);
    match group {
        Some(g)
            if split_on(app) && g != app.ui.split.active_pane && g < app.ui.split.panes.len() =>
        {
            if app.ui.split.panes[g].tab_id != tab {
                app.sync_active_document_to_entry();
                show_in(app, g, tab);
            }
            app.focus_group(g);
        }
        _ => {
            app.switch_to_tab(tab);
        }
    }
}

/// The close mark of `tab` in the strip of `group`. A tab that another
/// group also lists only leaves this group; otherwise the tab closes,
/// asking first when it has unsaved changes.
pub(crate) fn close_in_group(
    app: &mut App,
    ctx: &dyn lux_platform::PlatformContext,
    group: Option<usize>,
    tab: TabId,
) {
    match group {
        Some(g) if split_on(app) && listed_elsewhere(app, g, tab) => drop_view(app, g, tab),
        _ => app.close_tab(ctx, tab),
    }
}

/// Takes `tab` out of group `g` only; the document stays open in the
/// other group that lists it.
fn drop_view(app: &mut App, g: usize, tab: TabId) {
    crate::session::note_change(app);
    let pane = &mut app.ui.split.panes[g];
    let at = pane.tabs.iter().position(|id| *id == tab).unwrap_or(0);
    pane.tabs.retain(|id| *id != tab);
    if pane.tab_id != tab {
        app.scheduler.invalidate_all();
        return;
    }
    let focused = g == app.ui.split.active_pane;
    match neighbor(&app.ui.split.panes[g].tabs, at) {
        Some(next) if focused => {
            app.ui.split.panes[g].tab_id = next;
            app.switch_to_tab(next);
        }
        Some(next) => {
            app.sync_active_document_to_entry();
            show_in(app, g, next);
        }
        None => {
            app.sync_active_document_to_entry();
            remove_group(app, g);
            if focused && split_on(app) {
                app.documents.active_tab = None;
                let next = app.ui.split.active_pane.min(app.ui.split.panes.len() - 1);
                enter_pane(app, next);
            }
        }
    }
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
    app.scheduler.invalidate_all();
}
