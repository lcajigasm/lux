//! Writing the session (ADR 0019). The UI thread takes a cheap snapshot —
//! rope clones, cursor positions, and each tab's undo history shared
//! through an `Arc` that is rebuilt only when that history changed — and
//! hands it to the shared latest-only worker. The worker turns drafts into
//! text, encodes the histories, and writes everything transactionally:
//! history files first, then the session JSON atomically, then it prunes
//! history files the new JSON no longer names.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lux_core::buffer::Buffer;
use lux_core::history::{History, codec};
use lux_core::multicursor::MultiCursor;
use lux_workspace::WindowFrame;
use lux_workspace::session::{CursorState, Session, TabState};
use lux_workspace::session_store::{self, LayoutState, PaneState};

use crate::app::App;
use crate::latest_worker::LatestWorker;
use crate::split::LayoutPreset;

/// What one tab contributes beyond its JSON: the text to store as a draft
/// and the history to encode against that text.
pub(crate) struct TabPayload {
    buffer: Buffer,
    draft: bool,
    history: Option<Arc<History>>,
}

/// Everything the worker needs to write one session.
pub(crate) struct Snapshot {
    file: PathBuf,
    session: Session,
    payloads: Vec<TabPayload>,
    frame: Option<WindowFrame>,
    clean: bool,
}

static SESSION_WRITER: LatestWorker<Snapshot> = LatestWorker::new("lux-session", write);

/// Queues a session write on the worker. Returns at once.
pub(crate) fn submit(app: &mut App) {
    let snapshot = take(app, false);
    SESSION_WRITER.submit(snapshot);
}

/// Writes the session on the calling thread and marks it a clean exit.
/// Only the quit path calls this: the process must not exit before the
/// newest state is on disk.
pub(crate) fn write_now(app: &mut App, clean: bool) -> std::io::Result<()> {
    let snapshot = take(app, clean);
    let file = snapshot.file.clone();
    SESSION_WRITER.run_now(snapshot);
    if file.exists() {
        Ok(())
    } else {
        Err(std::io::Error::other("session was not written"))
    }
}

pub(crate) fn current_file(app: &App) -> PathBuf {
    app.ui
        .session
        .session_path
        .clone()
        .unwrap_or_else(|| super::location::session_file(workspace_root(app)))
}

fn workspace_root(app: &App) -> Option<&Path> {
    app.ui
        .sidebar
        .as_ref()
        .and_then(|s| s.workspace_root.as_deref())
}

/// Builds the snapshot. Cost is a rope clone per tab plus one history
/// clone for each tab whose history changed since the last snapshot.
fn take(app: &mut App, clean: bool) -> Snapshot {
    let file = current_file(app);
    let mut session = Session::new();
    if let Some(root) = workspace_root(app) {
        session.folders.push(root.to_path_buf());
    }
    let line_height = app.editor_view.metrics().line_height.max(1.0);
    let order: Vec<_> = app
        .documents
        .tabs
        .tabs()
        .iter()
        .map(|t| (t.id, t.title.clone(), t.pinned))
        .collect();
    let mut payloads = Vec::new();
    let mut index_of_tab = HashMap::new();
    let mut seen_histories = HashMap::new();
    let only_one = app.documents.entries.len() <= 1;

    for (tab_id, title, pinned) in order {
        let active = app.documents.active_tab == Some(tab_id);
        let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == tab_id) else {
            continue;
        };
        let (path, buffer, cursors, history, scroll_y, dirty) = if active {
            (
                app.documents.path.clone().or_else(|| entry.path.clone()),
                &app.buffer,
                &app.cursors,
                &app.history,
                app.viewport.scroll_y,
                app.history.state_id() != app.documents.saved_version,
            )
        } else {
            (
                entry.path.clone(),
                &entry.buffer,
                &entry.cursors,
                &entry.history,
                entry.scroll_y,
                entry.is_dirty(),
            )
        };
        let empty_untitled = path.is_none() && buffer.len_chars() == 0 && !dirty;
        if empty_untitled && !only_one {
            continue;
        }
        let is_dirty = dirty || (path.is_none() && buffer.len_chars() > 0);
        let keep_history =
            (history.can_undo() || history.can_redo()) && !buffer.is_huge_file_mode();
        let shared = keep_history.then(|| {
            let shared = shared_history(&app.ui.session.histories, entry.buffer_id, history);
            seen_histories.insert(entry.buffer_id, Arc::clone(&shared));
            shared
        });
        index_of_tab.insert(tab_id, session.tabs.len());
        if active {
            session.active_tab = Some(session.tabs.len());
        }
        session.tabs.push(TabState {
            path: path.clone(),
            title,
            pinned,
            cursors: cursor_states(cursors),
            scroll_line: (scroll_y / line_height).floor() as usize,
            language: entry
                .language
                .map(|l| crate::status_bar::language_name(Some(l)).to_string()),
            draft_text: None,
            is_dirty,
            history: None,
        });
        payloads.push(TabPayload {
            buffer: buffer.clone(),
            draft: is_dirty || path.is_none(),
            history: shared,
        });
    }
    // Histories of closed tabs are no longer worth keeping in memory.
    app.ui.session.histories = seen_histories;
    session.layout = layout_state(app, &index_of_tab, line_height);

    Snapshot {
        file,
        session,
        payloads,
        frame: app.ui.session.frame.clone(),
        clean,
    }
}

