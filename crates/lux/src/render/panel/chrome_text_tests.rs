use super::*;
use crate::render::panel::contrast_ratio;

#[test]
fn every_bundled_theme_paints_chrome_text_at_wcag_aa() {
    let schemes = crate::color_scheme::bundled_schemes();
    assert!(schemes.len() >= 14, "found {} themes", schemes.len());
    for scheme in schemes.iter() {
        let theme = &scheme.theme;
        let sidebar = color(theme.chrome.sidebar);
        let selected = sidebar_selection(theme);
        let status = color(theme.chrome.status_bar);
        let fg = color(theme.foreground);
        let mut checks = vec![
            ("label/sidebar", sidebar_label(fg, sidebar), sidebar, 4.5),
            (
                "label/selection",
                sidebar_label(fg, selected),
                selected,
                4.5,
            ),
            ("header/sidebar", sidebar_header(theme), sidebar, 4.5),
            ("status/bar", status_ink(theme), status, 4.5),
            ("outline/sidebar", outline_ink(theme, sidebar), sidebar, 3.0),
            (
                "outline/selection",
                outline_ink(theme, selected),
                selected,
                3.0,
            ),
        ];
        for under in [sidebar, selected] {
            let ink = chevron_ink(sidebar_label(fg, under), under);
            assert!(ink[3] >= CHEVRON_ALPHA, "{}", scheme.name);
            let chevron = over(ink, under);
            checks.push(("chevron", chevron, under, 3.0));
        }
        for mark in [
            crate::git::marks::GitMark::Modified,
            crate::git::marks::GitMark::Untracked,
            crate::git::marks::GitMark::Added,
            crate::git::marks::GitMark::Deleted,
            crate::git::marks::GitMark::Conflicted,
        ] {
            let ink = color(crate::git::colors::mark_color(mark, theme));
            checks.push(("git/sidebar", sidebar_label(ink, sidebar), sidebar, 4.5));
            checks.push(("git/selection", sidebar_label(ink, selected), selected, 4.5));
        }
        for (name, ink, background, floor) in checks {
            let ratio = contrast_ratio(ink, background);
            assert!(ratio >= floor, "{} {name} {ratio:.2}", scheme.name);
        }
    }
}

#[test]
fn the_selected_row_uses_the_theme_selection_and_stands_off_the_sidebar() {
    for scheme in crate::color_scheme::bundled_schemes() {
        let theme = &scheme.theme;
        let painted = color(lux_ui::theme::composite(
            theme.selection,
            theme.chrome.sidebar,
        ));
        let fill = sidebar_selection(theme);
        let sidebar = color(theme.chrome.sidebar);
        assert_ne!(fill, sidebar, "{}", scheme.name);
        if crate::render::panel::distinct_fill(painted, sidebar) == painted {
            assert_eq!(fill, painted, "{} keeps its selection color", scheme.name);
        }
    }
}

#[test]
fn chrome_sizes_follow_the_review_ramp() {
    assert_eq!(SIDEBAR_PT, 13.0);
    assert_eq!(STATUS_PT, 12.0);
    assert_eq!(HEADER_PT, 11.0);
    assert_eq!(PANEL_TAB_PT, 12.0);
    assert!((HEADER_TRACKING_EM - 0.06).abs() < f32::EPSILON);
    assert_eq!(crate::sidebar::SIDEBAR_ROW_HEIGHT, 22.0);
    assert_eq!(crate::status_bar::STATUS_BAR_HEIGHT, 22.0);
}

