//! Each split group's own tabs (ST-SPLIT-05). A group keeps an ordered
//! list of tabs and shows one of them; files open into the focused group;
//! activating a tab that lives in another group focuses that group. The
//! document registry (`Documents::tabs`, the entries) stays global: a
//! group's list says where a tab is shown, not what is open.

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_ui::tabs::TabId;

use crate::app::App;
use crate::pane_view::PaneView;
use crate::split::{LayoutPreset, SplitDirection};

/// Open tabs, in the global order.
fn open_tabs(app: &App) -> Vec<TabId> {
    app.documents.tabs.tabs().iter().map(|t| t.id).collect()
}

fn split_on(app: &App) -> bool {
    app.ui.split.is_active() && !app.ui.split.panes.is_empty()
}

/// The tabs of the focused group, or every tab without a split: what
/// Command+1..9 and Control+Tab walk.
pub(crate) fn focused_order(app: &App) -> Vec<TabId> {
    if split_on(app) {
        if let Some(pane) = app.ui.split.panes.get(app.ui.split.active_pane) {
            return pane.tabs.clone();
        }
    }
    open_tabs(app)
}

/// A view of `tab` as its entry last left it, for a group that is not
/// focused.
pub(crate) fn pane_for_tab(app: &App, tab: TabId) -> PaneView {
    let mut pane = PaneView::new(tab);
    if app.documents.active_tab == Some(tab) {
        pane.cursors = app.cursors.clone();
        pane.fold_state = app.fold_state.clone();
        pane.bookmarks = app.bookmarks.clone();
        pane.viewport.scroll_x = app.viewport.scroll_x;
        pane.viewport.scroll_y = app.viewport.scroll_y;
        if let Some(syntax) = &app.syntax {
            pane.tokens.replace(syntax.tokens.clone());
        }
    } else if let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == tab) {
        pane.cursors = entry.cursors.clone();
        pane.fold_state = entry.fold_state.clone();
        pane.bookmarks = entry.bookmarks.clone();
        pane.viewport.scroll_x = entry.scroll_x;
        pane.viewport.scroll_y = entry.scroll_y;
        pane.tokens.replace(crate::session::pane_tokens(entry));
    }
    pane
}

/// Shows `tab` in group `idx` without focusing it.
fn show_in(app: &mut App, idx: usize, tab: TabId) {
    let view = pane_for_tab(app, tab);
    let pane = &mut app.ui.split.panes[idx];
    if !pane.tabs.contains(&tab) {
        pane.tabs.push(tab);
    }
    pane.tab_id = tab;
    pane.active_tab = tab;
    pane.cursors = view.cursors;
    pane.fold_state = view.fold_state;
    pane.bookmarks = view.bookmarks;
    pane.viewport.scroll_x = view.viewport.scroll_x;
    pane.viewport.scroll_y = view.viewport.scroll_y;
    pane.tokens = view.tokens;
}

/// Copies the editor's live view state into the focused group.
pub(crate) fn store_active_pane(app: &mut App) {
    let idx = app.ui.split.active_pane;
    let Some(pane) = app.ui.split.panes.get_mut(idx) else {
        return;
    };
    pane.cursors = app.cursors.clone();
    pane.fold_state = app.fold_state.clone();
    pane.bookmarks = app.bookmarks.clone();
    pane.viewport.scroll_x = app.viewport.scroll_x;
    pane.viewport.scroll_y = app.viewport.scroll_y;
    if let Some(syntax) = &app.syntax {
        pane.tokens.replace(syntax.tokens.clone());
    }
}

/// Focuses group `idx` and loads what it shows into the editor, without
/// first storing the editor's state anywhere.
pub(crate) fn enter_pane(app: &mut App, idx: usize) {
    app.ui.split.active_pane = idx;
    let Some(pane) = app.ui.split.panes.get(idx).cloned() else {
        return;
    };
    app.switch_to_tab(pane.tab_id);
    let end = Position::document_end(&app.buffer).char;
    let clamped: Vec<_> = pane
        .cursors
        .cursors()
        .iter()
        .map(|s| {
            Selection::new(
                Position::from_char(&app.buffer, s.anchor.char.min(end)),
                Position::from_char(&app.buffer, s.head.char.min(end)),
            )
        })
        .collect();
    app.cursors = MultiCursor::from_cursors(clamped);
    app.viewport.scroll_x = pane.viewport.scroll_x;
    app.viewport.scroll_y = pane.viewport.scroll_y;
    app.fold_state = pane.fold_state;
    app.bookmarks = pane.bookmarks;
    // The group's scroll is part of its view; do not reveal the caret.
    app.carets.goal_columns = app
        .cursors
        .cursors()
        .iter()
        .map(|s| s.head.column)
        .collect();
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
    app.scheduler.invalidate_all();
}

