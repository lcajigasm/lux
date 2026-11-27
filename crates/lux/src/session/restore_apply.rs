//! Applying a session restore on the UI thread, as the workers report
//! back (see `restore`). Memory only.
//!
//! Tabs are added in session order as each one lands. The session's
//! active tab is shown as soon as it lands, unless the user has switched
//! tabs or edited since the restore began. The split layout is restored
//! once every tab has landed, since its panes name tabs by index.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lux_ui::tabs::TabId;
use lux_workspace::session::TabState;
use lux_workspace::session_store::LayoutState;

use super::restore::{Landed, Plan, RestoredTab};
use crate::app::App;
use crate::document::DocumentEntry;
use crate::split::LayoutPreset;

/// A restore in progress.
pub(crate) struct Restoring {
    generation: u64,
    /// Session tab index -> its tab, `None` until it lands or when its
    /// file is gone.
    tab_ids: Vec<Option<TabId>>,
    /// The saved state of each tab that has not landed yet.
    pending: Vec<Option<TabState>>,
    /// Where the history files named by `pending` live.
    history_dir: PathBuf,
    landed: usize,
    active: Option<usize>,
    layout: Option<LayoutState>,
    /// The launch's empty untitled tab, replaced by the restored ones.
    initial: Option<TabId>,
    /// What the user had open when the restore began. Restoring never
    /// takes the editor away from a tab the user moved to or typed in.
    active_at_start: Option<TabId>,
    state_at_start: u64,
    /// The session's tab was shown, or the user's own work kept instead.
    shown: bool,
    declined: bool,
}

/// The saved state of every tab of the restore in flight that has not
/// landed yet, in session order.
pub(crate) fn pending_tabs(app: &App) -> impl Iterator<Item = &TabState> {
    app.ui
        .session
        .restoring
        .iter()
        .flat_map(|restoring| restoring.pending.iter().flatten())
}

/// The history directory of the restore in flight, which the unlanded
/// tabs' history names refer to.
pub(crate) fn pending_history_dir(app: &App) -> Option<&Path> {
    let restoring = app.ui.session.restoring.as_ref()?;
    restoring
        .pending
        .iter()
        .any(Option::is_some)
        .then_some(restoring.history_dir.as_path())
}

/// The session's own split layout and active tab, for a session write
/// while its tabs are still landing, with tab indices remapped to the
/// snapshot's order: `index_of_tab` for landed tabs, then the unlanded
/// tabs from `landed` on. `None` once the restore is over or the user
/// declined it by moving to or typing in another tab. A layout or active
/// tab naming a tab whose file is gone comes back `None`.
pub(crate) fn saved_view(
    app: &App,
    index_of_tab: &HashMap<TabId, usize>,
    landed: usize,
) -> Option<(Option<LayoutState>, Option<usize>)> {
    let restoring = app.ui.session.restoring.as_ref()?;
    if restoring.declined {
        return None;
    }
    let map = |index: usize| -> Option<usize> {
        if let Some(tab_id) = restoring.tab_ids.get(index).copied().flatten() {
            return index_of_tab.get(&tab_id).copied();
        }
        restoring.pending.get(index)?.as_ref()?;
        let before = restoring.pending[..index].iter().flatten().count();
        Some(landed + before)
    };
    let layout = restoring.layout.clone().and_then(|mut layout| {
        for pane in &mut layout.panes {
            pane.tab = map(pane.tab)?;
            pane.tabs = pane.tabs.iter().filter_map(|index| map(*index)).collect();
        }
        Some(layout)
    });
    Some((layout, restoring.active.and_then(map)))
}

/// A new restore generation; results of older restores are dropped.
pub(crate) fn next_generation(app: &mut App) -> u64 {
    app.ui.session.restore_generation += 1;
    app.ui.session.restoring = None;
    app.ui.session.restore_generation
}