/// The history behind an `Arc`, cloned only when it changed since the
/// last snapshot of the same buffer.
fn shared_history(
    cache: &HashMap<u64, Arc<History>>,
    buffer_id: u64,
    history: &History,
) -> Arc<History> {
    match cache.get(&buffer_id) {
        Some(cached) if cached.revision() == history.revision() => Arc::clone(cached),
        _ => Arc::new(history.clone()),
    }
}

fn cursor_states(cursors: &MultiCursor) -> Vec<CursorState> {
    cursors
        .cursors()
        .iter()
        .map(|s| CursorState {
            char_offset: s.head.char,
            anchor: (s.anchor.char != s.head.char).then_some(s.anchor.char),
        })
        .collect()
}

fn layout_state(
    app: &App,
    index_of_tab: &HashMap<lux_ui::tabs::TabId, usize>,
    line_height: f32,
) -> Option<LayoutState> {
    let split = &app.ui.split;
    if !split.is_active() || split.panes.is_empty() {
        return None;
    }
    let preset = if split.preset == LayoutPreset::Single {
        match split.direction {
            crate::split::SplitDirection::Horizontal => LayoutPreset::Columns2,
            crate::split::SplitDirection::Vertical => LayoutPreset::Rows2,
        }
    } else {
        split.preset
    };
    let mut panes = Vec::with_capacity(split.panes.len());
    for (idx, pane) in split.panes.iter().enumerate() {
        let tab = *index_of_tab.get(&pane.tab_id)?;
        let (cursors, scroll_y) = if idx == split.active_pane {
            (&app.cursors, app.viewport.scroll_y)
        } else {
            (&pane.cursors, pane.viewport.scroll_y)
        };
        panes.push(PaneState {
            tab,
            cursors: cursor_states(cursors),
            scroll_line: (scroll_y / line_height).floor() as usize,
        });
    }
    Some(LayoutState {
        preset: format!("{preset:?}"),
        ratio_millis: (split.ratio.clamp(0.0, 1.0) * 1000.0).round() as u32,
        ratio_y_millis: (split.ratio_y.clamp(0.0, 1.0) * 1000.0).round() as u32,
        active_pane: split.active_pane,
        panes,
    })
}

/// Encoded history file names by history revision and buffer hash, so an
/// unchanged history is not encoded again on every write.
static ENCODED: Mutex<Option<HashMap<(u64, u64), String>>> = Mutex::new(None);

/// Runs on the worker.
fn write(snapshot: Snapshot) {
    let Snapshot {
        file,
        mut session,
        payloads,
        frame,
        clean,
    } = snapshot;
    let dir = session_store::history_dir(&file);
    let mut keep = HashSet::new();
    let mut encoded = ENCODED.lock().unwrap_or_else(|e| e.into_inner());
    let previous = encoded.take().unwrap_or_default();
    let mut current = HashMap::new();
    for (tab, payload) in session.tabs.iter_mut().zip(payloads) {
        if payload.draft {
            tab.draft_text = Some(payload.buffer.text());
        }
        let Some(history) = payload.history else {
            continue;
        };
        let key = (history.revision(), codec::content_hash(&payload.buffer));
        let name = match previous.get(&key).or_else(|| current.get(&key)) {
            Some(name) if session_store::history_exists(&dir, name) => Some(String::clone(name)),
            _ => encode_history(&dir, &history, &payload.buffer),
        };
        if let Some(name) = name {
            keep.insert(name.clone());
            current.insert(key, name.clone());
            tab.history = Some(name);
        }
    }
    *encoded = Some(current);
    drop(encoded);

    if let Some(frame) = frame {
        crate::place::mirror_global(&frame, &file);
        session.windows.push(frame);
    } else if let Ok(previous) = Session::load(&file) {
        session.windows = previous.windows;
    }
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let written = if clean {
        session.end_cleanly(&file)
    } else {
        session.save(&file)
    };
    // Prune only once the JSON that stops naming them is in place.
    if written.is_ok() {
        let _ = session_store::prune_histories(&dir, &keep);
    }
}

/// Encodes one history and writes it under a name derived from its bytes,
/// so a name always identifies exactly one content.
fn encode_history(dir: &Path, history: &History, buffer: &Buffer) -> Option<String> {
    let bytes = codec::encode(history, buffer, codec::DEFAULT_BUDGET)?;
    let tail: [u8; 8] = bytes.get(bytes.len().checked_sub(8)?..)?.try_into().ok()?;
    let name = format!("{:016x}.luxh", u64::from_le_bytes(tail));
    session_store::write_history(dir, &name, &bytes).ok()?;
    Some(name)
}
