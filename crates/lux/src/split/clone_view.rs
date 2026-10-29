//! Cloned views and shared buffer management across split panes (ST-SPLIT-03).

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;

use crate::app::App;
use crate::split::LayoutPreset;

/// Shifts cursors in an inactive pane when edits occur in a shared underlying buffer.
pub(crate) fn shift_pane_cursors(
    cursors: &mut MultiCursor,
    buffer: &lux_core::buffer::Buffer,
    edits: &[lux_core::edit::Edit],
) {
    let doc_end = Position::document_end(buffer);
    let mut selections = Vec::with_capacity(cursors.count());

    for s in cursors.cursors() {
        let mut anchor_char = s.anchor.char;
        let mut head_char = s.head.char;

        for edit in edits {
            match edit {
                lux_core::edit::Edit::Insert { char_idx, text } => {
                    let ins_len = text.chars().count();
                    if anchor_char > *char_idx {
                        anchor_char += ins_len;
                    }
                    if head_char > *char_idx {
                        head_char += ins_len;
                    }
                }
                lux_core::edit::Edit::Delete { char_idx, text } => {
                    let del_len = text.chars().count();
                    let del_end = *char_idx + del_len;
                    if anchor_char >= del_end {
                        anchor_char = anchor_char.saturating_sub(del_len);
                    } else if anchor_char > *char_idx {
                        anchor_char = *char_idx;
                    }
                    if head_char >= del_end {
                        head_char = head_char.saturating_sub(del_len);
                    } else if head_char > *char_idx {
                        head_char = *char_idx;
                    }
                }
            }
        }

        anchor_char = anchor_char.min(doc_end.char);
        head_char = head_char.min(doc_end.char);

        let new_anchor = Position::from_char(buffer, anchor_char);
        let new_head = Position::from_char(buffer, head_char);
        selections.push(Selection::new(new_anchor, new_head));
    }

    if !selections.is_empty() {
        *cursors = MultiCursor::from_cursors(selections);
    }
}

/// Sychronizes cursors in all other split panes viewing the same underlying buffer after an edit.
pub(crate) fn sync_cloned_panes_after_edit(app: &mut App, edits: &[lux_core::edit::Edit]) {
    let active_pane = app.ui.split.active_pane;
    let active_tab = app.documents.active_tab;
    let active_bid = app.documents.active_buffer_id();

    for (idx, pane) in app.ui.split.panes.iter_mut().enumerate() {
        if idx == active_pane {
            continue;
        }
        let is_cloned = if let Some(at) = active_tab {
            pane.tab_id == at
        } else {
            false
        } || (active_bid.is_some()
            && app.documents.buffer_id_for_tab(pane.tab_id) == active_bid);

        if is_cloned {
            shift_pane_cursors(&mut pane.cursors, &app.buffer, edits);
        }
    }

    if app.ui.split.panes.len() > 1 {
        app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
    }
}

/// Clamps cursors in all inactive cloned panes after undo or redo operations.
pub(crate) fn clamp_cloned_panes_after_undo(app: &mut App) {
    if !app.ui.split.is_active() {
        return;
    }
    let active_pane = app.ui.split.active_pane;
    let active_tab = app.documents.active_tab;
    let active_bid = app.documents.active_buffer_id();

    for (idx, pane) in app.ui.split.panes.iter_mut().enumerate() {
        if idx == active_pane {
            continue;
        }
        let is_cloned = if let Some(at) = active_tab {
            pane.tab_id == at
        } else {
            false
        } || (active_bid.is_some()
            && app.documents.buffer_id_for_tab(pane.tab_id) == active_bid);

        if is_cloned {
            let doc_end = Position::document_end(&app.buffer);
            let clamped: Vec<_> = pane
                .cursors
                .cursors()
                .iter()
                .map(|s| {
                    let a = s.anchor.char.min(doc_end.char);
                    let h = s.head.char.min(doc_end.char);
                    Selection::new(
                        Position::from_char(&app.buffer, a),
                        Position::from_char(&app.buffer, h),
                    )
                })
                .collect();
            pane.cursors = MultiCursor::from_cursors(clamped);
        }
    }
    if app.ui.split.panes.len() > 1 {
        app.ui.split.secondary = app.ui.split.panes.get(1).cloned();
    }
}