/// Applies a restore worker's result.
pub(crate) fn apply(app: &mut App, ctx: &dyn lux_platform::PlatformContext, landed: Landed) {
    match landed {
        Landed::Plan { generation, plan } => {
            let Some(plan) = plan else {
                return;
            };
            if let Some((generation, tabs, history_dir)) = begin(app, generation, *plan) {
                read_tabs(app, ctx, generation, tabs, history_dir);
            }
        }
        Landed::Tab {
            generation,
            index,
            tab,
        } => {
            apply_tab(app, generation, index, tab);
        }
    }
}

/// Reads the tabs on one worker thread, in session order.
fn read_tabs(
    app: &mut App,
    ctx: &dyn lux_platform::PlatformContext,
    generation: u64,
    tabs: Vec<TabState>,
    history_dir: PathBuf,
) {
    let jobs = tabs
        .into_iter()
        .enumerate()
        .map(|(index, state)| {
            let history_dir = history_dir.clone();
            Box::new(move || {
                let tab = super::restore::read_tab(state, &history_dir).map(Box::new);
                Some(crate::ui_io::Done::Session(Landed::Tab {
                    generation,
                    index,
                    tab,
                }))
            }) as crate::ui_io::Job
        })
        .collect();
    crate::ui_io::spawn_each(app, Some(ctx), jobs);
}

/// Opens the session's folder and records the restore. Returns the
/// restore's generation (opening the folder starts a new one), and the
/// tabs to read, or `None` when there is nothing to restore.
pub(crate) fn begin(
    app: &mut App,
    generation: u64,
    plan: Plan,
) -> Option<(u64, Vec<TabState>, PathBuf)> {
    if generation != app.ui.session.restore_generation {
        return None;
    }
    let mut generation = generation;
    if let Some(folder) = plan.root
        && app.workspace_root().as_ref() != Some(&folder)
    {
        // The restore moves the window to its own folder; anything tied
        // to the previous root ends, and this restore carries on under
        // the generation that change started.
        super::root_changing(app);
        generation = app.ui.session.restore_generation;
        crate::sidebar::reads::open_root(app, folder.clone());
        app.ui.session.init_watcher(&folder);
        app.update_viewport_dimensions();
    }
    app.scheduler.invalidate_all();
    let session = plan.session;
    if session.tabs.is_empty() {
        return None;
    }
    let initial = (app.documents.entries.len() == 1
        && app.documents.entries[0].path.is_none()
        && !app.documents.entries[0].is_dirty()
        && app.documents.entries[0].buffer.len_chars() == 0)
        .then(|| app.documents.entries[0].tab_id);
    app.ui.session.restoring = Some(Restoring {
        generation,
        tab_ids: vec![None; session.tabs.len()],
        pending: session.tabs.iter().cloned().map(Some).collect(),
        history_dir: plan.history_dir.clone(),
        landed: 0,
        active: session.active_tab,
        layout: session.layout,
        initial,
        active_at_start: app.documents.active_tab,
        state_at_start: app.history.state_id(),
        shown: false,
        declined: false,
    });
    Some((generation, session.tabs, plan.history_dir))
}

