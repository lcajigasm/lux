//! Cursor placement, stdin scratch files, and `--wait` lifetimes.
//!
//! The client counts one arrival per opened path. A failed open and a
//! closed tab each retire one. The client hears `closed` when none remain.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use lux_ui::tabs::TabId;

use super::Location;
use crate::app::App;
use crate::document::Request;

struct Gate {
    queue: VecDeque<(PathBuf, Location)>,
    pending: HashMap<u64, usize>,
    tabs: HashMap<TabId, u64>,
}

fn gate() -> &'static Mutex<Gate> {
    static GATE: OnceLock<Mutex<Gate>> = OnceLock::new();
    GATE.get_or_init(|| {
        Mutex::new(Gate {
            queue: VecDeque::new(),
            pending: HashMap::new(),
            tabs: HashMap::new(),
        })
    })
}

pub(crate) fn register(location: &Location) {
    let interesting = location.line.is_some()
        || location.column.is_some()
        || location.scratch
        || location.wait_token != 0;
    if !interesting {
        return;
    }
    let Ok(mut gate) = gate().lock() else {
        return;
    };
    if location.wait_token != 0 {
        *gate.pending.entry(location.wait_token).or_insert(0) += 1;
    }
    gate.queue
        .push_back((location.path.clone(), location.clone()));
}

fn take(path: &Path) -> Option<Location> {
    let Ok(mut gate) = gate().lock() else {
        return None;
    };
    let index = gate.queue.iter().position(|(queued, _)| queued == path)?;
    gate.queue.remove(index).map(|(_, location)| location)
}

pub(crate) fn note_open_failed(request: &Request) {
    let Request::Open(Some(path)) = request else {
        return;
    };
    let Some(location) = take(path) else {
        return;
    };
    retire(location.wait_token);
}

pub(crate) fn apply_opened(app: &mut App) {
    let Some(path) = app.documents.path.clone() else {
        return;
    };
    let Some(location) = take(&path) else {
        return;
    };
    if location.scratch {
        app.documents.path = None;
        let id = app.documents.active_tab;
        if let Some(id) = id
            && let Some(entry) = app
                .documents
                .entries
                .iter_mut()
                .find(|entry| entry.tab_id == id)
        {
            entry.path = None;
            if !entry.buffer.text().is_empty() {
                entry.saved_version = u64::MAX;
            }
        }
        if let Some(id) = id {
            app.documents.tabs.set_title(id, "Untitled");
        }
        if !app.buffer.text().is_empty() {
            app.documents.saved_version = u64::MAX;
        }
        let _ = std::thread::Builder::new()
            .name("lux-scratch-cleanup".into())
            .spawn(move || {
                let _ = std::fs::remove_file(path);
            });
    }
    if location.line.is_some() || location.column.is_some() {
        crate::goto::jump_to_line_and_col(app, location.line.unwrap_or(1), location.column);
    }
    if location.wait_token != 0
        && let Some(id) = app.documents.active_tab
    {
        watch(id, location.wait_token);
    }
}

fn watch(tab: TabId, token: u64) {
    if let Ok(mut gate) = gate().lock() {
        gate.tabs.insert(tab, token);
    }
}

pub(crate) fn release_tab(tab: TabId) {
    let token = gate()
        .lock()
        .ok()
        .and_then(|mut gate| gate.tabs.remove(&tab));
    if let Some(token) = token {
        retire(token);
    }
}

/// `--wait` with nothing left to open answers at once.
pub(crate) fn settle(token: u64) {
    if token == 0 {
        return;
    }
    let pending = match gate().lock() {
        Ok(gate) => gate.pending.get(&token).copied().unwrap_or(0),
        Err(_) => 0,
    };
    if pending == 0 {
        super::wait::finish(token);
    }
}

pub(crate) fn release_all() {
    let tokens: Vec<u64> = gate()
        .lock()
        .map(|mut gate| {
            gate.tabs.clear();
            gate.pending.keys().copied().collect()
        })
        .unwrap_or_default();
    for token in tokens {
        if let Ok(mut gate) = gate().lock() {
            gate.pending.remove(&token);
        }
        super::wait::finish(token);
    }
}

fn retire(token: u64) {
    if token == 0 {
        return;
    }
    let done = gate().lock().ok().is_some_and(|mut gate| {
        let left = gate.pending.entry(token).or_insert(0);
        *left = left.saturating_sub(1);
        if *left == 0 {
            gate.pending.remove(&token);
            true
        } else {
            false
        }
    });
    if done {
        super::wait::finish(token);
    }
}
