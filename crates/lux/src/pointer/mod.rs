//! The one place that chooses the pointer.
//!
//! `decide` maps the pointer position onto a zone. `sync` applies that
//! cursor on every move, `press`/`release` drag the sidebar edge, and
//! `leave` puts the arrow back when the pointer exits the window.

mod gutter;
mod preview;

use lux_platform::{CursorShape, MouseButton};

use crate::app::App;

/// Probe fields the pointer-zone job reads. Kept here so `probe.rs`
/// stays inside the line limit.
pub(crate) fn probe_lines(app: &App) -> String {
    let language = app.ui.status_bar.layout.language_rect;
    let mut out = format!(
        "pointer={}\nlanguage_rect={:.1},{:.1},{:.1},{:.1}\nscale={}\nsidebar_w={}\nminimap={}\nviewport={:.1},{:.1}\n",
        shape_name(app.mouse.shape),
        language.x,
        language.y,
        language.width,
        language.height,
        app.scale_factor,
        app.sidebar_width(),
        app.minimap_width(),
        app.viewport.width,
        app.viewport.height,
    );
    let strip = crate::tab_strip::layout(app);
    out.push_str(&format!(
        "tab_strip={:.1},{:.1},{:.1}\n",
        strip.left, strip.visible_width, strip.tab_width
    ));
    if let Some((x, width)) = preview_pane(app) {
        out.push_str(&format!("preview_pane={x:.1},{width:.1}\n"));
    }
    if let Some(y) = preview::first_link_y(app) {
        out.push_str(&format!("link_y={y:.1}\n"));
    }
    if let Some(gpu) = &app.gpu_window {
        let size = gpu.window.inner_size();
        if let Some((x, y)) = gpu.window.inner_position() {
            out.push_str(&format!("content={x},{y},{},{}\n", size.width, size.height));
        }
    }
    out
}

pub(crate) fn shape_name(shape: CursorShape) -> &'static str {
    match shape {
        CursorShape::Arrow => "arrow",
        CursorShape::Text => "text",
        CursorShape::Hand => "hand",
        CursorShape::ResizeEastWest => "ew",
        CursorShape::ResizeNorthSouth => "ns",
    }
}

pub(crate) fn sync(app: &mut App) {
    if sidebar_resizing(app) {
        drag_sidebar(app);
    }
    crate::minimap::drag::update(app);
    let shape = if !app.mouse.placed {
        CursorShape::Arrow
    } else if sidebar_resizing(app) {
        CursorShape::ResizeEastWest
    } else {
        decide(app)
    };
    app.mouse.shape = shape;
    apply(app, shape);
    let hover = gutter::hovered(app);
    gutter::update(app, hover);
}

pub(crate) fn leave(app: &mut App) {
    app.mouse.placed = false;
    if crate::tab_strip::clear_hover(app)
        && let Some(gpu) = &app.gpu_window
    {
        gpu.window.request_redraw();
    }
    // The button may come up outside the window, where no release arrives.
    crate::minimap::drag::release(app);
    if let Some(sidebar) = &mut app.ui.sidebar {
        sidebar.resizing = false;
    }
    app.mouse.shape = CursorShape::Arrow;
    apply(app, CursorShape::Arrow);
    gutter::update(app, None);
}

/// Starts a sidebar-edge drag. Returns true when this press is that edge,
/// so the click does not also land on a tree row.
pub(crate) fn press(app: &mut App, button: MouseButton) -> bool {
    if button != MouseButton::Left || !app.mouse.placed {
        return false;
    }
    let x = app.mouse.position.x as f32;
    let y = app.mouse.position.y as f32;
    if !sash_hit(app, x, y) {
        return false;
    }
    if let Some(sidebar) = &mut app.ui.sidebar {
        sidebar.resizing = true;
    }
    true
}

/// The window lost focus: no release for a held button will arrive, so
/// every pointer drag ends here.
pub(crate) fn focus_lost(app: &mut App) {
    release(app);
    crate::sidebar::drag::cancel(app);
}