/// An app on the Ahem test face (every glyph one em wide) with the system
/// UI face installed, or `None` on a machine without one.
fn app_with_ui_face() -> Option<crate::app::App> {
    let mut app = crate::app::App::new(
        include_bytes!("../../../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
    );
    crate::font::load_ui_font();
    crate::font::install_ui_font(app.editor_view.text_layout_mut());
    let (_, family) = crate::font::ui_font(app.font, app.editor_view.family());
    (family != app.editor_view.family()).then_some(app)
}

fn ui_width(app: &mut crate::app::App, size: f32, text: &str) -> f32 {
    let mut glyphs = Vec::new();
    let mut pen = ChromePen::new(
        &mut app.editor_view,
        &mut app.atlas_state.atlas,
        &mut app.atlas_state.dirty,
        &mut app.atlas_state.scale_cx,
        app.font,
        &mut glyphs,
    );
    pen.width(size, 0.0, text)
}

#[test]
fn the_status_bar_is_set_in_the_ui_face_at_12_pt() {
    let Some(mut app) = app_with_ui_face() else {
        return;
    };
    let buffer = lux_core::buffer::Buffer::from_bytes(b"hello\n").unwrap();
    let cursor =
        lux_core::selection::Selection::cursor(lux_core::position::Position::document_start());
    let (mut quads, mut glyphs) = (Vec::new(), Vec::new());
    let layout = crate::render::status_bar::render_status_bar(
        1000.0,
        700.0,
        1.0,
        1.0,
        app.font,
        &mut app.editor_view,
        &mut app.atlas_state.atlas,
        &mut app.atlas_state.dirty,
        &mut app.atlas_state.scale_cx,
        &lux_ui::Theme::MONOKAI,
        &buffer,
        cursor,
        None,
        false,
        None,
        &mut quads,
        &mut glyphs,
    );
    assert_eq!(layout.bar_rect.height, 22.0);
    let label = crate::status_bar::build_status_bar(&buffer, cursor.head, None, 0, None)
        .cursor
        .label;
    let drawn = layout.cursor_rect.width - 8.0;
    let ui = ui_width(&mut app, STATUS_PT, &label);
    assert!((drawn - ui).abs() < 0.5, "drawn {drawn} vs UI face {ui}");
    let ahem = label.chars().count() as f32 * STATUS_PT;
    assert!(drawn < ahem * 0.9, "not the editor face: {drawn} vs {ahem}");
}

#[test]
fn the_panel_header_is_flat_with_one_accent_underline_under_the_active_tab() {
    let Some(mut app) = app_with_ui_face() else {
        return;
    };
    let theme = lux_ui::Theme::MONOKAI;
    crate::render::paint::set_active_theme(theme);
    let mut term = crate::term::TerminalState::new();
    term.visible = true;
    term.show_tasks = true;
    let dock = [0.0, 400.0, 1000.0, 260.0];
    let (mut quads, mut glyphs) = (Vec::new(), Vec::new());
    let build = crate::build::BuildState::default();
    crate::render::term::render_term(
        &mut term,
        &build,
        &mut quads,
        &mut glyphs,
        &mut app.editor_view,
        &mut app.atlas_state.atlas,
        &mut app.atlas_state.dirty,
        &mut app.atlas_state.scale_cx,
        app.font,
        Some(dock),
        1.0,
        1.0,
    );
    let accent = color(theme.accent);
    let header_bottom = dock[1] + crate::term::dock::HEADER_HEIGHT;
    let accent_quads: Vec<_> = quads
        .iter()
        .filter(|q| q.color == accent && q.rect[1] < header_bottom)
        .collect();
    assert_eq!(accent_quads.len(), 1, "only the active tab is underlined");
    let underline = accent_quads[0].rect;
    assert_eq!(underline[3], 2.0, "2 pt underline");
    let tasks = ui_width(&mut app, PANEL_TAB_PT, "Tasks");
    assert!(
        (underline[2] - tasks).abs() < 0.5,
        "label set in the UI face"
    );
    let slots = crate::term::dock::header_slots(dock, 0, 1.0);
    assert!((underline[0] - (slots.tasks[0] + 8.0)).abs() < 0.01);
}

#[test]
fn sidebar_rows_indent_16_pt_per_level_and_share_a_name_column() {
    use crate::render::sidebar::{INDENT_PT, row_geometry};
    for scale in [1.0_f32, 2.0] {
        let top = row_geometry(0, scale);
        let nested = row_geometry(1, scale);
        assert_eq!(nested.name_x - top.name_x, INDENT_PT * scale);
        assert_eq!(nested.chevron_x - top.chevron_x, 16.0 * scale);
        assert!(top.chevron_x >= 8.0 * scale);
        assert!(top.icon_x >= top.chevron_x + crate::render::sidebar::CHEVRON_PT * scale);
        assert!(top.name_x >= top.icon_x + crate::render::sidebar::ICON_PT * scale);
    }
}

fn paint_sidebar(
    app: &mut crate::app::App,
    sidebar: &mut crate::sidebar::SidebarState,
    theme: &lux_ui::Theme,
) -> Vec<lux_render::QuadInstance> {
    let (mut quads, mut glyphs) = (Vec::new(), Vec::new());
    crate::render::sidebar::render_sidebar(
        sidebar,
        220.0,
        32.0,
        640.0,
        1.0,
        1.0,
        app.font,
        &mut app.editor_view,
        &mut app.atlas_state.atlas,
        &mut app.atlas_state.dirty,
        &mut app.atlas_state.scale_cx,
        theme,
        &mut quads,
        &mut glyphs,
    );
    quads
}

#[test]
fn the_selected_sidebar_row_uses_the_selection_color_and_an_accent_bar_only_with_focus() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    let mut sidebar = crate::sidebar::SidebarState::open_folder(dir.path().to_path_buf()).unwrap();
    let tree = sidebar.tree.as_mut().unwrap();
    let root = tree.root();
    tree.expand(root).unwrap();
    crate::sidebar::follow::select_listed(
        &mut sidebar,
        Some(&dir.path().join("a.txt")),
        1.0,
        600.0,
    );
    assert!(sidebar.selected_node.is_some());
    let mut app = crate::app::App::new(
        include_bytes!("../../../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
    );
    for scheme in crate::color_scheme::bundled_schemes().iter() {
        let theme = &scheme.theme;
        let accent = color(theme.accent);
        let fill = sidebar_selection(theme);
        let row_w = 220.0 - 1.0;
        for focused in [false, true] {
            sidebar.focused = focused;
            let quads = paint_sidebar(&mut app, &mut sidebar, theme);
            let row = quads
                .iter()
                .find(|q| q.color == fill && (q.rect[2] - row_w).abs() < 0.01)
                .unwrap_or_else(|| panic!("{} selected row", scheme.name));
            let row_h = crate::sidebar::SIDEBAR_ROW_HEIGHT;
            assert_eq!(row.rect[3], row_h);
            let bar = quads
                .iter()
                .any(|q| q.color == accent && q.rect[2] == 2.0 && q.rect[1] == row.rect[1]);
            assert_eq!(bar, focused, "{} focus bar", scheme.name);
            // The separator never turns accent with focus: one rule color.
            assert!(
                !quads
                    .iter()
                    .any(|q| q.color == accent && q.rect[3] >= 600.0),
                "{} no accent separator",
                scheme.name
            );
        }
    }
}
