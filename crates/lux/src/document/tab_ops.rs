use lux_platform::PlatformContext;
use lux_platform::clipboard::Clipboard;
use lux_ui::tabs::TabId;
use std::path::{Path, PathBuf};

/// Window title label: the file name, and the parent folder when there is
/// one. The full path belongs in the tab tooltip, not the title bar.
pub(crate) fn window_document_label(path: &Path) -> String {
    let file = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Untitled".to_string());
    match path.parent().and_then(|parent| parent.file_name()) {
        Some(folder) => format!("{file} — {}", folder.to_string_lossy()),
        None => file,
    }
}

use crate::app::App;

pub(crate) const TAB_MENU_WIDTH: f32 = 180.0;
pub(crate) const TAB_MENU_ITEM_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabContextMenuItem {
    Close,
    CloseOthers,
    CloseToRight,
    CloseSaved,
    TogglePin,
    CopyFilePath,
    RevealInFinder,
}

impl TabContextMenuItem {
    pub(crate) fn label(&self, is_pinned: bool) -> &'static str {
        match self {
            Self::Close => "Close",
            Self::CloseOthers => "Close Others",
            Self::CloseToRight => "Close to the Right",
            Self::CloseSaved => "Close Saved",
            Self::TogglePin => {
                if is_pinned {
                    "Unpin Tab"
                } else {
                    "Pin Tab"
                }
            }
            Self::CopyFilePath => "Copy File Path",
            Self::RevealInFinder => "Reveal in Finder",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TabContextMenu {
    pub(crate) tab_id: TabId,
    pub(crate) is_pinned: bool,
    pub(crate) rect: [f32; 4],
    pub(crate) items: Vec<TabContextMenuItem>,
}

impl TabContextMenu {
    pub(crate) fn new(tab_id: TabId, is_pinned: bool, x: f32, y: f32, scale: f32) -> Self {
        let items = vec![
            TabContextMenuItem::Close,
            TabContextMenuItem::CloseOthers,
            TabContextMenuItem::CloseToRight,
            TabContextMenuItem::CloseSaved,
            TabContextMenuItem::TogglePin,
            TabContextMenuItem::CopyFilePath,
            TabContextMenuItem::RevealInFinder,
        ];
        let width = TAB_MENU_WIDTH * scale;
        let height = (items.len() as f32 * TAB_MENU_ITEM_HEIGHT + 8.0) * scale;
        Self {
            tab_id,
            is_pinned,
            rect: [x, y, width, height],
            items,
        }
    }

    pub(crate) fn item_at(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        scale: f32,
    ) -> Option<TabContextMenuItem> {
        let [x, y, w, h] = self.rect;
        if mouse_x < x || mouse_x > x + w || mouse_y < y || mouse_y > y + h {
            return None;
        }
        let item_h = TAB_MENU_ITEM_HEIGHT * scale;
        let offset_y = mouse_y - y - 4.0 * scale;
        if offset_y < 0.0 {
            return None;
        }
        let index = (offset_y / item_h) as usize;
        self.items.get(index).copied()
    }
}

impl App {
    /// Switches to the tab at display index (0-indexed).
    pub(crate) fn switch_to_tab_index(&mut self, index: usize) -> bool {
        let tabs = self.documents.tabs.tabs();
        if tabs.is_empty() {
            return false;
        }
        let target_idx = index.min(tabs.len() - 1);
        self.switch_to_tab(tabs[target_idx].id)
    }

    /// Switches to the very last tab in the tab bar.
    pub(crate) fn switch_to_last_tab(&mut self) -> bool {
        let count = self.documents.tabs.tabs().len();
        if count == 0 {
            return false;
        }
        self.switch_to_tab_index(count - 1)
    }

    /// Cycles tab activation forward or backward.
    pub(crate) fn cycle_tab(&mut self, forward: bool) -> bool {
        let tabs = self.documents.tabs.tabs();
        if tabs.len() <= 1 {
            return false;
        }
        let Some(pos) = tabs
            .iter()
            .position(|t| Some(t.id) == self.documents.active_tab)
        else {
            return false;
        };
        let next = if forward {
            (pos + 1) % tabs.len()
        } else {
            (pos + tabs.len() - 1) % tabs.len()
        };
        self.switch_to_tab(tabs[next].id)
    }

    pub(crate) fn switch_to_next_tab(&mut self) -> bool {
        self.cycle_tab(true)
    }

    pub(crate) fn switch_to_prev_tab(&mut self) -> bool {
        self.cycle_tab(false)
    }

    pub(crate) fn reopen_last_closed_tab(&mut self) -> bool {
        let Some(reopened_id) = self.documents.tabs.reopen_last_closed() else {
            return false;
        };
        let Some(mut entry) = self.documents.closed_entries.pop() else {
            return false;
        };
        entry.tab_id = reopened_id;
        self.documents.entries.push(entry);
        self.switch_to_tab(reopened_id)
    }

    /// Closes all open tabs except the target tab.
    pub(crate) fn close_other_tabs(&mut self, ctx: &dyn PlatformContext, target_id: TabId) {
        self.switch_to_tab(target_id);
        let other_ids: Vec<TabId> = self
            .documents
            .tabs
            .tabs()
            .iter()
            .filter(|t| t.id != target_id)
            .map(|t| t.id)
            .collect();
        for id in other_ids {
            self.close_tab(ctx, id);
        }
    }

    /// Closes all tabs displayed to the right of the target tab.
    pub(crate) fn close_tabs_to_right(&mut self, ctx: &dyn PlatformContext, target_id: TabId) {
        let tabs = self.documents.tabs.tabs().to_vec();
        if let Some(pos) = tabs.iter().position(|t| t.id == target_id) {
            let right_ids: Vec<TabId> = tabs[pos + 1..].iter().map(|t| t.id).collect();
            for id in right_ids {
                self.close_tab(ctx, id);
            }
        }
    }

    /// Closes all unmodified (clean) tabs.
    pub(crate) fn close_saved_tabs(&mut self, ctx: &dyn PlatformContext) {
        let clean_ids: Vec<TabId> = self
            .documents
            .tabs
            .tabs()
            .iter()
            .filter(|t| {
                if Some(t.id) == self.documents.active_tab {
                    !self.document_dirty()
                } else {
                    self.documents
                        .entries
                        .iter()
                        .find(|e| e.tab_id == t.id)
                        .is_some_and(|e| !e.is_dirty())
                }
            })
            .map(|t| t.id)
            .collect();
        for id in clean_ids {
            self.close_tab(ctx, id);
        }
    }

    /// Toggles the pinned state of a tab.
    pub(crate) fn toggle_pin_tab(&mut self, target_id: TabId) {
        let is_pinned = self
            .documents
            .tabs
            .tabs()
            .iter()
            .find(|t| t.id == target_id)
            .is_some_and(|t| t.pinned);
        if is_pinned {
            self.documents.tabs.unpin(target_id);
        } else {
            self.documents.tabs.pin(target_id);
        }
        self.scheduler.invalidate_all();
    }

    /// Reorders a tab to a new display index (Drag & Drop).
    #[allow(dead_code)]
    pub(crate) fn reorder_tab(&mut self, target_id: TabId, to_index: usize) -> bool {
        let moved = self.documents.tabs.move_tab(target_id, to_index);
        if moved {
            self.scheduler.invalidate_all();
        }
        moved
    }

    /// Copies the file path of the given tab to the system clipboard.
    pub(crate) fn copy_tab_file_path(
        &mut self,
        clipboard: Option<&dyn Clipboard>,
        target_id: TabId,
    ) -> bool {
        let path = self.tab_path(target_id);
        let Some(p) = path else {
            return false;
        };
        let text = p.to_string_lossy().to_string();
        if let Some(cb) = clipboard {
            let _ = cb.write_text(&text);
        }
        self.documents.message = Some(format!("Copied path: {text}"));
        true
    }

    /// Reveals the file of the given tab in the OS file manager.
    pub(crate) fn reveal_tab_in_finder(&self, target_id: TabId) {
        let Some(path) = self.tab_path(target_id) else {
            return;
        };
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(&path)
                .spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn();
        }
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            if let Some(parent) = path.parent() {
                let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
            }
        }
    }

