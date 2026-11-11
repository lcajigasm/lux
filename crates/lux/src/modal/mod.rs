//! Unified hit-testing, mouse click handling, row activation, and live hover
//! for all floating modal dialogs (ST-FIX-01).

use lux_platform::PlatformContext;

use crate::app::App;

mod palette_goto;
mod pickers;
mod search;

#[cfg(test)]
mod tests;

pub(crate) fn is_any_modal_open(app: &App) -> bool {
    app.ui.command_palette.is_some()
        || app.ui.goto_anything.is_some()
        || app.ui.git.branch_picker.is_some()
        || app.ui.language_picker.is_some()
        || app.ui.theme_picker.is_some()
        || app.ui.snippets.picker.is_some()
        || app.ui.build.picker.is_some()
        || app.ui.project.picker.is_some()
        || app.ui.project_search.is_some()
        || app.ui.dialogs.about.is_some()
}

pub(crate) fn modal_viewport_width(app: &App) -> f32 {
    app.gpu_window
        .as_ref()
        .map(|g| g.window.inner_size().width as f32)
        .unwrap_or(app.viewport.width)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ModalMetrics {
    pub(crate) vp_width: f32,
    pub(crate) tab_bar_height: f32,
    pub(crate) scale: f32,
}

impl ModalMetrics {
    pub(crate) fn from_app(app: &App) -> Self {
        Self {
            vp_width: modal_viewport_width(app),
            tab_bar_height: app.tab_bar_height(),
            scale: app.scale_factor,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ModalGeometry {
    pub(crate) left: f32,
    pub(crate) top: f32,
    pub(crate) width: f32,
    pub(crate) total_height: f32,
    pub(crate) rows_top: f32,
    pub(crate) row_height: f32,
    pub(crate) visible_count: usize,
    pub(crate) start_idx: usize,
}

impl ModalGeometry {
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left
            && x <= self.left + self.width
            && y >= self.top
            && y <= self.top + self.total_height
    }

    pub(crate) fn row_index_at(&self, y: f32) -> Option<usize> {
        let rows_bottom = self.rows_top + (self.visible_count as f32 * self.row_height);
        if y >= self.rows_top && y < rows_bottom && self.visible_count > 0 && self.row_height > 0.0
        {
            Some(self.start_idx + ((y - self.rows_top) / self.row_height) as usize)
        } else {
            None
        }
    }
}

/// Handles left mouse clicks when any modal is open.
///
/// Returns `true` if a modal was open and consumed the event:
/// - Clicking a result row immediately executes/activates that item.
/// - Clicking search input or header fields keeps the modal open and focuses fields if applicable.
/// - Clicking outside the modal dismisses it cleanly (restoring snapshots or original theme).
///
/// In all cases, click-through to background text editors or tabs is prevented.
pub(crate) fn handle_modal_click(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    if !is_any_modal_open(app) {
        return false;
    }

    crate::about::click_about(app, mouse_x, mouse_y)
        || palette_goto::click_command_palette(app, ctx, mouse_x, mouse_y)
        || palette_goto::click_goto_anything(app, ctx, mouse_x, mouse_y)
        || pickers::click_branch_picker(app, mouse_x, mouse_y)
        || pickers::click_language_picker(app, mouse_x, mouse_y)
        || pickers::click_theme_picker(app, mouse_x, mouse_y)
        || pickers::click_snippet_picker(app, mouse_x, mouse_y)
        || pickers::click_build_picker(app, ctx, mouse_x, mouse_y)
        || pickers::click_project_picker(app, ctx, mouse_x, mouse_y)
        || search::click_project_search(app, ctx, mouse_x, mouse_y)
}

/// Handles mouse movement when any modal is open, updating visual row selection and previews.
pub(crate) fn handle_modal_hover(app: &mut App, mouse_x: f32, mouse_y: f32) -> bool {
    if !is_any_modal_open(app) {
        return false;
    }

    crate::about::hover_about(app, mouse_x, mouse_y)
        || palette_goto::hover_command_palette(app, mouse_x, mouse_y)
        || palette_goto::hover_goto_anything(app, mouse_x, mouse_y)
        || pickers::hover_language_picker(app, mouse_x, mouse_y)
        || pickers::hover_theme_picker(app, mouse_x, mouse_y)
        || pickers::hover_snippet_picker(app, mouse_x, mouse_y)
        || pickers::hover_build_picker(app, mouse_x, mouse_y)
        || pickers::hover_project_picker(app, mouse_x, mouse_y)
        || search::hover_project_search(app, mouse_x, mouse_y)
}