pub(crate) fn release(app: &mut App) {
    crate::minimap::drag::release(app);
    if let Some(sidebar) = &mut app.ui.sidebar {
        sidebar.resizing = false;
    }
}

pub(crate) fn decide(app: &mut App) -> CursorShape {
    let x = app.mouse.position.x as f32;
    let y = app.mouse.position.y as f32;
    if let Some(shape) = overlay(app, x, y) {
        return shape;
    }
    if let Some((_, hit)) = crate::tab_strip::hit(app, x, y) {
        return if matches!(hit, crate::tab_strip::RowHit::Tab(..)) {
            CursorShape::Hand
        } else {
            CursorShape::Arrow
        };
    }
    let editor_top = app.tab_bar_height();
    let editor_bottom = editor_top + app.viewport.height;
    if y >= editor_bottom {
        return below_editor(app, x, y);
    }
    if let Some(shape) = divider(app, x, y) {
        return shape;
    }
    if sash_hit(app, x, y) || sidebar_resizing(app) {
        return CursorShape::ResizeEastWest;
    }
    let sidebar = app.sidebar_width();
    if x < sidebar {
        return CursorShape::Arrow;
    }
    if x >= sidebar + app.viewport.width {
        return CursorShape::Arrow;
    }
    if let Some((pane_x, pane_w)) = preview_pane(app)
        && x >= pane_x
        && x < pane_x + pane_w
    {
        return preview::hit(app, x, y, pane_x).unwrap_or(CursorShape::Arrow);
    }
    if app.ui.split.is_active() {
        return split_pane(app, x, y);
    }
    let width = editor_width(app);
    editor_zone(app, x, y, [sidebar, editor_top, width, app.viewport.height])
}

fn overlay(app: &App, x: f32, y: f32) -> Option<CursorShape> {
    if let Some(about) = &app.ui.dialogs.about {
        let [bx, by, bw, bh] = about.close_btn_rect;
        if contains(bx, by, bw, bh, x, y) {
            return Some(CursorShape::Hand);
        }
        let [cx, cy, cw, ch] = about.card_rect;
        if contains(cx, cy, cw, ch, x, y) {
            return Some(CursorShape::Arrow);
        }
    }
    if over_menu(app, x, y) {
        return Some(CursorShape::Arrow);
    }
    None
}

fn over_menu(app: &App, x: f32, y: f32) -> bool {
    if let Some(menu) = &app.mouse.context_menu {
        let [rx, ry, rw, rh] = menu.rect(app.scale_factor);
        if contains(rx, ry, rw, rh, x, y) {
            return true;
        }
    }
    if let Some(menu) = &app.documents.tab_context_menu {
        let [rx, ry, rw, rh] = menu.rect;
        if contains(rx, ry, rw, rh, x, y) {
            return true;
        }
    }
    app.ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.context_menu.as_ref())
        .is_some_and(|menu| {
            let [rx, ry, rw, rh] = menu.rect;
            contains(rx, ry, rw, rh, x, y)
        })
}

fn below_editor(app: &App, x: f32, y: f32) -> CursorShape {
    let frame = app.frame_layout();
    let inside = |rect: Option<[f32; 4]>| {
        rect.is_some_and(|[rx, ry, rw, rh]| x >= rx && x < rx + rw && y >= ry && y < ry + rh)
    };
    if inside(frame.dock) {
        return CursorShape::Text;
    }
    if inside(frame.search_bar) {
        return CursorShape::Arrow;
    }
    let layout = &app.ui.status_bar.layout;
    let buttons = [
        layout.cursor_rect,
        layout.branch_rect,
        layout.indent_rect,
        layout.encoding_rect,
        layout.eol_rect,
        layout.language_rect,
    ];
    if buttons.iter().any(|rect| rect.contains(x, y)) {
        CursorShape::Hand
    } else {
        CursorShape::Arrow
    }
}

