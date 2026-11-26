//! Soft Undo / Soft Redo (`Cmd+U` / `Cmd+Shift+U`): step back and forth
//! through earlier caret and selection states without changing the text.
//!
//! The selection history is settled at the start of every keyboard and
//! editor mouse press, so whatever the previous command did to the
//! selections (a Goto jump, Cmd+D, a find, a click, a cursor move) becomes
//! one step. Goto Anything and the command palette preview positions while
//! they are open; nothing settles until they close, so a jump made through
//! them is one step back to where the carets were before they opened.

use std::collections::HashMap;

use lux_core::edit::Edit;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_core::selection_history::{SelectionHistory, SelectionSnapshot};
use lux_ui::tabs::TabId;

use crate::app::App;

/// The active document's selection history, and the histories of the
/// other open tabs, parked until they are active again.
#[derive(Default)]
pub(crate) struct SoftUndo {
    active: SelectionHistory,
    tab: Option<TabId>,
    parked: HashMap<TabId, SelectionHistory>,
}

impl App {
    fn selection_snapshot(&self) -> SelectionSnapshot {
        SelectionSnapshot {
            selections: crate::input::with_primary_last(&self.cursors)
                .iter()
                .map(|s| (s.anchor.char, s.head.char))
                .collect(),
            scroll: (self.viewport.scroll_x, self.viewport.scroll_y),
        }
    }

    /// Makes the selection history the active tab's own.
    fn follow_active_tab(&mut self) {
        let tab = self.documents.active_tab;
        let soft = &mut self.carets.soft_undo;
        if soft.tab == tab {
            return;
        }
        if let Some(old) = soft.tab {
            soft.parked.insert(old, std::mem::take(&mut soft.active));
        }
        soft.active = tab
            .and_then(|id| soft.parked.remove(&id))
            .unwrap_or_default();
        soft.tab = tab;
        let open = &self.documents.entries;
        soft.parked
            .retain(|id, _| open.iter().any(|entry| entry.tab_id == *id));
    }

    /// Records the current selections; a change since the last call
    /// becomes a soft-undo step.
    pub(crate) fn settle_selection_history(&mut self) {
        if self.ui.goto_anything.is_some() || self.ui.command_palette.is_some() {
            return;
        }
        self.follow_active_tab();
        let current = self.selection_snapshot();
        let version = self.history.state_id();
        self.carets.soft_undo.active.settle(current, version);
    }

    /// Keeps remembered selections on the same text after `edits`.
    pub(crate) fn note_edit_for_soft_undo(&mut self, edits: &[Edit]) {
        self.follow_active_tab();
        let current = self.selection_snapshot();
        let version = self.history.state_id();
        self.carets
            .soft_undo
            .active
            .after_edit(edits, current, version);
    }

    pub(crate) fn soft_undo(&mut self) {
        self.settle_selection_history();
        let current = self.selection_snapshot();
        if let Some(previous) = self.carets.soft_undo.active.soft_undo(current) {
            self.restore_selection_snapshot(previous);
        }
    }

    pub(crate) fn soft_redo(&mut self) {
        self.settle_selection_history();
        let current = self.selection_snapshot();
        if let Some(next) = self.carets.soft_undo.active.soft_redo(current) {
            self.restore_selection_snapshot(next);
        }
    }

    fn restore_selection_snapshot(&mut self, snapshot: SelectionSnapshot) {
        let len = self.buffer.len_chars();
        let selections: Vec<Selection> = snapshot
            .selections
            .iter()
            .map(|&(anchor, head)| {
                Selection::new(
                    Position::from_char(&self.buffer, anchor.min(len)),
                    Position::from_char(&self.buffer, head.min(len)),
                )
            })
            .collect();
        if selections.is_empty() {
            return;
        }
        self.snippet_session = None;
        self.cursors = MultiCursor::from_cursors(selections);
        for selection in self.cursors.cursors().to_vec() {
            if self.fold_state.is_line_hidden(selection.head.line) {
                self.fold_state.unfold_cursor(selection.head.line);
            }
        }
        (self.viewport.scroll_x, self.viewport.scroll_y) = snapshot.scroll;
        // Keeps the primary caret on screen if the view has since resized.
        self.reset_goal_columns();
    }
}
