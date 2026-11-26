use super::*;

fn sizes(scale: f32, sidebar: f32, search_bar: f32, dock: f32) -> ChromeSizes {
    ChromeSizes {
        window_width: 1000.0 * scale,
        window_height: 700.0 * scale,
        tab_bar: 32.0 * scale,
        status_bar: 24.0 * scale,
        search_bar: search_bar * scale,
        dock: dock * scale,
        sidebar: sidebar * scale,
        minimap: 90.0 * scale,
    }
}

fn bottom(rect: [f32; 4]) -> f32 {
    rect[1] + rect[3]
}

#[test]
fn an_open_dock_ends_the_editor_at_its_top_and_spans_the_editor_column() {
    for scale in [1.0, 2.0] {
        for sidebar in [0.0, 220.0] {
            let closed = compute(&sizes(scale, sidebar, 0.0, 0.0));
            let open = compute(&sizes(scale, sidebar, 0.0, 200.0));
            let dock = open.dock.expect("dock");
            assert_eq!(open.editor[3], closed.editor[3] - 200.0 * scale);
            assert_eq!(bottom(open.editor), dock[1], "editor ends at the dock");
            assert_eq!(dock[0], sidebar * scale, "dock starts right of the sidebar");
            assert_eq!(
                dock[0] + dock[2],
                1000.0 * scale,
                "and reaches the window edge"
            );
            assert_eq!(bottom(dock), (700.0 - 24.0) * scale, "on the status bar");
            assert_eq!(open.editor[2], closed.editor[2], "width is unchanged");
        }
    }
}

#[test]
fn find_bar_and_dock_stack_between_editor_and_status_bar() {
    let layout = compute(&sizes(2.0, 220.0, 64.0, 200.0));
    let bar = layout.search_bar.expect("bar");
    let dock = layout.dock.expect("dock");
    assert_eq!(bottom(layout.editor), bar[1]);
    assert_eq!(bottom(bar), dock[1]);
    assert_eq!(bottom(dock), (700.0 - 24.0) * 2.0);
    assert_eq!(bar[0], layout.column[0]);
    assert_eq!(bar[2], layout.column[2]);
    assert_eq!(layout.column[2], layout.editor[2] + 90.0 * 2.0);
}

fn app() -> App {
    App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

#[test]
fn opening_the_dock_shrinks_the_app_viewport() {
    let mut app = app();
    let before = app.fit_viewport(1000.0, 700.0);
    app.ui.term.visible = true;
    let after = app.fit_viewport(1000.0, 700.0);
    let dock = after.dock.expect("dock");
    assert_eq!(
        app.viewport.height,
        before.editor[3] - app.term_dock_height()
    );
    assert_eq!(
        app.frame_layout(),
        after,
        "hit tests read the painted layout"
    );
    assert_eq!(bottom(after.editor), dock[1]);
}

#[test]
fn the_snapshot_frame_has_the_presented_layout_at_its_scale() {
    let mut app = app();
    crate::search::open_find(&mut app, true);
    app.ui.term.visible = true;
    let presented = app.fit_viewport(1000.0, 700.0);
    let presented_viewport = app.viewport;

    // What `render_capture` + `redraw` do for a 2x snapshot of this window.
    let (width, height) = crate::snapshot::capture_size(1000, 700, app.scale_factor, 2.0);
    app.set_display_scale(2.0);
    let captured = app.fit_viewport(width as f32, height as f32);

    let doubled = |r: [f32; 4]| r.map(|v| v * 2.0);
    assert_eq!(captured.editor, doubled(presented.editor));
    assert_eq!(captured.search_bar, presented.search_bar.map(doubled));
    assert_eq!(captured.dock, presented.dock.map(doubled));
    assert_eq!(app.viewport.width, presented_viewport.width * 2.0);
    assert_eq!(app.viewport.height, presented_viewport.height * 2.0);
}

fn caret_on_line(app: &mut App, line: usize) {
    let text = "line\n".repeat(200);
    app.buffer = lux_core::buffer::Buffer::from_bytes(text.as_bytes()).expect("buffer");
    // Every line is "line\n": 5 chars.
    let at = lux_core::position::Position::from_char(&app.buffer, line * 5);
    let caret = lux_core::selection::Selection::cursor(at);
    app.cursors = lux_core::multicursor::MultiCursor::new(caret);
}

fn caret_visible(app: &App) -> bool {
    let lh = app.editor_view.metrics().line_height;
    let y = app.cursors.primary().head.line as f32 * lh;
    y >= app.viewport.scroll_y && y + lh <= app.viewport.scroll_y + app.viewport.height
}

#[test]
fn a_dock_opened_without_a_reveal_keeps_the_caret_on_screen() {
    let mut app = app();
    app.fit_viewport(1000.0, 700.0);
    let lh = app.editor_view.metrics().line_height;
    // The caret on the last fully visible line of the editor.
    let line = (app.viewport.height / lh) as usize - 1;
    caret_on_line(&mut app, line);
    assert!(caret_visible(&app));
    // A build start opens the dock; the next frame sizes the viewport.
    app.ui.term.visible = true;
    app.fit_viewport(1000.0, 700.0);
    assert!(caret_visible(&app), "the caret is not under the dock");
    let scrolled = app.viewport.scroll_y;
    assert!(scrolled > 0.0);
    // Later frames with the same layout do not scroll again.
    app.fit_viewport(1000.0, 700.0);
    assert_eq!(app.viewport.scroll_y, scrolled);
}

#[test]
fn a_shrink_does_not_pull_back_a_caret_scrolled_out_of_view() {
    let mut app = app();
    app.fit_viewport(1000.0, 700.0);
    caret_on_line(&mut app, 150);
    assert!(!caret_visible(&app));
    app.ui.term.visible = true;
    app.fit_viewport(1000.0, 700.0);
    assert_eq!(app.viewport.scroll_y, 0.0, "the reading position stays");
}
