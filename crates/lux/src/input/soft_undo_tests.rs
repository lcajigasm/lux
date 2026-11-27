//! Soft Undo / Soft Redo (ST-EDIT-01) through the window keyboard path.

use super::test_ctx::{TestCtx, app, chr, press, primary};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::{Key, Modifiers, NamedKey};

use crate::app::App;

fn type_text(app: &mut App, ctx: &TestCtx, text: &str) {
    for ch in text.chars() {
        let key = if ch == ' ' {
            Key::Named(NamedKey::Space)
        } else {
            chr(&ch.to_string())
        };
        press(app, ctx, key, Modifiers::default());
    }
}

fn named(app: &mut App, ctx: &TestCtx, key: NamedKey) {
    press(app, ctx, Key::Named(key), Modifiers::default());
}

fn soft_undo(app: &mut App, ctx: &TestCtx) {
    press(app, ctx, chr("u"), primary(false));
}

fn soft_redo(app: &mut App, ctx: &TestCtx) {
    press(app, ctx, chr("u"), primary(true));
}

fn selected(app: &App) -> Vec<String> {
    app.cursors
        .cursors()
        .iter()
        .map(|s| app.buffer.text_range(s.char_range()))
        .collect()
}

#[test]
fn cmd_u_after_three_cmd_d_drops_back_to_two_words_without_touching_text() {
    let mut app = app("");
    let ctx = TestCtx::default();
    type_text(&mut app, &ctx, "foo bar foo baz foo");
    let typed = app.buffer.text();
    assert_eq!(typed, "foo bar foo baz foo");
    // Put the caret inside the first "foo".
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&app.buffer, 1)));

    for _ in 0..3 {
        press(&mut app, &ctx, chr("d"), primary(false));
    }
    assert_eq!(selected(&app), ["foo", "foo", "foo"]);

    soft_undo(&mut app, &ctx);
    assert_eq!(selected(&app), ["foo", "foo"]);
    assert_eq!(app.buffer.text(), typed);

    soft_undo(&mut app, &ctx);
    assert_eq!(selected(&app), ["foo"]);
    soft_redo(&mut app, &ctx);
    soft_redo(&mut app, &ctx);
    assert_eq!(selected(&app), ["foo", "foo", "foo"]);
    assert_eq!(app.buffer.text(), typed);
    // Soft undo left the text undo history alone: Cmd+Z still undoes typing.
    press(&mut app, &ctx, chr("z"), primary(false));
    assert!(app.buffer.text().len() < typed.len());
}

#[test]
fn cmd_u_after_goto_line_500_restores_caret_and_view() {
    let text: String = (1..=1000).map(|i| format!("line {i}\n")).collect();
    let mut app = app(&text);
    let ctx = TestCtx::default();
    app.viewport.height = 400.0;
    app.viewport.width = 600.0;
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&app.buffer, 2)));
    app.viewport.scroll_y = 0.0;

    press(&mut app, &ctx, chr("p"), primary(false));
    assert!(app.ui.goto_anything.is_some());
    type_text(&mut app, &ctx, ":500");
    named(&mut app, &ctx, NamedKey::Enter);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.cursors.primary().head.line, 499);
    assert!(app.viewport.scroll_y > 0.0);

    soft_undo(&mut app, &ctx);
    assert_eq!(app.cursors.count(), 1);
    assert_eq!(app.cursors.primary().head.char, 2);
    assert_eq!(app.viewport.scroll_y, 0.0);

    soft_redo(&mut app, &ctx);
    assert_eq!(app.cursors.primary().head.line, 499);
}

#[test]
fn each_find_enter_is_its_own_step() {
    let mut app = app("x1 x2 x3 x4\n");
    let ctx = TestCtx::default();
    press(&mut app, &ctx, chr("f"), primary(false));
    type_text(&mut app, &ctx, "x");
    named(&mut app, &ctx, NamedKey::Enter);
    named(&mut app, &ctx, NamedKey::Enter);
    let at = app.cursors.primary().char_range();
    named(&mut app, &ctx, NamedKey::Enter);
    let last = app.cursors.primary().char_range();
    assert_ne!(at, last);
    named(&mut app, &ctx, NamedKey::Escape);

    soft_undo(&mut app, &ctx);
    assert_eq!(app.cursors.primary().char_range(), at);
}

#[test]
fn remembered_selections_follow_later_edits() {
    let mut app = app("head\nkeep this word\n");
    let ctx = TestCtx::default();
    // Select "this", then move to the start of the document and type.
    app.cursors = MultiCursor::new(Selection::new(
        Position::from_char(&app.buffer, 10),
        Position::from_char(&app.buffer, 14),
    ));
    press(
        &mut app,
        &ctx,
        Key::Named(NamedKey::ArrowUp),
        primary(false),
    );
    assert_eq!(app.cursors.primary().head.char, 0);
    type_text(&mut app, &ctx, ">> ");
    assert_eq!(app.buffer.text(), ">> head\nkeep this word\n");

    soft_undo(&mut app, &ctx);
    assert_eq!(selected(&app), ["this"]);
    assert_eq!(app.buffer.text(), ">> head\nkeep this word\n");
}

#[test]
fn a_mouse_click_is_a_step() {
    let mut app = app("alpha beta gamma");
    let ctx = TestCtx::default();
    app.cursors = MultiCursor::new(Selection::new(
        Position::from_char(&app.buffer, 6),
        Position::from_char(&app.buffer, 10),
    ));
    // Settle the selection, then click elsewhere on the line.
    press(
        &mut app,
        &ctx,
        Key::Named(NamedKey::Shift),
        Modifiers::default(),
    );
    let y = app.tab_bar_height() as f64 + 2.0;
    // Inside "gamma", clear of the "beta" selection.
    let x = app.text_left() + app.editor_view.reference_width("alpha beta gam");
    app.mouse.position = dpi::PhysicalPosition::new(f64::from(x), y);
    app.mouse.placed = true;
    crate::edit_menu::press_editor(&mut app, &ctx, lux_platform::MouseButton::Left);
    app.mouse.selection_anchor = None;
    assert!(app.cursors.primary().is_empty());

    soft_undo(&mut app, &ctx);
    assert_eq!(selected(&app), ["beta"]);
}

#[test]
fn palette_soft_undo_and_redo_run_the_same_commands() {
    let mut app = app("a b a");
    let ctx = TestCtx::default();
    press(&mut app, &ctx, chr("d"), primary(false));
    press(&mut app, &ctx, chr("d"), primary(false));
    assert_eq!(app.cursors.count(), 2);
    crate::palette::commands::execute_command(&mut app, &ctx, "edit.soft_undo");
    assert_eq!(app.cursors.count(), 1);
    crate::palette::commands::execute_command(&mut app, &ctx, "edit.soft_redo");
    assert_eq!(app.cursors.count(), 2);
}