/// `switch_to_tab` calls this first. A tab that is not in the focused
/// group but is in another one is shown there and that group takes focus,
/// as in Sublime Text. Returns the switch's result when it redirected.
pub(crate) fn redirect(app: &mut App, target: TabId) -> Option<bool> {
    if !split_on(app) {
        return None;
    }
    let active = app.ui.split.active_pane;
    let here = app.ui.split.panes.get(active)?;
    if here.tab_id == target || here.tabs.contains(&target) {
        return None;
    }
    let owner = app
        .ui
        .split
        .panes
        .iter()
        .position(|p| p.tabs.contains(&target))?;
    if app.ui.split.panes[owner].tab_id != target {
        show_in(app, owner, target);
    }
    Some(app.focus_group(owner))
}

/// `switch_to_tab` made `target` the editor's document: the focused group
/// shows it, and a newly opened tab joins that group.
pub(crate) fn show_in_active_pane(app: &mut App, target: TabId) {
    if !split_on(app) {
        return;
    }
    let idx = app.ui.split.active_pane.min(app.ui.split.panes.len() - 1);
    let pane = &mut app.ui.split.panes[idx];
    pane.tab_id = target;
    pane.active_tab = target;
    if !pane.tabs.contains(&target) {
        let at = pane.tabs.len();
        pane.tabs.insert(at, target);
    }
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
}

/// Brings every group's list in line with what is open: closed tabs leave,
/// a shown tab is listed, and an open tab no group lists joins the focused
/// group. A group left with nothing closes. Runs before each frame and
/// after anything that opens or closes tabs.
pub(crate) fn reconcile(app: &mut App) {
    if !split_on(app) {
        return;
    }
    let open = open_tabs(app);
    let active = app.ui.split.active_pane.min(app.ui.split.panes.len() - 1);
    app.ui.split.active_pane = active;
    if let Some(tab) = app.documents.active_tab {
        let pane = &mut app.ui.split.panes[active];
        pane.tab_id = tab;
        pane.active_tab = tab;
    }
    for pane in &mut app.ui.split.panes {
        pane.tabs.retain(|id| open.contains(id));
        if open.contains(&pane.tab_id) && !pane.tabs.contains(&pane.tab_id) {
            pane.tabs.push(pane.tab_id);
        }
    }
    for id in &open {
        if !app.ui.split.panes.iter().any(|p| p.tabs.contains(id)) {
            app.ui.split.panes[active].tabs.push(*id);
        }
    }
    let mut idx = app.ui.split.panes.len();
    while idx > 0 {
        idx -= 1;
        let pane = &app.ui.split.panes[idx];
        if open.contains(&pane.tab_id) {
            continue;
        }
        match pane.tabs.first().copied() {
            Some(next) => show_in(app, idx, next),
            None => remove_group(app, idx),
        }
    }
}

/// The session restored each group's tabs. A tab no group lists (an older
/// session, or one whose file is gone) joins the first group, so the
/// focused group at restore does not collect them all.
pub(crate) fn restored(app: &mut App) {
    if !split_on(app) {
        return;
    }
    let open = open_tabs(app);
    for pane in &mut app.ui.split.panes {
        pane.tabs.retain(|id| open.contains(id));
        if !pane.tabs.contains(&pane.tab_id) {
            pane.tabs.push(pane.tab_id);
        }
    }
    for id in open {
        if !app.ui.split.panes.iter().any(|p| p.tabs.contains(&id)) {
            app.ui.split.panes[0].tabs.push(id);
        }
    }
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
        let pane = &mut app.ui.split.panes[idx];
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

fn neighbor(tabs: &[TabId], removed_at: usize) -> Option<TabId> {
    tabs.get(removed_at.min(tabs.len().saturating_sub(1)))
        .copied()
}

/// The editor's document `closed` was just closed. The focused group shows
/// its neighbor in that group; a group left empty closes and the group
/// that takes its place gets focus. Returns false without a split, for the
/// caller's global fallback.
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
    forget(app, closed);
    true
}

