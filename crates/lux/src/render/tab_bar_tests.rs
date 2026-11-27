//! What the strip paints: tabs cut at the strip's edges, the unsaved dot in
//! place of `×` until the pointer is on it, at scale 1 and 2. Only rects
//! are checked; glyph widths depend on which UI face the runner has.

use lux_core::buffer::Buffer;
use lux_render::{GlyphInstance, QuadInstance};
use lux_ui::tabs::TabId;

use super::render_tab_bar;
use crate::app::{App, TAB_CLOSE_BTN_WIDTH};
use crate::document::DocumentEntry;

fn app(scale: f64, tabs: usize) -> (App, Vec<TabId>) {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.set_display_scale(scale);
    app.viewport.width = (780.0 * scale) as f32;
    let ids = (0..tabs)
        .map(|index| {
            let id = app
                .documents
                .tabs
                .open(format!("a_rather_long_file_name_{index}.rs"));
            let buffer = Buffer::from_bytes(b"x").unwrap();
            app.documents
                .entries
                .push(DocumentEntry::new(id, None, buffer));
            id
        })
        .collect();
    (app, ids)
}

/// Paints the strip from `strip_left` the way `redraw` does.
fn paint(app: &mut App, strip_left: f32) -> (Vec<QuadInstance>, Vec<GlyphInstance>) {
    let width = app.viewport.width + 220.0 * app.scale_factor - strip_left;
    let tab_width = app.tab_width();
    let (mut quads, mut glyphs) = (Vec::new(), Vec::new());
    render_tab_bar(
        &app.documents,
        false,
        strip_left,
        width,
        tab_width,
        app.tab_bar_height(),
        app.scale_factor,
        app.scale_factor,
        app.font,
        &mut app.editor_view,
        &mut app.atlas_state.atlas,
        &mut app.atlas_state.dirty,
        &mut app.atlas_state.scale_cx,
        &app.chrome.theme,
        &mut quads,
        &mut glyphs,
    );
    (quads, glyphs)
}

#[test]
fn a_scrolled_strip_paints_nothing_left_of_its_span() {
    for scale in [1.0, 2.0] {
        let (mut app, _) = app(scale, 30);
        let left = 220.0 * app.scale_factor;
        let tab_width = app.tab_width();
        // Half a tab scrolled off the left end.
        app.documents.tabs.set_scroll(tab_width * 2.5);
        let (quads, glyphs) = paint(&mut app, left);
        for q in &quads {
            assert!(q.rect[0] >= left - 0.01, "quad at {:?}", q.rect);
        }
        for g in &glyphs {
            assert!(g.rect[0] >= left - 0.01, "glyph at {:?}", g.rect);
        }
        // And the cut tab still paints its visible half.
        assert!(quads.iter().any(|q| q.rect[0] == left && q.rect[2] > 0.0));
    }
}

#[test]
fn tabs_shrink_to_100_points_before_scrolling() {
    for scale in [1.0, 2.0] {
        let (app, _) = app(scale, 30);
        assert_eq!(app.tab_width(), 100.0 * app.scale_factor);
        let (few, _) = self::app(scale, 2);
        assert!(few.tab_width() > 100.0 * few.scale_factor);
    }
}

fn dots(quads: &[QuadInstance], scale: f32) -> Vec<[f32; 4]> {
    let size = 8.0 * scale;
    quads
        .iter()
        .filter(|q| q.radius > 0.0 && (q.rect[2] - size).abs() < 0.01)
        .map(|q| q.rect)
        .collect()
}

#[test]
fn a_dirty_tab_shows_a_dot_until_its_close_mark_is_hovered() {
    for scale in [1.0, 2.0] {
        let (mut app, ids) = app(scale, 3);
        let dirty = ids[1];
        let entry = app
            .documents
            .entries
            .iter_mut()
            .find(|e| e.tab_id == dirty)
            .unwrap();
        entry.saved_version = entry.history.state_id() + 1;
        let left = 0.0;
        let (quads, _) = paint(&mut app, left);
        let found = dots(&quads, app.scale_factor);
        assert_eq!(found.len(), 1, "one dirty tab, one dot");
        // The dot sits where the close mark is: the end of the tab, as
        // hit-testing sees it.
        let s = app.scale_factor;
        let index = app
            .documents
            .tabs
            .tabs()
            .iter()
            .position(|t| t.id == dirty)
            .unwrap();
        let tab_width = app.tab_width();
        let close_center = left + (index + 1) as f32 * tab_width - TAB_CLOSE_BTN_WIDTH * s * 0.5;
        let dot = found[0];
        assert!((dot[0] + dot[2] * 0.5 - close_center).abs() < 0.01);
        let alpha = quads.iter().find(|q| q.rect == dot).unwrap().color[3];
        let ink = crate::render::paint::color(app.chrome.theme.foreground)[3];
        assert!((alpha - 0.7 * ink).abs() < 0.001);

        app.documents.strip.hover = Some((dirty, false));
        assert_eq!(
            dots(&paint(&mut app, left).0, s).len(),
            1,
            "body hover keeps it"
        );
        app.documents.strip.hover = Some((dirty, true));
        assert!(dots(&paint(&mut app, left).0, s).is_empty(), "× on hover");
    }
}

#[test]
fn hover_follows_pointer_moves_and_clears_on_leave() {
    let (mut app, ids) = app(1.0, 3);
    let tab_width = app.tab_width();
    let close = TAB_CLOSE_BTN_WIDTH * app.scale_factor;
    let first = app.documents.tabs.tabs()[0].id;
    app.mouse.position = dpi::PhysicalPosition::new(f64::from(tab_width - close * 0.5), 10.0);
    assert!(crate::tab_strip::pointer_moved(&mut app));
    assert_eq!(app.documents.strip.hover, Some((first, true)));
    assert!(
        !crate::tab_strip::pointer_moved(&mut app),
        "no change, no redraw"
    );
    app.mouse.position.x = f64::from(tab_width + 5.0);
    assert!(crate::tab_strip::pointer_moved(&mut app));
    let second = app.documents.tabs.tabs()[1].id;
    assert_eq!(app.documents.strip.hover, Some((second, false)));
    app.mouse.position.y = 300.0;
    assert!(crate::tab_strip::pointer_moved(&mut app));
    assert_eq!(app.documents.strip.hover, None);
    app.mouse.position.y = 10.0;
    crate::tab_strip::pointer_moved(&mut app);
    crate::pointer::leave(&mut app);
    assert_eq!(app.documents.strip.hover, None);
    assert!(ids.len() == 3);
}
