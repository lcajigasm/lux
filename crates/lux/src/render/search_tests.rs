use super::*;
use crate::app::App;
use lux_core::find_replace::Query;
use lux_core::multicursor::MultiCursor;
use std::time::Duration;

fn app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

/// The match boxes of a find for `needle` equal the selection boxes of
/// the same ranges selected with multiple cursors, in the same view.
fn assert_matches_land_on_selections(text: &str, needle: &str, viewport: Viewport) {
    let mut app = app(text);
    let mut search = SearchBarState::new(false);
    search
        .search_bar
        .set_query(&app.buffer, Query::new(needle))
        .unwrap();
    let matches = search.search_bar.matches().to_vec();
    assert!(matches.len() >= 2, "fixture has several matches");
    let cursors = MultiCursor::from_cursors(matches);
    let frame = app
        .editor_view
        .build_frame(&app.buffer, &cursors, &viewport, Duration::ZERO, None);
    let origin = TextOrigin {
        viewport: &viewport,
        editor_top: 64.0,
        sidebar_width: 440.0,
        text_left: 512.0,
        line_height: app.editor_view.metrics().line_height,
    };
    let selections: Vec<[f32; 4]> = frame
        .selections
        .iter()
        .map(|rect| selection_quad(rect, &viewport, 64.0, 512.0, origin.line_height))
        .collect();
    let found: Vec<[f32; 4]> = match_quads(&search, &app.buffer, &frame, &origin)
        .into_iter()
        .map(|(_, quad)| quad)
        .collect();
    assert!(!found.is_empty(), "some matches are on screen");
    assert_eq!(found, selections);
}

#[test]
fn match_boxes_equal_selection_boxes_after_tabs() {
    let text = "fn f() {\n\tlet counts = 1;\n\t\tcounts += 1;\n    *counts.entry(w) += 1;\n}\n";
    let viewport = Viewport {
        width: 1380.0,
        height: 600.0,
        scroll_x: 0.0,
        scroll_y: 0.0,
    };
    assert_matches_land_on_selections(text, "counts", viewport);
}

#[test]
fn match_boxes_equal_selection_boxes_in_a_scrolled_view() {
    let text: String = (0..120)
        .map(|i| format!("{}\tcounts[{i}] = counts.len();\n", " ".repeat(i % 7)))
        .collect();
    let viewport = Viewport {
        width: 600.0,
        height: 300.0,
        scroll_x: 37.0,
        scroll_y: 913.0,
    };
    assert_matches_land_on_selections(&text, "counts", viewport);
}

#[test]
fn the_match_fill_is_the_theme_accent_at_thirty_percent() {
    let theme = lux_ui::Theme::MONOKAI;
    let fill = match_fill(&theme);
    let accent = color(theme.accent);
    assert_eq!(&fill[..3], &accent[..3]);
    assert!((fill[3] - 0.30).abs() < f32::EPSILON);
}

#[test]
fn find_bar_labels_clear_wcag_aa_on_every_bundled_theme() {
    use super::super::panel::contrast_ratio;
    for scheme in crate::color_scheme::bundled_schemes() {
        let theme = &scheme.theme;
        let page = color(theme.chrome.panel);
        let control = distinct_fill(color(theme.chrome.panel_input), page);
        let accent = color(theme.accent);
        let pairs = [
            (label_ink(theme, control), control, "button/control"),
            (hint_ink(theme, control), control, "counter/field"),
            (label_ink(theme, accent), accent, "active toggle"),
            (label_ink(theme, page), page, "close/bar"),
        ];
        for (ink, fill, name) in pairs {
            let ratio = contrast_ratio(ink, fill);
            assert!(ratio >= 4.5, "{} {name} {ratio:.2}", scheme.name);
        }
    }
}
