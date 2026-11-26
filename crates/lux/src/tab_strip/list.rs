//! The `▼` list of open tabs, drawn on the shared overlay card. Type to
//! filter; arrows, Page Up/Down, Home/End, hover, or the wheel select;
//! Enter or a click switches.

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, ScrollDelta};
use lux_ui::tabs::TabId;

use crate::app::App;
use crate::document::Documents;
use crate::overlay_list::ListFrame;

/// This list's slot in the shared trackpad remainder.
const WHEEL_SLOT: u8 = 12;

pub(crate) struct TabListState {
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected_index: usize,
}

/// One open tab as the list shows it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabRow {
    pub(crate) id: TabId,
    pub(crate) title: String,
    pub(crate) dirty: bool,
    pub(crate) active: bool,
    /// Goto Anything's transient preview tab.
    pub(crate) transient: bool,
    /// Byte ranges of `title` the query matched; empty without a query.
    pub(crate) hits: crate::overlay_list::Hits,
}

/// Rows of the open list in strip order, filtered by its query. `None`
/// when the list is closed. `current_dirty` is the live state of the
/// active document, whose entry is only synced on switch.
pub(crate) fn rows(documents: &Documents, current_dirty: bool) -> Option<Vec<TabRow>> {
    let state = documents.strip.list.as_ref()?;
    let all: Vec<TabRow> = documents
        .tabs
        .tabs()
        .iter()
        .map(|tab| {
            let active = documents.active_tab == Some(tab.id);
            let dirty = if active {
                current_dirty
            } else {
                documents
                    .entries
                    .iter()
                    .find(|e| e.tab_id == tab.id)
                    .is_some_and(|e| e.is_dirty())
            };
            TabRow {
                id: tab.id,
                title: tab.title.clone(),
                dirty,
                active,
                transient: crate::goto::preview::is_transient(documents, tab.id),
                hits: Vec::new(),
            }
        })
        .collect();
    let query = state.query.trim();
    if query.is_empty() {
        return Some(all);
    }
    Some(
        lux_search::fuzzy::rank_by(query, &all, |row| row.title.as_str())
            .into_iter()
            .map(|ranked| TabRow {
                hits: lux_search::fuzzy::char_spans(&ranked.item.title, &ranked.indices),
                ..ranked.item.clone()
            })
            .collect(),
    )
}

/// What the renderer needs from an open list.
pub(crate) struct TabListView {
    pub(crate) query: String,
    pub(crate) selected: usize,
    pub(crate) rows: Vec<TabRow>,
}

pub(crate) fn view(documents: &Documents, current_dirty: bool) -> Option<TabListView> {
    let rows = rows(documents, current_dirty)?;
    let state = documents.strip.list.as_ref()?;
    Some(TabListView {
        query: state.query.to_string(),
        selected: state.selected_index.min(rows.len().saturating_sub(1)),
        rows,
    })
}

fn app_rows(app: &App) -> Option<Vec<TabRow>> {
    rows(&app.documents, app.document_dirty())
}

/// Opens the list with the active tab selected.
pub(crate) fn open(app: &mut App) {
    let selected_index = app
        .documents
        .tabs
        .tabs()
        .iter()
        .position(|t| Some(t.id) == app.documents.active_tab)
        .unwrap_or(0);
    app.documents.strip.list = Some(TabListState {
        query: crate::field::QueryField::new(),
        selected_index,
    });
    app.scheduler.invalidate_all();
}

fn close(app: &mut App) {
    app.documents.strip.list = None;
    crate::focus::focus_editor(app);
    app.scheduler.invalidate_all();
}

fn choose(app: &mut App, id: TabId) {
    close(app);
    app.switch_to_tab(id);
}

fn set_selected(app: &mut App, index: usize) {
    if let Some(list) = app.documents.strip.list.as_mut() {
        list.selected_index = index;
    }
    app.scheduler.invalidate_all();
}

pub(crate) fn handle_key(app: &mut App, event: &KeyEvent) -> bool {
    let Some(rows) = app_rows(app) else {
        return false;
    };
    let selected = selected_index(app, rows.len());
    if crate::overlay_list::motion_for(&event.key).is_some() {
        let visible = card(app, selected, rows.len()).visible;
        let mut next = selected;
        crate::overlay_list::nudge(&mut next, rows.len(), visible, &event.key);
        set_selected(app, next);
        return true;
    }
    match &event.key {
        Key::Named(NamedKey::Escape) => close(app),
        Key::Named(NamedKey::Enter) => match rows.get(selected) {
            Some(row) => choose(app, row.id),
            None => close(app),
        },
        Key::Named(NamedKey::Backspace) => {
            if let Some(list) = app.documents.strip.list.as_mut() {
                list.query.pop();
                list.selected_index = 0;
            }
            app.scheduler.invalidate_all();
        }
        Key::Character(s) if !crate::field::is_shortcut_chord(app) => {
            let text = event.text.as_deref().unwrap_or(s);
            if let Some(list) = app.documents.strip.list.as_mut()
                && !text.chars().any(char::is_control)
            {
                list.query.push_str(text);
                list.selected_index = 0;
            }
            app.scheduler.invalidate_all();
        }
        _ => return false,
    }
    true
}