/// The preset with `count` groups in the same arrangement as `preset`.
fn preset_for(preset: LayoutPreset, count: usize) -> LayoutPreset {
    let rows = matches!(preset, LayoutPreset::Rows2 | LayoutPreset::Rows3);
    match (count, rows) {
        (0 | 1, _) => LayoutPreset::Single,
        (2, true) => LayoutPreset::Rows2,
        (_, true) => LayoutPreset::Rows3,
        (2, false) => LayoutPreset::Columns2,
        (3, false) => LayoutPreset::Columns3,
        _ => preset,
    }
}

/// Closes group `idx`. Its tabs, if any, join the group before it (or the
/// next one). The focused index follows the group it pointed at; when
/// that was `idx`, the caller loads the new focus.
pub(crate) fn remove_group(app: &mut App, idx: usize) {
    if idx >= app.ui.split.panes.len() {
        return;
    }
    let removed = app.ui.split.panes.remove(idx);
    let count = app.ui.split.panes.len();
    if count > 0 {
        let heir = idx.saturating_sub(1).min(count - 1);
        merge_tabs(&mut app.ui.split.panes[heir].tabs, &removed.tabs);
    }
    if app.ui.split.active_pane > idx {
        app.ui.split.active_pane -= 1;
    }
    app.ui.split.active_pane = app.ui.split.active_pane.min(count.saturating_sub(1));
    let preset = preset_for(app.ui.split.preset, count);
    if preset == LayoutPreset::Single {
        collapse_to_single(app);
    } else {
        app.ui.split.preset = preset;
    }
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
    app.update_viewport_dimensions();
    app.scheduler.invalidate_all();
}

fn merge_tabs(into: &mut Vec<TabId>, from: &[TabId]) {
    for id in from {
        if !into.contains(id) {
            into.push(*id);
        }
    }
}

/// Leaves the split. The single strip takes the groups' orders one after
/// another, so tabs stay where the user put them.
fn collapse_to_single(app: &mut App) {
    let mut order = Vec::new();
    for pane in &app.ui.split.panes {
        merge_tabs(&mut order, &pane.tabs);
    }
    for (to, id) in order.into_iter().enumerate() {
        app.documents.tabs.move_tab(id, to);
    }
    app.ui.split.preset = LayoutPreset::Single;
    app.ui.split.secondary = None;
    app.ui.split.panes.clear();
    app.ui.split.active_pane = 0;
}

/// A group added to the split takes a tab from the focused group that no
/// group shows, or else a second view of the focused group's tab.
fn new_group(app: &mut App) -> PaneView {
    let donor = app.ui.split.active_pane.min(app.ui.split.panes.len() - 1);
    let shown: Vec<TabId> = app.ui.split.panes.iter().map(|p| p.tab_id).collect();
    let spare = app.ui.split.panes[donor]
        .tabs
        .iter()
        .copied()
        .find(|id| !shown.contains(id));
    match spare {
        Some(tab) => {
            app.ui.split.panes[donor].tabs.retain(|id| *id != tab);
            pane_for_tab(app, tab)
        }
        None => pane_for_tab(app, app.ui.split.panes[donor].tab_id),
    }
}

