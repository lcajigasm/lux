//! Syntax, diagnostics, and paint inputs belong to one buffer.
//!
//! The editor used to keep a single highlighter, a single diagnostic list,
//! and a single gutter-mark list on the process. Closing Settings as JSON
//! left that list painted on the Plain Text buffer (red dashes on the
//! settings file's line numbers, including rows the Plain Text buffer does
//! not have). Tab changes park the outgoing buffer's view and install the
//! incoming one. Closing a buffer drops its parked view.

use std::collections::HashMap;
use std::path::Path;

use lux_lsp::async_lsp::lsp_types::Url;

use super::App;
use super::types::SyntaxState;
use crate::lsp::DiagnosticItem;

#[derive(Default)]
pub(crate) struct ViewCache {
    syntax: HashMap<u64, SyntaxState>,
    diagnostics: HashMap<u64, Vec<DiagnosticItem>>,
}

impl ViewCache {
    fn park_syntax(&mut self, syntax: SyntaxState) {
        self.syntax.insert(syntax.buffer_id, syntax);
    }

    fn take_syntax(&mut self, buffer_id: u64) -> Option<SyntaxState> {
        self.syntax.remove(&buffer_id)
    }

    pub(crate) fn drop_syntax(&mut self, buffer_id: u64) {
        self.syntax.remove(&buffer_id);
    }

    fn park_diagnostics(&mut self, buffer_id: u64, items: Vec<DiagnosticItem>) {
        if items.is_empty() {
            self.diagnostics.remove(&buffer_id);
        } else {
            self.diagnostics.insert(buffer_id, items);
        }
    }

    fn take_diagnostics(&mut self, buffer_id: u64) -> Vec<DiagnosticItem> {
        self.diagnostics.remove(&buffer_id).unwrap_or_default()
    }

    fn drop_diagnostics(&mut self, buffer_id: u64) {
        self.diagnostics.remove(&buffer_id);
    }

    fn drop_buffer(&mut self, buffer_id: u64) {
        self.drop_syntax(buffer_id);
        self.drop_diagnostics(buffer_id);
    }

    fn tokens(&self, buffer_id: u64) -> Option<Vec<lux_syntax::Token>> {
        self.syntax
            .get(&buffer_id)
            .map(|syntax| syntax.tokens.clone())
    }
}

fn buffer_open(app: &App, buffer_id: u64) -> bool {
    app.documents
        .entries
        .iter()
        .any(|entry| entry.buffer_id == buffer_id)
}

/// The painted diagnostic list is the active buffer's, and a later event
/// for another file cannot overwrite it.
pub(crate) fn remember_diagnostics(app: &mut App, buffer_id: u64, items: Vec<DiagnosticItem>) {
    if app.ui.lsp.diagnostics_for == Some(buffer_id) {
        set_buffer_diagnostics(app, items);
        return;
    }
    app.documents.view_cache.park_diagnostics(buffer_id, items);
}

pub(crate) fn set_buffer_diagnostics(app: &mut App, items: Vec<DiagnosticItem>) {
    let id = app.documents.active_buffer_id();
    if let Some(id) = id {
        app.documents.view_cache.drop_diagnostics(id);
    }
    app.ui.lsp.diagnostics = items;
    app.ui.lsp.diagnostics_for = id;
}

/// Drop everything parked for a buffer that is going away.
pub(crate) fn forget(app: &mut App, buffer_id: u64, path: Option<&Path>) {
    app.documents.view_cache.drop_buffer(buffer_id);
    if app
        .syntax
        .as_ref()
        .is_some_and(|syntax| syntax.buffer_id == buffer_id)
    {
        app.syntax = None;
    }
    if app.ui.lsp.diagnostics_for == Some(buffer_id) {
        app.ui.lsp.diagnostics.clear();
        app.ui.lsp.diagnostics_for = None;
    }
    if let Some(path) = path
        && let Ok(url) = Url::from_file_path(path)
    {
        app.ui
            .lsp
            .all_diagnostics
            .retain(|(candidate, _)| candidate != &url);
        if app.ui.lsp.current_url.as_ref() == Some(&url) {
            app.ui.lsp.current_url = None;
        }
    }
    for pane in &mut app.ui.split.panes {
        if app.documents.buffer_id_for_tab(pane.tab_id) == Some(buffer_id) {
            pane.tokens.clear();
        }
    }
    if let Some(secondary) = &mut app.ui.split.secondary
        && app.documents.buffer_id_for_tab(secondary.tab_id) == Some(buffer_id)
    {
        secondary.tokens.clear();
    }
}