/// Adds one landed tab. Returns `Some(any tab restored)` once the last
/// tab of the restore has landed.
pub(crate) fn apply_tab(
    app: &mut App,
    generation: u64,
    index: usize,
    tab: Option<Box<RestoredTab>>,
) -> Option<bool> {
    let mut restoring = app.ui.session.restoring.take()?;
    if restoring.generation != generation || index >= restoring.tab_ids.len() {
        app.ui.session.restoring = Some(restoring);
        return None;
    }
    restoring.landed += 1;
    restoring.pending[index] = None;
    if let Some(tab) = tab {
        // A file the user already has open keeps the user's tab and text;
        // the session's copy is not added beside it. The session's carets
        // and split describe the other text, so its view is not applied.
        let tab_id = match tab.state.path.as_deref().and_then(|p| open_tab(app, p)) {
            Some(open) => {
                restoring.declined = true;
                open
            }
            None => {
                let added = add_tab(app, *tab);
                place_in_session_order(app, &restoring.tab_ids, index, added);
                added
            }
        };
        restoring.tab_ids[index] = Some(tab_id);
        if restoring.active == Some(index) {
            show(app, &mut restoring, tab_id);
        }
    }
    let finished = restoring.landed == restoring.tab_ids.len();
    if !finished {
        app.ui.session.restoring = Some(restoring);
        app.scheduler.invalidate_all();
        return None;
    }
    let first = restoring.tab_ids.iter().flatten().next().copied();
    let any = first.is_some();
    if !restoring.shown
        && !restoring.declined
        && let Some(first) = restoring
            .active
            .and_then(|idx| restoring.tab_ids.get(idx).copied().flatten())
            .or(first)
    {
        show(app, &mut restoring, first);
    }
    if restoring.shown
        && !restoring.declined
        && let Some(layout) = &restoring.layout
    {
        let line_height = app.editor_view.metrics().line_height.max(1.0);
        restore_layout(app, layout, &restoring.tab_ids, line_height);
    }
    app.scheduler.invalidate_all();
    Some(any)
}

/// Moves the tab that just landed for session index `index` next to the
/// landed tabs around it, so the strip keeps the session's order however
/// the reads finish: right after the nearest earlier landed tab, else
/// right before the nearest later one.
fn place_in_session_order(app: &mut App, tab_ids: &[Option<TabId>], index: usize, tab: TabId) {
    let tabs = &mut app.documents.tabs;
    let Some(from) = tabs.position(tab) else {
        return;
    };
    let before = tab_ids[..index]
        .iter()
        .rev()
        .flatten()
        .find_map(|id| tabs.position(*id));
    let after = || {
        tab_ids[index + 1..]
            .iter()
            .flatten()
            .find_map(|id| tabs.position(*id))
    };
    // `move_tab` removes the tab first; a slot past it shifts down by one.
    let to = match before {
        Some(at) if at < from => at + 1,
        Some(at) => at,
        None => match after() {
            Some(at) if at < from => at,
            Some(at) => at.saturating_sub(1),
            None => return,
        },
    };
    if to != from {
        tabs.move_tab(tab, to);
    }
}

/// The tab already showing `path`, if any.
fn open_tab(app: &App, path: &Path) -> Option<TabId> {
    if app.documents.path.as_deref() == Some(path) {
        return app.documents.active_tab;
    }
    app.documents
        .entries
        .iter()
        .find(|e| e.path.as_deref() == Some(path))
        .map(|e| e.tab_id)
}

fn add_tab(app: &mut App, tab: RestoredTab) -> TabId {
    let state = tab.state;
    let title = if !state.title.is_empty() {
        state.title.clone()
    } else if let Some(path) = &state.path {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string()
    } else {
        "Untitled".to_string()
    };
    let tab_id = app.documents.tabs.open(&title);
    let mut entry = DocumentEntry::new(tab_id, state.path.clone(), tab.buffer);
    if let Some(language) = tab.language {
        entry.language = language;
    }
    if let Some(history) = tab.history {
        entry.history = history;
    }
    // A restored dirty tab stays dirty even if undo walks back to the text
    // that was on disk: nothing proves the file still holds that text.
    entry.saved_version =
        if state.is_dirty || (state.path.is_none() && entry.buffer.len_chars() > 0) {
            u64::MAX
        } else {
            entry.history.state_id()
        };
    if let Some(cursors) = tab.cursors {
        entry.cursors = cursors;
    }
    let line_height = app.editor_view.metrics().line_height.max(1.0);
    entry.scroll_y = state.scroll_line as f32 * line_height;
    if state.pinned {
        app.documents.tabs.pin(tab_id);
    }
    app.documents.entries.push(entry);
    tab_id
}