fn divider(app: &App, x: f32, y: f32) -> Option<CursorShape> {
    if let Some(dragging) = app.ui.split.dragging_divider {
        return Some(divider_shape(dragging));
    }
    if !app.ui.split.is_active() {
        return None;
    }
    let rect = crate::split::groups::editor_area(app);
    crate::split::hit_test_divider(&app.ui.split, rect, app.scale_factor, x, y).map(divider_shape)
}

fn divider_shape(divider: crate::split::DraggingDivider) -> CursorShape {
    match divider {
        crate::split::DraggingDivider::Vertical(_) => CursorShape::ResizeEastWest,
        crate::split::DraggingDivider::Horizontal(_) => CursorShape::ResizeNorthSouth,
    }
}

fn split_pane(app: &mut App, x: f32, y: f32) -> CursorShape {
    for group in crate::split::groups::rects(app) {
        if crate::split::groups::contains(group.text, x, y) {
            return editor_zone(app, x, y, group.text);
        }
    }
    CursorShape::Arrow
}

fn editor_zone(app: &mut App, x: f32, y: f32, pane: [f32; 4]) -> CursorShape {
    if !contains(pane[0], pane[1], pane[2], pane[3], x, y) {
        return CursorShape::Arrow;
    }
    if x >= text_left(app, pane[0]) {
        CursorShape::Text
    } else {
        CursorShape::Arrow
    }
}

fn text_left(app: &mut App, pane_x: f32) -> f32 {
    if app.ui.distraction_free.is_some() {
        let columns = app
            .ui
            .distraction_free
            .as_ref()
            .map_or(80, |mode| mode.column_width_chars);
        let char_width = app.editor_view.reference_width("0");
        let content = char_width * columns as f32;
        let window = window_width(app);
        return ((window - content) / 2.0).max(24.0 * app.scale_factor);
    }
    pane_x + gutter_width(app) + 6.0 * app.scale_factor
}

fn gutter_width(app: &mut App) -> f32 {
    let digits = app.buffer.len_lines().max(1).to_string().len();
    let sample = "0".repeat(digits);
    let width = app.editor_view.reference_width(&sample);
    crate::render::text::gutter_total_width(width, app.scale_factor)
}

fn window_width(app: &App) -> f32 {
    if let Some(gpu) = &app.gpu_window {
        return gpu.window.inner_size().width as f32;
    }
    app.sidebar_width() + app.viewport.width + app.minimap_width()
}

/// Same pane rectangle `render.rs` paints. The preview width is taken off
/// `viewport.width` after the sidebar has already been excluded, and this
/// copies that so the pointer and the paint agree.
fn preview_pane(app: &App) -> Option<(f32, f32)> {
    if app.ui.split.is_active() {
        return None;
    }
    let html = app.documents.active_entry().is_some_and(|entry| {
        entry.markdown_preview && entry.language == Some(lux_syntax::LanguageId::Html)
    });
    let markdown = app.documents.active_entry().is_some_and(|entry| {
        entry.markdown_preview && entry.language == Some(lux_syntax::LanguageId::Markdown)
    });
    if !html && !markdown {
        return None;
    }
    let sidebar = app.sidebar_width();
    let content_width = (app.viewport.width - sidebar).max(1.0);
    let pane_cap = (content_width * 0.62).max(1.0);
    let preview_width = (content_width * 0.46).clamp(180.0_f32.min(pane_cap), pane_cap);
    let editor_width = (content_width - preview_width).max(1.0);
    Some((sidebar + editor_width, preview_width))
}

fn editor_width(app: &App) -> f32 {
    match preview_pane(app) {
        Some((pane_x, _)) => (pane_x - app.sidebar_width()).max(1.0),
        None => app.viewport.width,
    }
}

fn sash_hit(app: &App, x: f32, y: f32) -> bool {
    let edge = app.sidebar_width();
    if edge <= 0.0 {
        return false;
    }
    let slop = 4.0 * app.scale_factor;
    // The sidebar is a full-height column, beside the find bar and dock.
    let top = app.tab_bar_height();
    let bottom = app.window_extent().1 - app.status_bar_height();
    (x - edge).abs() <= slop && y >= top && y < bottom
}