/// Install the active buffer's syntax, diagnostics, and paint inputs.
pub(crate) fn bind(app: &mut App) {
    let active = app.documents.active_buffer_id();
    retarget_syntax(app, active);
    retarget_diagnostics(app, active);
    app.ui.lsp.completion_popup = None;
    app.ui.lsp.hover_popup = None;
    app.ui.git.markers.clear();
    app.ui.git.hunks.clear();
    app.ui.git.active_popup = None;
    sync_pane_tokens(app);
    scrub_closed_diagnostics(app);
    app.scheduler.invalidate_all();
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
}

fn retarget_syntax(app: &mut App, active: Option<u64>) {
    let owner = app.syntax.as_ref().map(|syntax| syntax.buffer_id);
    if owner == active && owner.is_some() {
        return;
    }
    if let Some(syntax) = app.syntax.take()
        && buffer_open(app, syntax.buffer_id)
    {
        app.documents.view_cache.park_syntax(syntax);
    }
    if let Some(id) = active
        && let Some(syntax) = app.documents.view_cache.take_syntax(id)
        && app.documents.language() == Some(syntax.language)
    {
        let ranges = syntax.fold_ranges.clone();
        app.syntax = Some(syntax);
        app.fold_state.set_foldable_ranges(ranges);
        return;
    }
    app.update_syntax_on_open();
}

fn retarget_diagnostics(app: &mut App, active: Option<u64>) {
    if app.ui.lsp.diagnostics_for != active {
        if let Some(owner) = app.ui.lsp.diagnostics_for.take() {
            let items = std::mem::take(&mut app.ui.lsp.diagnostics);
            if buffer_open(app, owner) {
                app.documents.view_cache.park_diagnostics(owner, items);
            }
        } else {
            app.ui.lsp.diagnostics.clear();
        }
        let restored = active
            .map(|id| app.documents.view_cache.take_diagnostics(id))
            .unwrap_or_default();
        app.ui.lsp.diagnostics = restored;
        app.ui.lsp.diagnostics_for = active;
    }
    let active_url = app
        .documents
        .path
        .as_ref()
        .and_then(|path| Url::from_file_path(path).ok());
    if app.ui.lsp.current_url != active_url {
        app.ui.lsp.current_url = active_url;
    }
}

fn scrub_closed_diagnostics(app: &mut App) {
    app.ui.lsp.all_diagnostics.retain(|(url, _)| {
        app.documents.entries.iter().any(|entry| {
            entry
                .path
                .as_ref()
                .and_then(|path| Url::from_file_path(path).ok())
                .as_ref()
                == Some(url)
        })
    });
}

fn sync_pane_tokens(app: &mut App) {
    let active = app.documents.active_buffer_id();
    let planned: Vec<Vec<lux_syntax::Token>> = app
        .ui
        .split
        .panes
        .iter()
        .map(|pane| tokens_for(app, app.documents.buffer_id_for_tab(pane.tab_id), active))
        .collect();
    for (pane, tokens) in app.ui.split.panes.iter_mut().zip(planned) {
        pane.tokens = tokens;
    }
    let secondary_tokens = app.ui.split.secondary.as_ref().map(|secondary| {
        tokens_for(
            app,
            app.documents.buffer_id_for_tab(secondary.tab_id),
            active,
        )
    });
    if let Some(tokens) = secondary_tokens
        && let Some(secondary) = &mut app.ui.split.secondary
    {
        secondary.tokens = tokens;
    }
}

fn tokens_for(app: &App, buffer_id: Option<u64>, active: Option<u64>) -> Vec<lux_syntax::Token> {
    if buffer_id == active {
        return app
            .syntax
            .as_ref()
            .map(|syntax| syntax.tokens.clone())
            .unwrap_or_default();
    }
    buffer_id
        .and_then(|id| app.documents.view_cache.tokens(id))
        .unwrap_or_default()
}

#[cfg(test)]
mod view_bind_tests;