/// Shows a restored tab, replacing the launch's empty untitled tab, unless
/// the user has moved to another tab or typed since the restore began.
/// The split layout follows only a shown tab.
fn show(app: &mut App, restoring: &mut Restoring, tab_id: TabId) {
    if restoring.declined
        || app.documents.active_tab != restoring.active_at_start
        || app.history.state_id() != restoring.state_at_start
    {
        restoring.declined = true;
        return;
    }
    restoring.shown = true;
    if let Some(initial) = restoring.initial.take()
        && let Some(position) = app
            .documents
            .entries
            .iter()
            .position(|e| e.tab_id == initial && e.path.is_none() && e.buffer.len_chars() == 0)
    {
        app.documents.entries.remove(position);
        app.documents.tabs.close(initial);
    }
    app.switch_to_tab(tab_id);
    // Switching reveals the caret; the restored scroll wins, even when the
    // caret was scrolled out of view.
    if let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == tab_id) {
        app.viewport.scroll_y = entry.scroll_y;
    }
    restoring.active_at_start = app.documents.active_tab;
    restoring.state_at_start = app.history.state_id();
}

fn parse_preset(name: &str) -> Option<LayoutPreset> {
    Some(match name {
        "Columns2" => LayoutPreset::Columns2,
        "Columns3" => LayoutPreset::Columns3,
        "Columns4" => LayoutPreset::Columns4,
        "Rows2" => LayoutPreset::Rows2,
        "Rows3" => LayoutPreset::Rows3,
        "Grid2x2" => LayoutPreset::Grid2x2,
        _ => return None,
    })
}

fn restore_layout(
    app: &mut App,
    layout: &LayoutState,
    tab_ids: &[Option<TabId>],
    line_height: f32,
) {
    let Some(preset) = parse_preset(&layout.preset) else {
        return;
    };
    let ratio = |millis: u32| (millis as f32 / 1000.0).clamp(0.1, 0.9);
    app.ui.split.ratio = ratio(layout.ratio_millis);
    app.ui.split.ratio_y = ratio(layout.ratio_y_millis);
    app.set_layout(preset);
    let count = app.ui.split.panes.len();
    for (idx, saved) in layout.panes.iter().enumerate().take(count) {
        let Some(tab_id) = tab_ids.get(saved.tab).copied().flatten() else {
            continue;
        };
        let Some(entry) = app.documents.entries.iter().find(|e| e.tab_id == tab_id) else {
            continue;
        };
        let cursors = super::restore::multi_cursor(&entry.buffer, &saved.cursors);
        let tokens = super::pane_tokens(entry);
        let listed: Vec<TabId> = saved
            .tabs
            .iter()
            .filter_map(|index| tab_ids.get(*index).copied().flatten())
            .collect();
        let pane = &mut app.ui.split.panes[idx];
        pane.tab_id = tab_id;
        pane.active_tab = tab_id;
        pane.tabs = listed;
        if let Some(cursors) = cursors {
            pane.cursors = cursors;
        }
        pane.viewport.scroll_y = saved.scroll_line as f32 * line_height;
        pane.tokens.replace(tokens);
    }
    // Every tab has landed by now, so each group's list holds open tabs
    // only. Older sessions list no group tabs: every tab joins the first
    // group, and each other group keeps the tab it shows.
    crate::split::tabs::restored(app);
    let active = layout.active_pane.min(count.saturating_sub(1));
    if let Some(pane) = app.ui.split.panes.get(active).cloned() {
        // Load the pane straight into the editor. `focus_group` would first
        // copy the editor's current state over the restored pane.
        app.ui.split.active_pane = active;
        app.switch_to_tab(pane.tab_id);
        app.ui.split.panes[active] = pane.clone();
        app.cursors = pane.cursors;
        app.reset_goal_columns();
        app.viewport.scroll_y = pane.viewport.scroll_y;
    }
    app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
}