impl App {
    /// Clones the active document view into another split pane/group (ST-SPLIT-03).
    /// If in single layout mode, opens a 2-column split first.
    pub(crate) fn clone_view_to_group(&mut self, target_group: Option<usize>) -> bool {
        let active_tab = self
            .documents
            .active_tab
            .or_else(|| self.documents.tabs.tabs().first().map(|t| t.id));
        let Some(active_tab) = active_tab else {
            return false;
        };

        if !self.ui.split.is_active() || self.ui.split.preset == LayoutPreset::Single {
            self.set_layout(LayoutPreset::Columns2);
        }

        let num_panes = self.ui.split.num_panes();
        if num_panes < 2 {
            return false;
        }

        let cur_pane = self.ui.split.active_pane;
        let target = target_group.unwrap_or_else(|| (cur_pane + 1) % num_panes);
        if target >= num_panes {
            return false;
        }

        // Save current active pane's view state before transferring focus
        if cur_pane < self.ui.split.panes.len() {
            self.ui.split.panes[cur_pane].cursors = self.cursors.clone();
            self.ui.split.panes[cur_pane].fold_state = self.fold_state.clone();
            self.ui.split.panes[cur_pane].bookmarks = self.bookmarks.clone();
            self.ui.split.panes[cur_pane].viewport.scroll_x = self.viewport.scroll_x;
            self.ui.split.panes[cur_pane].viewport.scroll_y = self.viewport.scroll_y;
            if let Some(syntax) = &self.syntax {
                self.ui.split.panes[cur_pane].tokens = syntax.tokens.clone();
            }
        }

        // Configure target pane with a clone of the current document view
        if target < self.ui.split.panes.len() {
            self.ui.split.panes[target].tab_id = active_tab;
            self.ui.split.panes[target].active_tab = active_tab;
            self.ui.split.panes[target].cursors = self.cursors.clone();
            self.ui.split.panes[target].fold_state = self.fold_state.clone();
            self.ui.split.panes[target].bookmarks = self.bookmarks.clone();
            self.ui.split.panes[target].viewport.scroll_x = self.viewport.scroll_x;
            self.ui.split.panes[target].viewport.scroll_y = self.viewport.scroll_y;
            if let Some(syntax) = &self.syntax {
                self.ui.split.panes[target].tokens = syntax.tokens.clone();
            }
        }

        if target == 1 || cur_pane == 1 {
            self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
        }

        self.focus_group(target)
    }

    /// Creates a second tab and view into the current file/buffer (ST-SPLIT-03).
    pub(crate) fn new_view_into_file(&mut self) -> bool {
        let active_tab = self.documents.active_tab;
        let Some(active_tab) = active_tab else {
            return false;
        };
        self.sync_active_document_to_entry();

        let (title, path, buf_id) = {
            let entry = self
                .documents
                .entries
                .iter()
                .find(|e| e.tab_id == active_tab);
            let Some(entry) = entry else {
                return false;
            };
            let title = entry
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|f| f.to_str())
                .unwrap_or("Untitled")
                .to_string();
            (title, entry.path.clone(), entry.buffer_id)
        };

        let new_tab_id = self.documents.tabs.open(&title);
        let mut new_entry =
            crate::document::DocumentEntry::new(new_tab_id, path, self.buffer.clone());
        new_entry.buffer_id = buf_id;
        new_entry.history = self.history.clone();
        new_entry.saved_version = self.documents.saved_version;
        new_entry.disk_snapshot = self.documents.disk_snapshot.clone();
        new_entry.cursors = self.cursors.clone();
        new_entry.goal_columns = self.goal_columns.clone();
        new_entry.scroll_x = self.viewport.scroll_x;
        new_entry.scroll_y = self.viewport.scroll_y;
        new_entry.fold_state = self.fold_state.clone();
        new_entry.bookmarks = self.bookmarks.clone();
        new_entry.language = self.documents.language();
        self.documents.entries.push(new_entry);

        self.switch_to_tab(new_tab_id);
        if self.ui.split.is_active() && self.ui.split.active_pane < self.ui.split.panes.len() {
            self.ui.split.panes[self.ui.split.active_pane].tab_id = new_tab_id;
            self.ui.split.panes[self.ui.split.active_pane].active_tab = new_tab_id;
            if self.ui.split.active_pane == 1 {
                self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
            }
        }
        self.scheduler.invalidate_all();
        true
    }
}
