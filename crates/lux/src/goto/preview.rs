//! The transient preview tab of Goto Anything (ST-NAV-01).
//!
//! Arrowing onto a file that is not open reads it on a `ui_io` worker and
//! shows it in the window's single transient tab when the text arrives.
//! The next preview replaces that tab's document; Escape discards it. It
//! becomes an ordinary tab on Enter, on an edit, on a save, or on a
//! double-click of the tab. A read the panel stopped waiting for (the
//! selection moved on, or the panel closed) is dropped when it lands.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use lux_core::buffer::Buffer;
use lux_core::huge_file::{HugeFilePolicy, classify_path};
use lux_core::multicursor::MultiCursor;
use lux_ui::tabs::TabId;

use crate::app::App;
use crate::document::{DocumentEntry, Documents};

/// Shown in the panel input when the selected file is at or above the
/// huge-file threshold: previews never load a huge file. Enter opens it.
pub(crate) const TOO_LARGE_NOTICE: &str = "Too large to preview: Enter opens it";
/// Shown when the selected file could not be read (binary, missing, …).
pub(crate) const UNREADABLE_NOTICE: &str = "Cannot preview this file";

/// Two presses on the same tab within this window promote it.
const DOUBLE_CLICK_MS: u128 = 400;

/// The window's one transient tab, and the history state it was loaded
/// at: any other state means it was edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Transient {
    pub(crate) id: TabId,
    baseline: u64,
}

/// Transient-tab state kept beside the tab strip.
#[derive(Debug, Default)]
pub(crate) struct PreviewTab {
    transient: Option<Transient>,
    /// Last left press on a tab, for double-click promotion.
    last_press: Option<(std::time::Instant, TabId)>,
}

/// A caret a preview moved in an already-open tab, restored on Escape.
#[derive(Debug, Clone)]
pub(crate) struct TouchedTab {
    pub(crate) id: TabId,
    pub(crate) cursors: MultiCursor,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
}

/// Identifies one preview read; process-wide so a result from a closed
/// panel can never match the ticket of a panel opened later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ticket(u64);

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
enum Refusal {
    TooLarge,
    Unreadable,
}

/// A finished preview read, applied on the UI thread by `ui_io::poll`.
pub(crate) struct Loaded {
    ticket: Ticket,
    path: PathBuf,
    line: Option<u32>,
    col: Option<u32>,
    result: Result<Buffer, Refusal>,
}

/// The transient tab, if it is still open.
pub(crate) fn transient(documents: &Documents) -> Option<TabId> {
    let id = documents.strip.preview.transient?.id;
    documents
        .entries
        .iter()
        .any(|e| e.tab_id == id)
        .then_some(id)
}

pub(crate) fn is_transient(documents: &Documents, id: TabId) -> bool {
    transient(documents) == Some(id)
}

/// Starts reading `path` on a worker; the panel waits for this ticket only.
pub(crate) fn request(app: &mut App, path: PathBuf, line: Option<u32>, col: Option<u32>) {
    let ticket = Ticket(NEXT_TICKET.fetch_add(1, Ordering::Relaxed));
    let Some(goto) = app.ui.goto_anything.as_mut() else {
        return;
    };
    goto.preview = Some(ticket);
    crate::ui_io::spawn(app, None, move || {
        Some(crate::ui_io::Done::Preview(Box::new(read(
            ticket, path, line, col,
        ))))
    });
}

/// The panel no longer waits for any preview read.
pub(crate) fn cancel(app: &mut App) {
    if let Some(goto) = app.ui.goto_anything.as_mut() {
        goto.preview = None;
        if matches!(goto.notice, Some(TOO_LARGE_NOTICE | UNREADABLE_NOTICE)) {
            goto.notice = None;
        }
    }
}

fn read(ticket: Ticket, path: PathBuf, line: Option<u32>, col: Option<u32>) -> Loaded {
    let policy = HugeFilePolicy::default();
    let result = match classify_path(&path, policy) {
        Ok(class) if class.is_huge() => Err(Refusal::TooLarge),
        Ok(_) => Buffer::open_with_policy(&path, policy).map_err(|_| Refusal::Unreadable),
        Err(_) => Err(Refusal::Unreadable),
    };
    Loaded {
        ticket,
        path,
        line,
        col,
        result,
    }
}

/// Shows a finished read, unless the panel moved on or closed.
pub(crate) fn apply(app: &mut App, loaded: Loaded) {
    let Some(goto) = app.ui.goto_anything.as_mut() else {
        return;
    };
    if goto.preview != Some(loaded.ticket) {
        return;
    }
    goto.preview = None;
    match loaded.result {
        Ok(buffer) => {
            show(app, loaded.path, buffer);
            if let Some(line) = loaded.line {
                super::jump_to_line_and_col(app, line, loaded.col);
            }
        }
        Err(Refusal::TooLarge) => goto.notice = Some(TOO_LARGE_NOTICE),
        Err(Refusal::Unreadable) => goto.notice = Some(UNREADABLE_NOTICE),
    }
    app.scheduler.invalidate_all();
}

