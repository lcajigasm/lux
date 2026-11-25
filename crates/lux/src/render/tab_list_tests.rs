//! Geometry check for the `▼` list: what the card paints sits where
//! hit-testing says it is. No pixels or screenshots, only the quads the
//! renderer emits. Glyph rects are left out: tests lay text out with the
//! system UI font, which a CI runner may not have.

use lux_core::buffer::Buffer;
use lux_ui::tabs::TabId;

use crate::app::App;
use crate::document::DocumentEntry;
use crate::render::overlay_list::overlay_colors;
use crate::tab_strip::list;

fn app_with_tabs(count: usize) -> (App, Vec<TabId>) {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let opened = (0..count)
        .map(|index| {
            let id = app.documents.tabs.open(format!("f{index}.txt"));
            let buffer = Buffer::from_bytes(format!("{index}").as_bytes()).unwrap();
            app.documents
                .entries
                .push(DocumentEntry::new(id, None, buffer));
            id
        })
        .collect();
    (app, opened)
}

fn center(rect: [f32; 4]) -> (f32, f32) {
    (rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5)
}

/// Paints the list the way `render_all_modals` does and returns the frame
/// with the emitted quads and glyphs.
fn paint(
    app: &mut App,
) -> (
    crate::overlay_list::ListFrame,
    list::TabListView,
    Vec<lux_render::QuadInstance>,
) {
    let view = list::view(&app.documents, false).expect("list is open");
    let tab = app.tab_bar_height();
    let status = app.status_bar_height();
    let width = crate::modal::modal_viewport_width(app);
    let height = crate::overlay_list::window_height(app.viewport.height, tab, status);
    let scale = app.scale_factor;
    let theme = app.chrome.theme;
    let mut quads = Vec::new();
    let mut glyphs = Vec::new();
    let atlas = &mut app.atlas_state;
    let frame = super::render_tab_list(
        &view,
        width,
        height,
        tab,
        status,
        scale,
        scale,
        app.font,
        &mut app.editor_view,
        &mut atlas.atlas,
        &mut atlas.dirty,
        &mut atlas.scale_cx,
        &theme,
        &mut quads,
        &mut glyphs,
    );
    (frame, view, quads)
}

#[test]
fn painted_list_rows_match_hit_testing() {
    let mut badged = 0;
    for scale in [1.0_f32, 2.0] {
        for selected in [0_usize, 7, 39] {
            let (mut app, opened) = app_with_tabs(40);
            app.scale_factor = scale;
            app.documents.active_tab = Some(opened[selected / 2]);
            let dirty = 33;
            let entry = app
                .documents
                .entries
                .iter_mut()
                .find(|e| e.tab_id == opened[dirty])
                .unwrap();
            entry.saved_version = entry.history.state_id().wrapping_add(1);
            list::open(&mut app);
            app.documents.strip.list.as_mut().unwrap().selected_index = selected;

            let (frame, view, quads) = paint(&mut app);
            let total = view.rows.len();
            assert_eq!(
                frame,
                list::card(&app, selected, total),
                "painted and hit-tested frames differ at scale {scale} selected {selected}"
            );
            assert!(selected >= frame.origin && selected < frame.origin + frame.visible);

            // The selection highlight is the moving-row quad.
            let colors = overlay_colors(&app.chrome.theme);
            let highlight: Vec<_> = quads
                .iter()
                .filter(|q| q.radius == 6.0 * scale && q.color[..3] == colors.selection[..3])
                .collect();
            assert_eq!(highlight.len(), 1, "one selection highlight");
            let (x, y) = center(highlight[0].rect);
            assert_eq!(list::row_at(&frame, total, x, y), Some(selected));

            // Each badge (›, ●, or both) sits on the row that carries it.
            let mut want: Vec<usize> = view
                .rows
                .iter()
                .enumerate()
                .skip(frame.origin)
                .take(frame.visible)
                .filter(|(_, row)| list::badge(row).is_some())
                .map(|(index, _)| index)
                .collect();
            let mut got: Vec<usize> = quads
                .iter()
                .filter(|q| q.radius == 3.0 * scale)
                .filter_map(|q| {
                    let (x, y) = center(q.rect);
                    list::row_at(&frame, total, x, y)
                })
                .collect();
            want.sort_unstable();
            got.sort_unstable();
            assert_eq!(got, want, "badges at scale {scale} selected {selected}");
            badged += got.len();

            // A click at the highlight's center switches to that tab.
            assert!(list::click(&mut app, x, y));
            assert!(app.documents.strip.list.is_none());
            assert_eq!(app.documents.active_tab, Some(view.rows[selected].id));
        }
    }
    assert!(badged >= 4, "the active and dirty badges were on screen");
}