    fn tab_path(&self, target_id: TabId) -> Option<PathBuf> {
        if Some(target_id) == self.documents.active_tab {
            self.documents.path.clone()
        } else {
            self.documents
                .entries
                .iter()
                .find(|e| e.tab_id == target_id)
                .and_then(|e| e.path.clone())
        }
    }

    pub(crate) fn update_document_title(&self) {
        let Some(gpu) = &self.gpu_window else {
            return;
        };
        let name = self
            .documents
            .path
            .as_ref()
            .map(|p| window_document_label(p))
            .unwrap_or_else(|| "Untitled".into());
        let dirty = if self.document_dirty() { " *" } else { "" };
        let readonly = if self.buffer.is_huge_file_mode() {
            " [read-only: large file]"
        } else {
            ""
        };
        let status = if self.documents.pending.is_some() {
            " — File operation in progress".to_string()
        } else {
            self.documents
                .message
                .as_ref()
                .map(|m| format!(" — {m}"))
                .unwrap_or_default()
        };
        gpu.window
            .set_title(&format!("{name}{dirty}{readonly} — Lux{status}"));
    }

    pub(crate) fn close_tab(&mut self, ctx: &dyn PlatformContext, target_id: TabId) {
        if self.documents.active_tab == Some(target_id) {
            self.request_document(ctx, crate::document::Request::Close);
            return;
        }
        let is_dirty = self
            .documents
            .entries
            .iter()
            .find(|e| e.tab_id == target_id)
            .is_some_and(|e| e.is_dirty());
        if is_dirty {
            self.switch_to_tab(target_id);
            self.request_document(ctx, crate::document::Request::Close);
        } else {
            self.documents.tabs.close(target_id);
            if let Some(pos) = self
                .documents
                .entries
                .iter()
                .position(|e| e.tab_id == target_id)
            {
                let removed = self.documents.entries.remove(pos);
                self.documents.closed_entries.push(removed);
            }
            if self.ui.split.is_active() {
                let fallback = self
                    .documents
                    .active_tab
                    .or_else(|| self.documents.tabs.tabs().first().map(|t| t.id));
                if let Some(fb) = fallback {
                    for pane in &mut self.ui.split.panes {
                        if pane.tab_id == target_id {
                            pane.tab_id = fb;
                            pane.active_tab = fb;
                        }
                    }
                    self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
                }
            }
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_title_is_the_file_and_its_folder() {
        let path = Path::new("/Users/runner/work/lux/lux/docs/compare/fixtures/monokai.rs");
        assert_eq!(window_document_label(path), "monokai.rs — fixtures");
        assert_eq!(window_document_label(Path::new("monokai.rs")), "monokai.rs");
    }
}
