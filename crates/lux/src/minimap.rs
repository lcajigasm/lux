//! Minimap state and interaction for the Lux desktop application.

use lux_ui::minimap::Minimap;

pub(crate) const MINIMAP_WIDTH: f32 = 90.0;
pub(crate) const MINIMAP_ROW_HEIGHT: f32 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MinimapState {
    pub(crate) inner: Minimap,
    pub(crate) width: f32,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            inner: Minimap::new(),
            width: MINIMAP_WIDTH,
        }
    }
}

impl MinimapState {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn toggle(&mut self) {
        self.inner.toggle();
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.enabled
    }

    pub(crate) fn effective_width(&self, scale_factor: f32) -> f32 {
        if self.is_enabled() {
            self.width * scale_factor
        } else {
            0.0
        }
    }
}

pub(crate) fn handle_minimap_click(app: &mut crate::app::App, mouse_x: f32, mouse_y: f32) -> bool {
    let scale = app.scale_factor;
    let minimap_w = app.ui.minimap.effective_width(scale);
    if minimap_w <= 0.0 {
        return false;
    }

    let Some(gpu_window) = &app.gpu_window else {
        return false;
    };
    let window_w = gpu_window.window.inner_size().width as f32;
    let minimap_x = window_w - minimap_w;
    if mouse_x < minimap_x || mouse_x > window_w {
        return false;
    }

    let tab_bar_h = app.tab_bar_height();
    let status_bar_h = app.status_bar_height();
    let window_h = gpu_window.window.inner_size().height as f32;
    let minimap_bottom = window_h - status_bar_h;

    if mouse_y < tab_bar_h || mouse_y >= minimap_bottom {
        return false;
    }

    let available_height = minimap_bottom - tab_bar_h;
    let row_h = MINIMAP_ROW_HEIGHT * scale;
    let available_rows = (available_height / row_h).floor() as usize;
    if available_rows == 0 {
        return false;
    }

    let clicked_row = ((mouse_y - tab_bar_h) / row_h).floor() as usize;
    let total_lines = app.buffer.len_lines();
    let target_line = lux_ui::minimap::line_for_row_with_folds(
        total_lines,
        available_rows,
        clicked_row,
        Some(&app.fold_state),
    );

    let line_height = crate::app::LINE_HEIGHT * scale;
    let visual_row = app
        .fold_state
        .buffer_to_visual_line(target_line)
        .unwrap_or(0);
    let target_scroll_y = (visual_row as f32 * line_height - app.viewport.height * 0.5).max(0.0);
    app.viewport.scroll_y = target_scroll_y;
    app.scheduler.invalidate_all();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_ui::minimap::line_for_row;

    #[test]
    fn minimap_state_defaults_to_enabled() {
        let state = MinimapState::new();
        assert!(state.is_enabled());
        assert_eq!(state.width, MINIMAP_WIDTH);
    }

    #[test]
    fn minimap_state_toggle_and_width() {
        let mut state = MinimapState::new();
        assert_eq!(state.effective_width(2.0), 180.0);

        state.toggle();
        assert!(!state.is_enabled());
        assert_eq!(state.effective_width(2.0), 0.0);

        state.toggle();
        assert!(state.is_enabled());
        assert_eq!(state.effective_width(1.0), 90.0);
    }

    #[test]
    fn minimap_row_to_line_mapping() {
        let total_lines = 100;
        let available_rows = 50;
        let line_mid = line_for_row(total_lines, available_rows, 25);
        assert_eq!(line_mid, 50);

        let line_end = line_for_row(total_lines, available_rows, 49);
        assert_eq!(line_end, 98);

        let line_overflow = line_for_row(total_lines, available_rows, 100);
        assert_eq!(line_overflow, 99);
    }
}