impl App {
    pub(crate) fn set_layout(&mut self, preset: LayoutPreset) {
        crate::session::note_change(self);
        if self.documents.active_tab.is_none() && self.documents.tabs.tabs().is_empty() {
            return;
        }
        if preset == LayoutPreset::Single {
            if split_on(self) {
                store_active_pane(self);
                collapse_to_single(self);
            }
            self.ui.split.preset = LayoutPreset::Single;
            self.ui.split.secondary = None;
            self.ui.split.panes.clear();
            self.ui.split.active_pane = 0;
            self.update_viewport_dimensions();
            self.scheduler.invalidate_all();
            return;
        }
        if self.ui.split.panes.is_empty() {
            let tab = self
                .documents
                .active_tab
                .unwrap_or_else(|| self.documents.tabs.tabs()[0].id);
            let mut first = pane_for_tab(self, tab);
            first.tabs = open_tabs(self);
            self.ui.split.panes = vec![first];
            self.ui.split.active_pane = 0;
        } else {
            reconcile(self);
            store_active_pane(self);
        }
        let needed = preset.num_panes();
        while self.ui.split.panes.len() > needed {
            let extra = self.ui.split.panes.pop().expect("more panes than needed");
            let last = self.ui.split.panes.len() - 1;
            merge_tabs(&mut self.ui.split.panes[last].tabs, &extra.tabs);
        }
        while self.ui.split.panes.len() < needed {
            let pane = new_group(self);
            self.ui.split.panes.push(pane);
        }
        self.ui.split.preset = preset;
        self.ui.split.direction = match preset {
            LayoutPreset::Rows2 | LayoutPreset::Rows3 => SplitDirection::Vertical,
            _ => SplitDirection::Horizontal,
        };
        let active = self.ui.split.active_pane.min(needed - 1);
        enter_pane(self, active);
        let rects = crate::split::groups::rects(self);
        for (pane, group) in self.ui.split.panes.iter_mut().zip(rects) {
            let [x, y, width, height] = group.text;
            (pane.viewport.x, pane.viewport.y) = (x, y);
            (pane.viewport.width, pane.viewport.height) = (width, height);
        }
        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }

    /// Moves the focused tab to group `group_idx` and focuses it
    /// (Control+Shift+1..4).
    pub(crate) fn move_to_group(&mut self, group_idx: usize) -> bool {
        if group_idx >= self.ui.split.num_panes() || !split_on(self) {
            return false;
        }
        let from = self.ui.split.active_pane;
        let Some(tab) = self.documents.active_tab else {
            return false;
        };
        move_tab(self, tab, from, group_idx, None);
        true
    }
}

/// Moves `tab` from group `from` to group `to` at `slot` (the end when
/// `None`) and focuses `to`. Within one group it only reorders. A group
/// left with no tab shows a second view of another open tab rather than
/// nothing, since every group has a document.
pub(crate) fn move_tab(app: &mut App, tab: TabId, from: usize, to: usize, slot: Option<usize>) {
    let count = app.ui.split.panes.len();
    if from >= count || to >= count {
        return;
    }
    if from == to {
        let tabs = &mut app.ui.split.panes[to].tabs;
        if let Some(at) = tabs.iter().position(|id| *id == tab) {
            tabs.remove(at);
            let slot = slot.unwrap_or(tabs.len());
            let slot = if slot > at { slot - 1 } else { slot };
            tabs.insert(slot.min(tabs.len()), tab);
        }
        app.scheduler.invalidate_all();
        return;
    }
    store_active_pane(app);
    app.sync_active_document_to_entry();
    let source = &mut app.ui.split.panes[from];
    let at = source.tabs.iter().position(|id| *id == tab).unwrap_or(0);
    source.tabs.retain(|id| *id != tab);
    let was_shown = source.tab_id == tab;
    let target = &mut app.ui.split.panes[to].tabs;
    target.retain(|id| *id != tab);
    let slot = slot.unwrap_or(target.len()).min(target.len());
    target.insert(slot, tab);
    if was_shown {
        let next = neighbor(&app.ui.split.panes[from].tabs, at).or_else(|| {
            let shown: Vec<TabId> = app.ui.split.panes.iter().map(|p| p.tab_id).collect();
            let open = open_tabs(app);
            let spare = open.iter().copied().filter(|id| *id != tab);
            let mut spare: Vec<TabId> = spare.collect();
            spare.sort_by_key(|id| shown.contains(id));
            spare.first().copied()
        });
        match next {
            Some(next) => show_in(app, from, next),
            // The only open tab: it stays shown where it was too.
            None => show_in(app, from, tab),
        }
    }
    show_in(app, to, tab);
    app.documents.active_tab = None;
    enter_pane(app, to);
}
