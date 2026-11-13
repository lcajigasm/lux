//! Mouse hit-testing and selection logic for App (including multi-pane support).

use lux_core::edit::Edit;
use lux_core::history::Transaction;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_core::selection_ops;

use crate::app::App;
use crate::app::SelectionDrag;

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

    pub(crate) fn begin_mouse_selection(&mut self, clicks: u8) {
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
        // A second or third click still selects a word or a line. Only the
        // first click inside an existing selection can drag it.
        if clicks == 1 && self.arm_selection_drag(position) {
            return;
        }
        self.mouse.selection_drag = None;
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
        if let Some(origin) = self.mouse.line_anchor.or(self.mouse.word_anchor) {
            let point = Selection::cursor(position);
            let point = if self.mouse.line_anchor.is_some() {
                selection_ops::expand_to_line(&self.buffer, point)
            } else {
                selection_ops::expand_to_word(&self.buffer, point)
            };
            let (origin_start, origin_end) = origin.ordered();
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

    /// Arms a move, or a copy when Option is held, and leaves the selection
    /// in place. A press that never leaves its origin is a caret click.
    fn arm_selection_drag(&mut self, position: Position) -> bool {
        if self.modifiers.shift || self.command_modifier() {
            return false;
        }
        let primary = self.cursors.primary();
        if primary.is_empty() {
            return false;
        }
        let (start, end) = primary.ordered();
        if position.char < start.char || position.char >= end.char {
            return false;
        }
        self.mouse.selection_drag = Some(SelectionDrag {
            origin: self.mouse.position,
            copy: self.modifiers.alt,
            active: false,
            selection: primary,
        });
        self.mouse.selection_anchor = None;
        true
    }

    pub(crate) fn update_selection_drag(&mut self) {
        let Some(drag) = self.mouse.selection_drag.as_mut() else {
            return;
        };
        let dx = self.mouse.position.x - drag.origin.x;
        let dy = self.mouse.position.y - drag.origin.y;
        if dx * dx + dy * dy >= 16.0 {
            drag.active = true;
        }
    }

    pub(crate) fn finish_selection_drag(&mut self) {
        let Some(drag) = self.mouse.selection_drag.take() else {
            return;
        };
        if !drag.active {
            let position = self.position_at_mouse();
            self.cursors = MultiCursor::new(Selection::cursor(position));
            self.snippet_session = None;
            self.reset_goal_columns();
            return;
        }
        let drop_at = self.position_at_mouse();
        self.apply_selection_drag(drag.selection, drop_at, drag.copy);
    }

    fn apply_selection_drag(&mut self, selection: Selection, drop_at: Position, copy: bool) {
        if self.buffer.is_huge_file_mode() || self.editing_blocked() {
            return;
        }
        let (start, end) = selection.ordered();
        let len = end.char.saturating_sub(start.char);
        if len == 0 {
            return;
        }
        let drop_char = drop_at.char.min(self.buffer.len_chars());
        if !copy && drop_char >= start.char && drop_char <= end.char {
            return;
        }
        let text = self.buffer.text_range(start.char..end.char);
        let before = self.cursors.cursors().to_vec();
        let mut edits = Vec::new();
        let inserted_at = if copy {
            drop_char
        } else if drop_char > end.char {
            let delete = Edit::delete(&self.buffer, start.char, len);
            delete.apply(&mut self.buffer);
            edits.push(delete);
            drop_char - len
        } else {
            drop_char
        };
        let insert = Edit::insert(inserted_at, text);
        insert.apply(&mut self.buffer);
        edits.push(insert);
        if !copy && drop_char <= start.char {
            let delete = Edit::delete(&self.buffer, start.char + len, len);
            delete.apply(&mut self.buffer);
            edits.push(delete);
        }
        let moved = Selection::new(
            Position::from_char(&self.buffer, inserted_at),
            Position::from_char(&self.buffer, inserted_at + len),
        );
        let transaction = Transaction::new(edits, before, vec![moved]);
        self.history.record(transaction.clone());
        self.cursors = MultiCursor::new(moved);
        self.reset_goal_columns();
        self.sync_syntax_incremental(&transaction.edits);
        if self.ui.split.is_active() {
            crate::split::clone_view::sync_cloned_panes_after_edit(self, &transaction.edits);
        }
    }
}

#[cfg(test)]
mod tests {
    use lux_core::multicursor::MultiCursor;
    use lux_core::position::Position;
    use lux_core::selection::Selection;

    use super::super::tests::{app, command};

    fn point_for_char(app: &mut crate::app::App, target: usize) -> dpi::PhysicalPosition<f64> {
        let y = app.tab_bar_height() as f64 + 2.0;
        let gutter = 16.0 * app.scale_factor as f64 + 1.0;
        for x in (gutter as i32..800).step_by(2) {
            app.mouse.position = dpi::PhysicalPosition::new(x as f64, y);
            if app.position_at_mouse().char == target {
                return app.mouse.position;
            }
        }
        panic!("no pointer position lands on char {target}");
    }

    fn select(app: &mut crate::app::App, start: usize, end: usize) {
        app.cursors = MultiCursor::new(Selection::new(
            Position::from_char(&app.buffer, start),
            Position::from_char(&app.buffer, end),
        ));
    }

    #[test]
    fn dragging_a_selection_moves_it_and_undo_puts_it_back() {
        let mut app = app("alpha beta gamma");
        select(&mut app, 0, 5);
        let from = point_for_char(&mut app, 1);
        app.mouse.position = from;
        app.begin_mouse_selection(1);
        assert!(app.mouse.selection_drag.is_some());
        assert_eq!(app.buffer.text(), "alpha beta gamma");
        app.mouse.position = point_for_char(&mut app, 16);
        app.update_selection_drag();
        app.finish_selection_drag();
        assert_eq!(app.buffer.text(), " beta gammaalpha");
        assert_eq!(
            app.buffer.text_range(app.cursors.primary().char_range()),
            "alpha"
        );
        command(&mut app, "z", false);
        assert_eq!(app.buffer.text(), "alpha beta gamma");
    }

    #[test]
    fn dragging_a_selection_up_moves_it_before_the_drop_point() {
        let mut app = app("alpha beta gamma");
        select(&mut app, 11, 16);
        app.mouse.position = point_for_char(&mut app, 12);
        app.begin_mouse_selection(1);
        app.mouse.position = point_for_char(&mut app, 0);
        app.update_selection_drag();
        app.finish_selection_drag();
        assert_eq!(app.buffer.text(), "gammaalpha beta ");
        assert_eq!(
            app.buffer.text_range(app.cursors.primary().char_range()),
            "gamma"
        );
    }

    #[test]
    fn option_drag_on_a_selection_copies_it() {
        let mut app = app("alpha beta gamma");
        select(&mut app, 6, 10);
        app.modifiers.alt = true;
        app.mouse.position = point_for_char(&mut app, 7);
        app.begin_mouse_selection(1);
        app.mouse.position = point_for_char(&mut app, 16);
        app.update_selection_drag();
        app.finish_selection_drag();
        assert_eq!(app.buffer.text(), "alpha beta gammabeta");
        assert_eq!(app.buffer.text_range(6..10), "beta");
        command(&mut app, "z", false);
        assert_eq!(app.buffer.text(), "alpha beta gamma");
    }

    #[test]
    fn a_click_inside_a_selection_places_the_caret() {
        let mut app = app("alpha beta gamma");
        select(&mut app, 0, 5);
        app.mouse.position = point_for_char(&mut app, 2);
        app.begin_mouse_selection(1);
        app.finish_selection_drag();
        assert!(app.cursors.primary().is_empty());
        assert_eq!(app.cursors.primary().head.char, 2);
        assert_eq!(app.buffer.text(), "alpha beta gamma");
    }

    #[test]
    fn dropping_inside_the_selection_leaves_it() {
        let mut app = app("alpha beta gamma");
        select(&mut app, 0, 16);
        app.mouse.position = point_for_char(&mut app, 1);
        app.begin_mouse_selection(1);
        app.mouse.position = point_for_char(&mut app, 8);
        app.update_selection_drag();
        app.finish_selection_drag();
        assert_eq!(app.buffer.text(), "alpha beta gamma");
        assert_eq!(
            app.buffer.text_range(app.cursors.primary().char_range()),
            "alpha beta gamma"
        );
    }

    #[test]
    fn option_drag_outside_a_selection_stays_a_column() {
        let mut app = app("1234\n5678\n9012");
        select(&mut app, 0, 4);
        app.modifiers.alt = true;
        let y = app.tab_bar_height() as f64 + crate::app::LINE_HEIGHT as f64 * 2.0 + 1.0;
        app.mouse.position = dpi::PhysicalPosition::new(10_000.0, y);
        app.begin_mouse_selection(1);
        assert!(app.mouse.selection_drag.is_none());
        assert!(app.mouse.selection_anchor.is_some());
        app.mouse.position =
            dpi::PhysicalPosition::new(10_000.0, app.tab_bar_height() as f64 + 1.0);
        app.update_mouse_selection();
        assert!(app.cursors.count() >= 2);
    }

    #[test]
    fn a_second_click_inside_a_selection_still_selects_the_word() {
        let mut app = app("alpha beta gamma");
        select(&mut app, 0, 5);
        app.mouse.position = point_for_char(&mut app, 2);
        app.begin_mouse_selection(2);
        assert!(app.mouse.selection_drag.is_none());
        assert!(app.mouse.selection_anchor.is_some());
    }
}
