//! The document request a worker is running, and which tab it belongs to.
//!
//! On macOS the save and confirmation dialogs are window sheets, so the
//! event loop keeps running while a worker waits on one: the user can switch
//! tabs before the answer comes back. The result belongs to the tab that
//! asked, not to whichever tab is active when it lands.

use lux_core::multicursor::MultiCursor;
use lux_platform::PlatformContext;
use lux_ui::tabs::TabId;

use super::Request;
use crate::app::App;

pub(super) struct Pending {
    pub(super) request: Request,
    pub(super) version: u64,
    /// Undo on-save whitespace edits when the worker rejects the save.
    pub(super) revert_transform: bool,
    /// The tab that was active when the request started.
    pub(super) tab: Option<TabId>,
}

/// Where the user was when a result for another tab landed.
pub(super) enum Focus {
    /// The owning tab is active; nothing to restore.
    Owner,
    /// The owning tab was activated to apply the result; return here after.
    Return(TabId),
    /// The owning tab is gone. Results for it are dropped.
    OwnerGone,
}

impl App {
    /// Whether `tab` has a dialog or a document request in flight.
    pub(crate) fn tab_has_pending_request(&self, tab: TabId) -> bool {
        self.documents
            .pending
            .as_ref()
            .is_some_and(|pending| pending.tab == Some(tab))
    }

    /// Makes the tab that started `pending` active, so its result is
    /// applied to that tab's buffer, history and title.
    pub(super) fn focus_pending_owner(&mut self, pending: &Pending) -> Focus {
        let Some(owner) = pending.tab else {
            return Focus::Owner;
        };
        let active = self.documents.active_tab;
        if active == Some(owner) {
            return Focus::Owner;
        }
        if !self.documents.entries.iter().any(|e| e.tab_id == owner) {
            return Focus::OwnerGone;
        }
        self.switch_to_tab(owner);
        match active {
            Some(active) => Focus::Return(active),
            None => Focus::Owner,
        }
    }

    /// Goes back to the tab the user was on, unless the result opened a
    /// document (that tab is now the one to show).
    pub(super) fn restore_focus(&mut self, focus: Focus, opened: bool) {
        if let Focus::Return(tab) = focus
            && !opened
            && self.documents.entries.iter().any(|e| e.tab_id == tab)
        {
            self.switch_to_tab(tab);
        }
    }

    /// Closes the active tab after a confirmed Close; the last tab exits.
    pub(super) fn close_finished_tab(&mut self, ctx: &dyn PlatformContext) {
        if self.documents.tabs.tabs().len() <= 1 {
            if let Some(active) = self.documents.tabs.active() {
                crate::cli::release_tab(active.id);
                self.documents.tabs.close(active.id);
            }
            crate::cli::release_all();
            ctx.exit();
            return;
        }
        let Some(active_id) = self.documents.active_tab else {
            return;
        };
        crate::cli::release_tab(active_id);
        self.documents.tabs.close(active_id);
        if let Some(pos) = self
            .documents
            .entries
            .iter()
            .position(|e| e.tab_id == active_id)
        {
            let removed = self.documents.entries.remove(pos);
            self.documents.closed_entries.push(removed);
        }
        if !crate::split::tabs::after_active_closed(self, active_id)
            && let Some(new_active) = self.documents.tabs.active()
        {
            let new_id = new_active.id;
            self.documents.active_tab = None;
            self.switch_to_tab(new_id);
        }
    }

    /// Drops on-save whitespace edits when the worker did not write the
    /// file. Nothing to drop when the owning tab was closed meanwhile.
    pub(super) fn revert_rejected_save_transform(&mut self, pending: &Pending, owner_gone: bool) {
        if owner_gone || !pending.revert_transform || self.history.state_id() != pending.version {
            return;
        }
        if let Some(selections) = self.history.undo(&mut self.buffer) {
            self.cursors = MultiCursor::from_cursors(selections);
            self.reset_goal_columns();
            self.sync_syntax_full();
        }
    }
}

#[cfg(test)]
#[path = "pending_tests.rs"]
mod tests;
