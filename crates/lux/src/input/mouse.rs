//! Mouse hit-testing and selection logic for App (including multi-pane support).

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_core::selection_ops;

use crate::app::App;

impl App {
    pub(crate) fn position_at_mouse(&mut self) -> Position {
        let (local_x, local_y, viewport) = if self.ui.split.is_active() {
            let sb_w = self.sidebar_width();
            let top = self.tab_bar_height();
            let primary_rect = [sb_w, top, self.viewport.width, self.viewport.height];
            let geom = crate::split::compute_layout_geometry(
                &self.ui.split,
                primary_rect,
                self.scale_factor,
            );
            let active_idx = self.ui.split.active_pane;
            let [px, py, pw, ph] = geom.pane_rects.get(active_idx).copied().unwrap_or([
                sb_w,
                top,
                self.viewport.width,
                self.viewport.height,
            ]);
            let pane_vp = lux_ui::editor_view::Viewport {
                width: pw,
                height: ph,
                scroll_x: self.viewport.scroll_x,
                scroll_y: self.viewport.scroll_y,
            };
            (
                (self.mouse.position.x as f32 - px).max(0.0),
                (self.mouse.position.y as f32 - py).max(0.0),
                pane_vp,
            )
        } else {
            (
                (self.mouse.position.x as f32 - self.sidebar_width()).max(0.0),
                (self.mouse.position.y as f32 - self.tab_bar_height()).max(0.0),
                self.viewport,
            )
        };

        let digits = self.buffer.len_lines().max(1).to_string().len();
        let gutter_w = self.editor_view.reference_width(&"0".repeat(digits));
        let left_padding = if self.ui.split.is_active() {
            crate::render::text::gutter_number_pad(self.scale_factor) + 22.0 * self.scale_factor
        } else {
            self.text_left() - self.sidebar_width() - gutter_w
        };

        self.editor_view.hit_test(
            &self.buffer,
            &viewport,
            local_x,
            local_y,
            left_padding,
            Some(&self.fold_state),
        )
    }

    pub(crate) fn begin_mouse_selection(&mut self) {
        let position = self.position_at_mouse();
        let local_x = if self.ui.split.is_active() {
            let sb_w = self.sidebar_width();
            let top = self.tab_bar_height();
            let primary_rect = [sb_w, top, self.viewport.width, self.viewport.height];
            let geom = crate::split::compute_layout_geometry(
                &self.ui.split,
                primary_rect,
                self.scale_factor,
            );
            let active_idx = self.ui.split.active_pane;
            let px = geom
                .pane_rects
                .get(active_idx)
                .map(|r| r[0])
                .unwrap_or(sb_w);
            (self.mouse.position.x as f32 - px).max(0.0)
        } else {
            (self.mouse.position.x as f32 - self.sidebar_width()).max(0.0)
        };

        if !self.is_distraction_free() && local_x <= 16.0 * self.scale_factor {
            self.toggle_bookmark_at_line(position.line);
            return;
        }
        if self.fold_state.is_line_folded(position.line) {
            let digits = self.buffer.len_lines().max(1).to_string().len();
            let gutter_w = self.editor_view.reference_width(&"0".repeat(digits));
            let text_left = crate::render::text::gutter_total_width(gutter_w, self.scale_factor)
                + 6.0 * self.scale_factor;
            let line_content = self.buffer.line_content(position.line).unwrap_or_default();
            let trimmed = line_content.trim_end_matches(['\r', '\n']);
            let text_w = self.editor_view.reference_width(trimmed);
            let pill_x = text_left + text_w - self.viewport.scroll_x + 6.0 * self.scale_factor;
            let pill_w = 26.0 * self.scale_factor;
            if local_x >= pill_x - 6.0 * self.scale_factor
                && local_x <= pill_x + pill_w + 6.0 * self.scale_factor
            {
                self.unfold_at_line(position.line);
                return;
            }
        }
        let anchor = if self.modifiers.shift && !self.command_modifier() {
            self.cursors.primary().anchor
        } else {
            position
        };
        self.mouse.selection_anchor = Some(anchor);
        if self.modifiers.alt {
            // Option starts a column selection. The drag replaces it.
            self.cursors = MultiCursor::new(Selection::new(anchor, position));
        } else if self.command_modifier() {
            let mut cursors = self.cursors.cursors().to_vec();
            cursors.push(Selection::cursor(position));
            self.cursors = MultiCursor::from_cursors(cursors);
        } else {
            self.cursors = MultiCursor::new(Selection::new(anchor, position));
        }
        self.snippet_session = None;
        self.reset_goal_columns();
    }

    pub(crate) fn update_mouse_selection(&mut self) {
        let Some(anchor) = self.mouse.selection_anchor else {
            return;
        };
        let position = self.position_at_mouse();
        if let Some(word) = self.mouse.word_anchor {
            let point = selection_ops::expand_to_word(&self.buffer, Selection::cursor(position));
            let (origin_start, origin_end) = word.ordered();
            let (point_start, point_end) = point.ordered();
            let start = if origin_start.char <= point_start.char {
                origin_start
            } else {
                point_start
            };
            let end = if origin_end.char >= point_end.char {
                origin_end
            } else {
                point_end
            };
            self.cursors = MultiCursor::new(Selection::new(start, end));
            self.snippet_session = None;
            self.reset_goal_columns();
            return;
        }
        if self.modifiers.alt {
            self.cursors = selection_ops::box_select(&self.buffer, anchor, position);
        } else if self.command_modifier() {
            let mut cursors: Vec<_> = self
                .cursors
                .cursors()
                .iter()
                .copied()
                .filter(|selection| selection.anchor != anchor)
                .collect();
            cursors.push(Selection::new(anchor, position));
            self.cursors = MultiCursor::from_cursors(cursors);
        } else {
            self.cursors = MultiCursor::new(Selection::new(anchor, position));
        }
        self.snippet_session = None;
        self.reset_goal_columns();
    }
}
