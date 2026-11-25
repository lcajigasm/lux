//! Transpose (ST-EDIT-05) through the window keyboard path and the palette.

use super::test_ctx::{TestCtx, app, chr, ctrl, press, primary};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;

fn select(app: &mut crate::app::App, ranges: &[(usize, usize)]) {
    let selections = ranges
        .iter()
        .map(|&(a, h)| {
            Selection::new(
                Position::from_char(&app.buffer, a),
                Position::from_char(&app.buffer, h),
            )
        })
        .collect();
    app.cursors = MultiCursor::from_cursors(selections);
}

#[test]
fn ctrl_t_between_a_and_b_gives_ba() {
    let mut app = app("ab");
    let ctx = TestCtx::default();
    select(&mut app, &[(1, 1)]);
    press(&mut app, &ctx, chr("t"), ctrl());
    assert_eq!(app.buffer.text(), "ba");
    assert_eq!(app.cursors.primary().head.char, 2);
}

#[test]
fn ctrl_t_transposes_at_every_caret_as_one_undo_step() {
    let mut app = app("ab\ncd\nxy");
    let ctx = TestCtx::default();
    select(&mut app, &[(1, 1), (4, 4), (8, 8)]);
    press(&mut app, &ctx, chr("t"), ctrl());
    // The last caret sits at the end of its line: the two chars before swap.
    assert_eq!(app.buffer.text(), "ba\ndc\nyx");
    press(&mut app, &ctx, chr("z"), primary(false));
    assert_eq!(app.buffer.text(), "ab\ncd\nxy");
    assert_eq!(app.cursors.count(), 3);
}

#[test]
fn ctrl_t_rotates_selected_words() {
    let mut app = app("red green blue");
    let ctx = TestCtx::default();
    select(&mut app, &[(0, 3), (4, 9), (10, 14)]);
    press(&mut app, &ctx, chr("t"), ctrl());
    assert_eq!(app.buffer.text(), "blue red green");
}

#[test]
fn ctrl_t_swaps_two_selected_lines() {
    let mut app = app("top\nbottom\n");
    let ctx = TestCtx::default();
    select(&mut app, &[(0, 6)]);
    press(&mut app, &ctx, chr("t"), ctrl());
    assert_eq!(app.buffer.text(), "bottom\ntop\n");
}

#[test]
fn palette_transpose_runs_the_same_command() {
    let mut app = app("xy z");
    let ctx = TestCtx::default();
    select(&mut app, &[(1, 1)]);
    crate::palette::commands::execute_command(&mut app, &ctx, "edit.transpose");
    assert_eq!(app.buffer.text(), "yx z");
}