/// The shared overlay card for `total` rows with `selected` kept on screen.
/// Painting and hit-testing both size the card here.
pub(crate) fn card(app: &App, selected: usize, total: usize) -> ListFrame {
    let metrics = crate::modal::ModalMetrics::from_app(app);
    crate::overlay_list::centered_frame(
        metrics.vp_width,
        metrics.vp_height,
        metrics.tab_bar_height,
        metrics.status_bar,
        metrics.scale,
        total,
        selected,
    )
}

/// The badge a row carries on the card: `›` for the active tab and `●` for
/// unsaved changes. The title stays the plain tab title so match
/// highlighting lines up with the query.
/// The detail line of a row: the transient tab says it is a preview.
pub(crate) fn detail(row: &TabRow) -> Option<&'static str> {
    row.transient.then_some(PREVIEW_DETAIL)
}

pub(crate) const PREVIEW_DETAIL: &str = "Preview";

pub(crate) fn badge(row: &TabRow) -> Option<&'static str> {
    match (row.active, row.dirty) {
        (true, true) => Some("› ●"),
        (true, false) => Some("›"),
        (false, true) => Some("●"),
        (false, false) => None,
    }
}

/// Whether `(x, y)` is on the card at all.
pub(crate) fn card_contains(frame: &ListFrame, x: f32, y: f32) -> bool {
    x >= frame.left
        && x <= frame.left + frame.width
        && y >= frame.top
        && y <= frame.top + frame.height
}

/// The list index of the painted row under `(x, y)`, if any.
pub(crate) fn row_at(frame: &ListFrame, total: usize, x: f32, y: f32) -> Option<usize> {
    let rows_bottom = frame.rows_top + frame.visible as f32 * frame.row_height;
    if x < frame.left
        || x > frame.left + frame.width
        || y < frame.rows_top
        || y >= rows_bottom
        || frame.row_height <= 0.0
    {
        return None;
    }
    let index = frame.origin + ((y - frame.rows_top) / frame.row_height) as usize;
    (index < total).then_some(index)
}

fn selected_index(app: &App, total: usize) -> usize {
    app.documents
        .strip
        .list
        .as_ref()
        .map_or(0, |l| l.selected_index)
        .min(total.saturating_sub(1))
}

fn hit(app: &App) -> Option<(Vec<TabRow>, ListFrame)> {
    let rows = app_rows(app)?;
    let frame = card(app, selected_index(app, rows.len()), rows.len());
    Some((rows, frame))
}

/// A click on a row switches to that tab. A click outside the card closes
/// the list. Either way the click does not reach what is underneath.
pub(crate) fn click(app: &mut App, x: f32, y: f32) -> bool {
    let Some((rows, frame)) = hit(app) else {
        return false;
    };
    if !card_contains(&frame, x, y) {
        close(app);
        return true;
    }
    if let Some(row) = row_at(&frame, rows.len(), x, y).and_then(|i| rows.get(i)) {
        choose(app, row.id);
    }
    true
}

pub(crate) fn hover(app: &mut App, x: f32, y: f32) -> bool {
    let Some((rows, frame)) = hit(app) else {
        return false;
    };
    let Some(index) = row_at(&frame, rows.len(), x, y) else {
        return false;
    };
    if selected_index(app, rows.len()) == index {
        return false;
    }
    set_selected(app, index);
    true
}

/// Wheel moves the selection, like the other overlay lists.
pub(crate) fn scroll(app: &mut App, delta: ScrollDelta) -> bool {
    let Some(total) = app_rows(app).map(|r| r.len()) else {
        return false;
    };
    let steps = crate::overlay_list::wheel_steps(WHEEL_SLOT, delta, app.scale_factor);
    let Some(list) = app.documents.strip.list.as_mut() else {
        return false;
    };
    if total > 0 && steps != 0 {
        list.selected_index = list
            .selected_index
            .saturating_add_signed(steps)
            .min(total - 1);
        app.scheduler.invalidate_all();
    }
    true
}