/// Puts `buffer` in the transient tab, creating the tab if there is none.
fn show(app: &mut App, path: PathBuf, buffer: Buffer) {
    let title = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let id = match transient(&app.documents) {
        Some(id) => {
            if app.documents.active_tab == Some(id) {
                // The old preview is discarded, not synced back.
                app.documents.active_tab = None;
            }
            if let Some(entry) = app.documents.entries.iter_mut().find(|e| e.tab_id == id) {
                *entry = DocumentEntry::new(id, Some(path), buffer);
            }
            app.documents.tabs.set_title(id, title);
            id
        }
        None => {
            app.sync_active_document_to_entry();
            let id = app.documents.tabs.open(title);
            app.documents
                .entries
                .push(DocumentEntry::new(id, Some(path), buffer));
            id
        }
    };
    app.switch_to_tab(id);
    app.documents.strip.preview.transient = Some(Transient {
        id,
        baseline: app.history.state_id(),
    });
}

/// The transient tab becomes an ordinary tab.
pub(crate) fn promote(app: &mut App) {
    if app.documents.strip.preview.transient.take().is_some() {
        app.scheduler.invalidate_all();
    }
}

/// Promotes the transient tab if it is the active one.
pub(crate) fn promote_active(app: &mut App) {
    if let Some(id) = transient(&app.documents)
        && app.documents.active_tab == Some(id)
    {
        promote(app);
    }
}

/// An edit makes the transient tab permanent. Cheap: one comparison.
pub(crate) fn settle(app: &mut App) {
    let Some(t) = app.documents.strip.preview.transient else {
        return;
    };
    let state = if app.documents.active_tab == Some(t.id) {
        Some(app.history.state_id())
    } else {
        app.documents
            .entries
            .iter()
            .find(|e| e.tab_id == t.id)
            .map(|e| e.history.state_id())
    };
    if state.is_some_and(|s| s != t.baseline) {
        promote(app);
    }
}

/// A left press on tab `id`: the second press in a row promotes it.
pub(crate) fn note_tab_press(app: &mut App, id: TabId) {
    let now = std::time::Instant::now();
    let double = app
        .documents
        .strip
        .preview
        .last_press
        .is_some_and(|(at, last)| {
            last == id && now.duration_since(at).as_millis() < DOUBLE_CLICK_MS
        });
    app.documents.strip.preview.last_press = (!double).then_some((now, id));
    if double && is_transient(&app.documents, id) {
        promote(app);
    }
}

/// Closes the transient tab without a trace: no reopen entry, no session
/// entry. If it was active, its neighbor becomes active.
pub(crate) fn discard(app: &mut App) {
    let Some(id) = transient(&app.documents) else {
        app.documents.strip.preview.transient = None;
        return;
    };
    app.documents.strip.preview.transient = None;
    let was_active = app.documents.active_tab == Some(id);
    app.documents.tabs.discard(id);
    app.documents.entries.retain(|e| e.tab_id != id);
    if was_active {
        app.documents.active_tab = None;
        if let Some(next) = app.documents.tabs.active().map(|t| t.id) {
            app.switch_to_tab(next);
        }
    }
    app.scheduler.invalidate_all();
}

/// Remembers the caret of open tab `id` before a preview moves it.
pub(crate) fn note_touched(app: &mut App, id: TabId) {
    let Some(goto) = app.ui.goto_anything.as_mut() else {
        return;
    };
    let Some(snapshot) = goto.snapshot.as_mut() else {
        return;
    };
    if snapshot.active_tab == Some(id) || snapshot.touched.iter().any(|t| t.id == id) {
        return;
    }
    let touched = if app.documents.active_tab == Some(id) {
        TouchedTab {
            id,
            cursors: app.cursors.clone(),
            scroll_x: app.viewport.scroll_x,
            scroll_y: app.viewport.scroll_y,
        }
    } else {
        let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == id) else {
            return;
        };
        TouchedTab {
            id,
            cursors: entry.cursors.clone(),
            scroll_x: entry.scroll_x,
            scroll_y: entry.scroll_y,
        }
    };
    snapshot.touched.push(touched);
}

/// Escape, a click outside, or Cmd+P again: back to the tab, carets and
/// scroll from before the panel opened, with the transient tab gone.
pub(crate) fn dismiss(app: &mut App, mut goto: super::GotoState) {
    let Some(snapshot) = goto.snapshot.take() else {
        return;
    };
    if let Some(origin) = snapshot.active_tab
        && app.documents.active_tab != Some(origin)
    {
        app.switch_to_tab(origin);
    }
    discard(app);
    for touched in snapshot.touched {
        if let Some(entry) = app
            .documents
            .entries
            .iter_mut()
            .find(|e| e.tab_id == touched.id)
        {
            entry.cursors = touched.cursors;
            entry.scroll_x = touched.scroll_x;
            entry.scroll_y = touched.scroll_y;
        }
    }
    app.cursors = snapshot.cursors;
    app.viewport.scroll_x = snapshot.scroll_x;
    app.viewport.scroll_y = snapshot.scroll_y;
    app.reset_goal_columns();
}
