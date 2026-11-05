//! The painted caret has to sit on the buffer position. A tab used to
//! shape as one `.notdef` cell while `LineLayout::width` dropped that
//! trailing whitespace, so the blink landed at x = 0.

use std::time::Duration;

use lux_core::buffer::{Indent, IndentStyle};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::NamedKey;
use lux_ui::Viewport;

use super::tests::{app, command, named};
use crate::app::App;

fn caret_x(app: &mut App) -> f32 {
    let frame = app.editor_view.build_frame(
        &app.buffer,
        &app.cursors,
        &Viewport {
            width: 800.0,
            height: 400.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        },
        Duration::ZERO,
        None,
    );
    let caret = frame.cursors.iter().find(|cursor| cursor.primary).unwrap();
    let end = frame.lines[caret.line]
        .layout
        .clusters()
        .last()
        .map(|cluster| cluster.x + cluster.advance)
        .unwrap_or(0.0);
    assert!(
        (caret.x - end).abs() < 0.01,
        "caret {} != shaped end {end}",
        caret.x
    );
    caret.x
}

fn at_end(app: &mut App) {
    app.cursors = MultiCursor::new(Selection::cursor(Position::document_end(&app.buffer)));
}

#[test]
fn tab_shift_tab_and_bracket_indent_paint_the_caret_on_the_buffer() {
    let mut tabs = app("");
    tabs.buffer.set_indent(Indent {
        style: IndentStyle::Tabs,
        width: 1,
    });
    named(&mut tabs, NamedKey::Tab);
    assert_eq!(tabs.buffer.text(), "\t");
    let after_tab = caret_x(&mut tabs);
    assert!(
        after_tab > 1.0,
        "caret snapped back to the origin after Tab, x={after_tab}"
    );

    tabs.modifiers.shift = true;
    named(&mut tabs, NamedKey::Tab);
    tabs.modifiers.shift = false;
    assert_eq!(tabs.buffer.text(), "");
    assert!(caret_x(&mut tabs).abs() < 0.01);

    let mut indented = app("a");
    indented.buffer.set_indent(Indent {
        style: IndentStyle::Tabs,
        width: 1,
    });
    at_end(&mut indented);
    command(&mut indented, "]", false);
    assert_eq!(indented.buffer.text(), "\ta");
    let after_indent = caret_x(&mut indented);
    assert!(after_indent > after_tab);

    command(&mut indented, "[", false);
    assert_eq!(indented.buffer.text(), "a");
    let after_outdent = caret_x(&mut indented);
    assert!(after_outdent < after_indent);
    assert!(after_outdent > 0.0);

    let mut spaces = app("a   ");
    at_end(&mut spaces);
    let trailing = caret_x(&mut spaces);
    assert!(
        trailing > after_outdent * 3.0,
        "trailing spaces pulled the caret back, x={trailing}"
    );
}
