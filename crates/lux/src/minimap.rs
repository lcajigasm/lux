//! Minimap state and interaction for the Lux desktop application.

mod colors;
pub(crate) mod drag;

pub(crate) use colors::SyntaxColors;
use lux_core::buffer::Buffer;
use lux_core::folding::FoldState;
use lux_core::multicursor::MultiCursor;
use lux_ui::minimap::{LineEdit, Minimap, MinimapFrame, MinimapSample};
use std::ops::Range;

pub(crate) const MINIMAP_WIDTH: f32 = 90.0;
pub(crate) const MINIMAP_ROW_HEIGHT: f32 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MinimapState {
    pub(crate) inner: Minimap,
    pub(crate) width: f32,
    pub(crate) edit: Option<LineEdit>,
    pub(crate) model: lux_ui::minimap::MinimapModel,
    /// While the viewport box is held: the pointer's offset below its top.
    pub(crate) drag: Option<f32>,
}

impl Default for MinimapState {
    fn default() -> Self {
        Self {
            inner: Minimap::new(),
            width: MINIMAP_WIDTH,
            edit: None,
            model: lux_ui::minimap::MinimapModel::default(),
            drag: None,
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

    /// Records the text input that took the buffer from `before` to `after`.
    /// Only one cursor typing text without a line break changes a single line;
    /// anything else leaves no edit, so the next frame rebuilds every bar.
    pub(crate) fn note_input(
        &mut self,
        cursors: &MultiCursor,
        text: &str,
        before: u64,
        after: u64,
    ) {
        self.edit = (cursors.count() == 1 && !text.contains('\n')).then(|| LineEdit {
            line: cursors.primary().head.line,
            before,
            after,
        });
    }

    /// The minimap frame for `buffer` in tab `document`, reusing cached bars
    /// while the buffer revision is unchanged and cached strokes while the
    /// syntax colors are unchanged.
    pub(crate) fn frame(
        &mut self,
        document: u64,
        buffer: &Buffer,
        visible: Range<usize>,
        available_rows: usize,
        folds: Option<&FoldState>,
        colors: Option<&dyn lux_ui::minimap::LineColors>,
    ) -> Option<MinimapFrame> {
        let edit = self.edit.take();
        self.model.frame(
            &self.inner,
            MinimapSample {
                revision: buffer.revision(),
                document,
                edit,
                buffer,
                visible,
                available_rows,
                folds,
                colors,
            },
        )
    }

    pub(crate) fn effective_width(&self, scale_factor: f32) -> f32 {
        if self.is_enabled() {
            self.width * scale_factor
        } else {
            0.0
        }
    }
}

impl crate::app::App {
    /// Called after text input moved the buffer away from revision `before`.
    pub(crate) fn note_minimap_input(&mut self, text: &str, before: u64) {
        let after = self.buffer.revision();
        self.ui
            .minimap
            .note_input(&self.cursors, text, before, after);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_core::position::Position;
    use lux_core::selection::Selection;
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

    fn test_app(text: &str) -> crate::app::App {
        let font = include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec();
        let mut app = crate::app::App::new(font);
        app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        app
    }

    fn bars(app: &mut crate::app::App) -> Vec<usize> {
        let document = app.minimap_document();
        let frame = app
            .ui
            .minimap
            .frame(document, &app.buffer, 0..3, 100, None, None);
        frame
            .unwrap()
            .rows
            .iter()
            .map(|row| row.content_columns)
            .collect()
    }

    fn caret(app: &crate::app::App, char_idx: usize) -> Selection {
        Selection::cursor(Position::from_char(&app.buffer, char_idx))
    }

    #[test]
    fn typing_with_several_cursors_updates_every_bar() {
        let mut app = test_app(
            "a
b
",
        );
        assert_eq!(bars(&mut app), vec![1, 1, 0]);
        app.cursors = MultiCursor::from_cursors(vec![caret(&app, 1), caret(&app, 3)]);
        app.insert_at_cursor("x");
        assert_eq!(
            app.buffer.text(),
            "ax
bx
"
        );
        assert_eq!(bars(&mut app), vec![2, 2, 0]);
    }

    #[test]
    fn edits_made_while_the_minimap_is_hidden_show_when_it_returns() {
        let mut app = test_app(
            "a
b
",
        );
        assert_eq!(bars(&mut app), vec![1, 1, 0]);
        app.ui.minimap.toggle();
        app.cursors = MultiCursor::new(caret(&app, 1));
        app.insert_at_cursor("x");
        app.cursors = MultiCursor::new(caret(&app, 4));
        app.insert_at_cursor("yy");
        assert_eq!(
            app.buffer.text(),
            "ax
byy
"
        );
        app.ui.minimap.toggle();
        assert_eq!(bars(&mut app), vec![2, 3, 0]);
    }

    #[test]
    fn replacing_the_buffer_outside_history_refreshes_the_bars() {
        let mut app = test_app(
            "a
b
",
        );
        assert_eq!(bars(&mut app), vec![1, 1, 0]);
        let history = app.history.state_id();
        app.buffer = Buffer::from_bytes(
            b"longer
b
",
        )
        .unwrap();
        assert_eq!(app.history.state_id(), history);
        assert_eq!(bars(&mut app), vec![6, 1, 0]);
    }
}
