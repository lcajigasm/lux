//! Up/Down and Home/End under soft word wrap move by the rows drawn on
//! screen. Each check reads the caret back from a built frame, so the
//! motion and the renderer have to agree on where rows break.

use std::time::Duration;

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::NamedKey;

use super::super::tests::{app, named};
use crate::app::App;

fn paragraph(words: usize) -> String {
    (0..words)
        .map(|i| format!("w{:03}", i % 1000))
        .collect::<Vec<_>>()
        .join(" ")
}

fn wrapped(text: &str, width: f32) -> App {
    let mut app = app(text);
    app.editor_view.metrics_mut().word_wrap = true;
    app.viewport.width = width;
    app.viewport.height = 4000.0;
    app
}

/// The primary caret as drawn: (buffer line, soft-wrap row, x).
fn caret(app: &mut App) -> (usize, usize, f32) {
    let viewport = app.viewport;
    let frame = app.editor_view.build_frame(
        &app.buffer,
        &app.cursors,
        &viewport,
        Duration::ZERO,
        Some(&app.fold_state),
    );
    let caret = frame.cursors.iter().find(|c| c.primary).unwrap();
    let top = frame
        .lines
        .iter()
        .find(|l| l.index == caret.line)
        .unwrap()
        .top;
    let row = ((caret.y - top) / app.editor_view.metrics().line_height).round() as usize;
    (caret.line, row, caret.x)
}

fn rows_of(app: &mut App, line: usize) -> usize {
    let width = app.viewport.width;
    app.editor_view
        .wrap_row_starts(&app.buffer, line, width)
        .len()
}

fn place(app: &mut App, line: usize, column: usize) {
    let pos = lux_core::movement::position_at_line_column(&app.buffer, line, column);
    app.cursors = MultiCursor::new(Selection::cursor(pos));
    app.reset_goal_columns();
}

#[test]
fn down_steps_through_every_wrapped_row_then_into_the_next_line() {
    let text = format!("{}\nnext line\n", paragraph(500));
    let mut app = wrapped(&text, 4000.0);
    let rows = rows_of(&mut app, 0);
    assert!(rows >= 4, "the paragraph wraps into {rows} rows");
    place(&mut app, 0, 3);
    let (_, first_row, x) = caret(&mut app);
    assert_eq!(first_row, 0);

    for row in 1..rows {
        named(&mut app, NamedKey::ArrowDown);
        let (line, visual_row, caret_x) = caret(&mut app);
        assert_eq!((line, visual_row), (0, row), "press {row}");
        assert!((caret_x - x).abs() < 0.01, "row {row}: x {caret_x} != {x}");
    }
    named(&mut app, NamedKey::ArrowDown);
    let head = app.cursors.primary().head;
    assert_eq!((head.line, head.column), (1, 3));

    for row in (0..rows).rev() {
        named(&mut app, NamedKey::ArrowUp);
        let (line, visual_row, _) = caret(&mut app);
        assert_eq!((line, visual_row), (0, row));
    }
    assert_eq!(
        app.cursors.primary().head,
        Position::from_char(&app.buffer, 3)
    );
}

#[test]
fn the_goal_x_survives_a_short_wrapped_row() {
    // Cut a paragraph a few columns into its third row, so line 0 ends
    // in a row shorter than the goal column whatever the font metrics.
    let long = paragraph(500);
    let mut probe_app = wrapped(&long, 4000.0);
    let probe = probe_app
        .editor_view
        .wrap_row_starts(&probe_app.buffer, 0, 4000.0);
    assert!(
        probe.len() >= 3,
        "the paragraph wraps into {} rows",
        probe.len()
    );
    let goal = 100;
    assert!(probe[1] > goal, "a row holds more than {goal} columns");
    let short = &long[..probe[2] + 12];
    let text = format!("{short}\n{long}");
    let mut app = wrapped(&text, 4000.0);
    assert_eq!(
        app.editor_view.wrap_row_starts(&app.buffer, 0, 4000.0),
        probe[..3]
    );

    place(&mut app, 0, goal);
    let (_, _, x) = caret(&mut app);
    named(&mut app, NamedKey::ArrowDown);
    let (line, row, middle_x) = caret(&mut app);
    assert_eq!((line, row), (0, 1));
    assert!((middle_x - x).abs() < 0.01, "goal x {x} lost: {middle_x}");
    named(&mut app, NamedKey::ArrowDown);
    let (line, row, short_x) = caret(&mut app);
    assert_eq!((line, row), (0, 2));
    assert!(short_x < x, "the short row clamps the caret");
    assert_eq!(app.cursors.primary().head.column, short.len());
    named(&mut app, NamedKey::ArrowDown);
    let (line, row, back_x) = caret(&mut app);
    assert_eq!((line, row), (1, 0));
    assert!((back_x - x).abs() < 0.01, "goal x {x} lost: {back_x}");
    assert_eq!(app.cursors.primary().head.column, goal);
    named(&mut app, NamedKey::ArrowUp);
    assert_eq!(caret(&mut app).1, 2);
    named(&mut app, NamedKey::ArrowUp);
    let (line, row, up_x) = caret(&mut app);
    assert_eq!((line, row), (0, 1));
    assert!((up_x - x).abs() < 0.01, "goal x {x} lost going up: {up_x}");
}

#[test]
fn shift_down_extends_the_selection_by_one_row() {
    let text = paragraph(500);
    let mut app = wrapped(&text, 4000.0);
    place(&mut app, 0, 5);
    app.modifiers.shift = true;
    named(&mut app, NamedKey::ArrowDown);
    let selection = app.cursors.primary();
    assert_eq!(selection.anchor.column, 5);
    assert_eq!(selection.head.line, 0);
    assert_eq!(caret(&mut app).1, 1);
}

#[test]
fn home_and_end_stop_at_the_wrapped_row_edges_first() {
    let text = format!("    {}", paragraph(500));
    let mut app = wrapped(&text, 4000.0);
    let starts = app.editor_view.wrap_row_starts(&app.buffer, 0, 4000.0);
    let row_one = starts[1];
    place(&mut app, 0, row_one + 6);

    named(&mut app, NamedKey::Home);
    assert_eq!(app.cursors.primary().head.column, row_one);
    assert_eq!(caret(&mut app).1, 1);
    named(&mut app, NamedKey::Home);
    assert_eq!(
        app.cursors.primary().head.column,
        4,
        "then the smart line start"
    );

    place(&mut app, 0, row_one + 6);
    named(&mut app, NamedKey::End);
    let (_, row, _) = caret(&mut app);
    assert_eq!(row, 1, "End stays on the row");
    assert_eq!(app.cursors.primary().head.column, starts[2] - 1);
    named(&mut app, NamedKey::End);
    let end = app.cursors.primary().head;
    assert_eq!(end.column, text.len(), "then the line end");
}

#[test]
fn without_wrap_down_still_moves_by_buffer_line() {
    let text = format!("{}\nnext\n", paragraph(500));
    let mut app = app(&text);
    app.viewport.width = 4000.0;
    place(&mut app, 0, 3);
    named(&mut app, NamedKey::ArrowDown);
    let head = app.cursors.primary().head;
    assert_eq!((head.line, head.column), (1, 3));
}