fn sidebar_resizing(app: &App) -> bool {
    app.ui
        .sidebar
        .as_ref()
        .is_some_and(|sidebar| sidebar.resizing)
}

fn drag_sidebar(app: &mut App) {
    let scale = app.scale_factor.max(0.5);
    let width = (app.mouse.position.x as f32 / scale).clamp(120.0, 480.0);
    let changed = if let Some(sidebar) = &mut app.ui.sidebar {
        if (sidebar.width - width).abs() > 0.5 {
            sidebar.width = width;
            true
        } else {
            false
        }
    } else {
        false
    };
    if changed {
        app.update_viewport_dimensions();
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
}

fn apply(app: &App, shape: CursorShape) {
    if let Some(gpu) = &app.gpu_window {
        gpu.window.set_cursor(shape);
    }
}

fn contains(x: f32, y: f32, w: f32, h: f32, px: f32, py: f32) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_core::buffer::Buffer;
    use lux_syntax::LanguageId;

    fn app() -> App {
        App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
    }

    fn at(app: &mut App, x: f32, y: f32) -> CursorShape {
        app.mouse.placed = true;
        app.mouse.position.x = f64::from(x);
        app.mouse.position.y = f64::from(y);
        decide(app)
    }

    #[test]
    fn editor_text_is_an_ibeam_and_the_gutter_is_an_arrow() {
        let mut app = app();
        app.buffer = Buffer::from_bytes(b"hello\n").expect("text");
        let gutter = at(&mut app, 8.0, 80.0);
        let text = at(&mut app, 80.0, 80.0);
        assert_eq!(gutter, CursorShape::Arrow);
        assert_eq!(text, CursorShape::Text);
    }

    #[test]
    fn a_tab_and_its_close_button_are_a_hand() {
        let mut app = app();
        assert_eq!(at(&mut app, 40.0, 10.0), CursorShape::Hand);
        assert_eq!(at(&mut app, 190.0, 10.0), CursorShape::Hand);
        assert_eq!(at(&mut app, 900.0, 10.0), CursorShape::Arrow);
    }

    #[test]
    fn the_sidebar_edge_resizes_and_the_tree_does_not() {
        let dir = std::env::temp_dir().join(format!("lux-pointer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("note.txt"), "hello\n").expect("file");
        let mut app = app();
        app.open_workspace(dir);
        assert_eq!(at(&mut app, 40.0, 80.0), CursorShape::Arrow);
        assert_eq!(at(&mut app, 220.0, 120.0), CursorShape::ResizeEastWest);
        assert_eq!(at(&mut app, 320.0, 80.0), CursorShape::Text);
        app.mouse.position.x = 220.0;
        app.mouse.position.y = 120.0;
        assert!(press(&mut app, MouseButton::Left));
        app.mouse.position.x = 300.0;
        sync(&mut app);
        let sidebar = app.ui.sidebar.as_ref().expect("sidebar");
        assert_eq!(sidebar.width, 300.0);
        assert!(sidebar.resizing);
        release(&mut app);
        assert!(!app.ui.sidebar.as_ref().expect("sidebar").resizing);
    }

    #[test]
    fn split_dividers_use_the_resize_arrows() {
        let mut app = app();
        app.set_layout(crate::split::LayoutPreset::Columns2);
        let [x, y, w, h] = crate::split::groups::editor_area(&app);
        assert_eq!(
            at(&mut app, x + w * 0.5, 200.0),
            CursorShape::ResizeEastWest
        );
        app.set_layout(crate::split::LayoutPreset::Rows2);
        assert_eq!(
            at(&mut app, 200.0, y + h * 0.5),
            CursorShape::ResizeNorthSouth
        );
    }

    #[test]
    fn a_menu_is_an_arrow_and_a_status_button_is_a_hand() {
        let mut app = app();
        app.mouse.context_menu = Some(crate::edit_menu::BufferContextMenu::at(
            &mut app, 80.0, 80.0,
        ));
        let [x, y, _, _] = app.mouse.context_menu.as_ref().expect("menu").rect(1.0);
        assert_eq!(at(&mut app, x + 8.0, y + 8.0), CursorShape::Arrow);
        app.mouse.context_menu = None;
        app.ui.status_bar.layout.language_rect = crate::status_bar::Rect {
            x: 800.0,
            y: 680.0,
            width: 80.0,
            height: 20.0,
        };
        assert_eq!(at(&mut app, 820.0, 688.0), CursorShape::Hand);
        assert_eq!(at(&mut app, 20.0, 688.0), CursorShape::Arrow);
    }

    #[test]
    fn preview_text_is_an_ibeam_and_a_link_is_a_hand() {
        let mut app = app();
        let source = b"# Title\n\nSee [Lux](https://example.com).\n";
        app.buffer = Buffer::from_bytes(source).expect("markdown");
        app.documents.entries[0].language = Some(LanguageId::Markdown);
        app.documents.entries[0].markdown_preview = true;
        let line = app.editor_view.metrics().line_height;
        let heading_y = app.tab_bar_height() + line * 0.25;
        let (pane_x, _) = preview_pane(&app).expect("pane");
        let text_left = pane_x + 16.0;
        assert_eq!(
            at(&mut app, text_left + 4.0, heading_y + 4.0),
            CursorShape::Text
        );
        assert_eq!(
            at(&mut app, pane_x + 4.0, heading_y + 4.0),
            CursorShape::Arrow
        );
        let size = app.editor_view.metrics().font_size * 1.55;
        let paragraph_y = heading_y + size * 1.4 + line * 0.35;
        let link_y = preview::first_link_y(&app).expect("markdown link");
        assert!((link_y - (paragraph_y + app.editor_view.metrics().font_size * 0.5)).abs() < 0.1);
        assert_eq!(
            at(&mut app, text_left + 2.0, paragraph_y + 4.0),
            CursorShape::Text
        );
        let see = {
            let family = app.editor_view.family().to_string();
            let style = lux_render::LineStyle {
                family: &family,
                font_size: app.editor_view.metrics().font_size,
                ligatures: false,
            };
            app.editor_view
                .text_layout_mut()
                .layout_line(&style, "See ", &[])
                .width()
        };
        assert_eq!(
            at(&mut app, text_left + see + 2.0, paragraph_y + 4.0),
            CursorShape::Hand
        );

        let html = b"<h1>Title</h1>\n<p>See <a href=\"https://example.com\">Lux</a>.</p>\n";
        app.buffer = Buffer::from_bytes(html).expect("html");
        app.documents.entries[0].language = Some(LanguageId::Html);
        let html_size = app.editor_view.metrics().font_size * 1.45;
        let html_paragraph = heading_y + html_size * 1.6;
        let html_link = preview::first_link_y(&app).expect("html link");
        assert!(
            (html_link - (html_paragraph + app.editor_view.metrics().font_size * 0.5)).abs() < 0.1
        );
        assert_eq!(
            at(&mut app, text_left + 4.0, heading_y + 4.0),
            CursorShape::Text
        );
        assert_eq!(
            at(&mut app, text_left + see + 2.0, html_paragraph + 4.0),
            CursorShape::Hand
        );
    }

    #[test]
    fn leaving_the_window_restores_the_arrow() {
        let mut app = app();
        app.mouse.placed = true;
        app.mouse.shape = CursorShape::Text;
        if let Some(sidebar) = &mut app.ui.sidebar {
            sidebar.resizing = true;
        }
        leave(&mut app);
        assert!(!app.mouse.placed);
        assert_eq!(app.mouse.shape, CursorShape::Arrow);
        assert!(!sidebar_resizing(&app));
    }
}
