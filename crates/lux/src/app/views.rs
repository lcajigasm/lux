//! UI view toggle and opening methods for App.

use std::path::PathBuf;

use crate::app::{App, TAB_BAR_HEIGHT, TAB_MAX_WIDTH, TAB_MIN_WIDTH};

impl App {
    pub(crate) fn toggle_minimap(&mut self) {
        self.ui.minimap.toggle();
        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }

    pub(crate) fn toggle_word_wrap(&mut self) {
        let enabled = !self.editor_view.metrics().word_wrap;
        self.editor_view.metrics_mut().word_wrap = enabled;
        if enabled {
            self.viewport.scroll_x = 0.0;
        }
        self.ui
            .settings
            .write_user_value("editor.wordWrap", serde_json::Value::Bool(enabled));
        self.scheduler.invalidate_all();
    }

    pub(crate) fn toggle_command_palette(&mut self) {
        if self.ui.command_palette.is_some() {
            self.ui.command_palette = None;
            crate::focus::focus_editor(self);
            self.scheduler.invalidate_all();
        } else {
            self.open_command_palette("");
        }
    }

    pub(crate) fn open_command_palette(&mut self, query: &str) {
        let mut state = crate::palette::CommandPaletteState::new();
        state.registry.import_usage(
            self.ui.settings.palette_uses.clone(),
            self.ui.settings.palette_clock,
        );
        state.query = crate::field::QueryField::from_text(query);
        state.shortcuts = crate::palette::shortcut_labels(&self.keymap);
        self.ui.command_palette = Some(state);
        self.chrome
            .focus
            .set_focus(lux_ui::focus::FocusTarget::CommandPalette);
        self.scheduler.invalidate_all();
    }

    pub(crate) fn toggle_goto_anything(&mut self) {
        if self.ui.goto_anything.is_some() {
            if let Some(mut goto) = self.ui.goto_anything.take()
                && let Some(snapshot) = goto.snapshot.take()
            {
                if let Some(orig_tab) = snapshot.active_tab
                    && self.documents.active_tab != Some(orig_tab)
                {
                    self.switch_to_tab(orig_tab);
                }
                self.cursors = snapshot.cursors;
                self.viewport.scroll_x = snapshot.scroll_x;
                self.viewport.scroll_y = snapshot.scroll_y;
                self.reset_goal_columns();
            }
            crate::focus::focus_editor(self);
            self.scheduler.invalidate_all();
        } else {
            self.open_goto_anything("");
        }
    }

    pub(crate) fn open_goto_anything(&mut self, initial_query: &str) {
        let mut state = crate::goto::GotoState::new(self);
        state.query = crate::field::QueryField::from_text(initial_query);
        state.refresh_results(self);
        self.ui.goto_anything = Some(state);
        self.chrome
            .focus
            .set_focus(lux_ui::focus::FocusTarget::CommandPalette);
        crate::goto::update_live_preview(self);
        self.scheduler.invalidate_all();
    }

    pub(crate) fn open_goto_project_symbols(&mut self) {
        let mut state = crate::goto::GotoState::new_project_symbols(self);
        state.refresh_results(self);
        self.ui.goto_anything = Some(state);
        self.chrome
            .focus
            .set_focus(lux_ui::focus::FocusTarget::CommandPalette);
        crate::goto::update_live_preview(self);
        self.scheduler.invalidate_all();
    }

    pub(crate) fn open_find(&mut self) {
        crate::search::open_find(self, false);
    }

    pub(crate) fn open_replace(&mut self) {
        crate::search::open_find(self, true);
    }

    pub(crate) fn open_workspace(&mut self, path: PathBuf) {
        let session_file = crate::session::default_session_path(Some(&path));
        let restored = if session_file.exists() {
            crate::session::restore_session(self, &session_file).unwrap_or(false)
        } else {
            false
        };
        if !restored {
            self.ui.sidebar = crate::sidebar::SidebarState::open_folder(path.clone());
            self.ui.session.init_watcher(&path);
        }
        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }

    pub(crate) fn open_folder_dialog(&mut self, ctx: &dyn lux_platform::PlatformContext) {
        let opts = lux_platform::FileDialogOptions {
            title: Some("Open Folder".into()),
            starting_directory: self.workspace_root(),
            filters: Vec::new(),
        };
        if let Some(path) = ctx.file_dialogs().pick_folder(&opts) {
            self.open_workspace(path);
        }
    }

    pub(crate) fn open_about_dialog(&mut self) {
        self.ui.dialogs.about = Some(crate::about::AboutDialogState::new());
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn toggle_sidebar(&mut self) {
        if let Some(sidebar) = &mut self.ui.sidebar {
            sidebar.visible = !sidebar.visible;
        } else {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            self.ui.sidebar = crate::sidebar::SidebarState::open_folder(cwd);
        }
        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }

    pub(crate) fn open_project_find(&mut self) {
        crate::project_search::open_project_find(self);
    }

    pub(crate) fn open_project_replace(&mut self) {
        crate::project_search::open_project_replace(self);
    }

    pub(crate) fn tab_bar_height(&self) -> f32 {
        if self.ui.distraction_free.is_some() {
            return 0.0;
        }
        TAB_BAR_HEIGHT * self.scale_factor
    }

    pub(crate) fn status_bar_height(&self) -> f32 {
        if self.ui.distraction_free.is_some() {
            return 0.0;
        }
        crate::status_bar::STATUS_BAR_HEIGHT * self.scale_factor
    }

    pub(crate) fn search_bar_height(&self) -> f32 {
        self.ui
            .search_bar
            .as_ref()
            .map_or(0.0, |s| s.height(self.scale_factor))
    }

    pub(crate) fn term_dock_height(&self) -> f32 {
        if self.ui.term.visible {
            self.ui.term.height * self.scale_factor
        } else {
            0.0
        }
    }

    pub(crate) fn workspace_root(&self) -> Option<PathBuf> {
        self.ui
            .project
            .active_project_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| {
                self.ui
                    .sidebar
                    .as_ref()
                    .and_then(|s| s.workspace_root.clone())
            })
    }

    pub(crate) fn sidebar_width(&self) -> f32 {
        if self.ui.distraction_free.is_some() {
            return 0.0;
        }
        self.ui
            .sidebar
            .as_ref()
            .filter(|s| s.visible && s.tree.is_some())
            .map_or(0.0, |s| s.width * self.scale_factor)
    }

    pub(crate) fn minimap_width(&self) -> f32 {
        if self.ui.distraction_free.is_some() {
            return 0.0;
        }
        self.ui.minimap.effective_width(self.scale_factor)
    }

    pub(crate) fn update_viewport_dimensions(&mut self) {
        if let Some(gpu_window) = &self.gpu_window {
            let size = gpu_window.window.inner_size();
            let tab_bar_height = self.tab_bar_height();
            let status_bar_height = self.status_bar_height();
            let search_bar_height = self.search_bar_height();
            let term_dock_height = self.term_dock_height();
            let sidebar_width = self.sidebar_width();
            let minimap_width = self.minimap_width();
            self.viewport.width = (size.width as f32 - sidebar_width - minimap_width).max(1.0);
            self.viewport.height = (size.height as f32
                - tab_bar_height
                - status_bar_height
                - search_bar_height
                - term_dock_height)
                .max(1.0);
        }
    }

    pub(crate) fn tab_width(&self) -> f32 {
        let count = self.documents.tabs.tabs().len().max(1) as f32;
        let per_tab = self.viewport.width / count;
        per_tab.clamp(
            TAB_MIN_WIDTH * self.scale_factor,
            TAB_MAX_WIDTH * self.scale_factor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_directory_shows_that_folder() {
        let dir = std::env::temp_dir().join(format!("lux-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp workspace");
        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.open_workspace(dir);
        let sidebar = app.ui.sidebar.expect("workspace sidebar");
        assert!(sidebar.visible);
        assert!(sidebar.tree.is_some());
    }
}
