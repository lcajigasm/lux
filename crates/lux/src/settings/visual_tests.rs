//! Settings page geometry and the quads the renderer emits for it.

use lux_platform::event::KeyEvent;
use lux_platform::{
    ElementState, FileDialogs, Key, NamedKey, PlatformContext, PlatformError, ScrollDelta,
};
use lux_render::QuadInstance;
use std::path::PathBuf;
use std::sync::Arc;

use super::layout::{self, SettingsBounds, layout_settings};
use super::panel::SettingKind;
use crate::app::App;
use crate::render::paint::color;
use crate::render::panel::selected_fill;

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

fn bounds(app: &App) -> SettingsBounds {
    super::panel::bounds_for(app)
}

fn paint(
    app: &mut App,
    width: f32,
    height: f32,
    scale: f32,
) -> (Vec<QuadInstance>, Vec<lux_render::GlyphInstance>) {
    app.scale_factor = scale;
    let panel = app.ui.settings.panel.clone().expect("settings page");
    let mut quads = Vec::new();
    let mut glyphs = Vec::new();
    crate::render::settings::render_settings_panel(
        &panel,
        &mut quads,
        &mut glyphs,
        &mut app.editor_view,
        &mut app.atlas_state.atlas,
        &mut app.atlas_state.dirty,
        &mut app.atlas_state.scale_cx,
        app.font,
        width,
        height,
        32.0 * scale,
        24.0 * scale,
        scale,
        &lux_ui::Theme::MONOKAI,
    );
    (quads, glyphs)
}

fn same_rgb(a: [f32; 4], b: [f32; 4]) -> bool {
    (a[0] - b[0]).abs() < 0.02 && (a[1] - b[1]).abs() < 0.02 && (a[2] - b[2]).abs() < 0.02
}

fn page_rect(scale: f32) -> [f32; 4] {
    let tab = 32.0 * scale;
    let status = 24.0 * scale;
    [0.0, tab, 1000.0 * scale, 700.0 * scale - tab - status]
}

#[test]
fn settings_panel_has_no_debug_magenta_border() {
    for scale in [1.0_f32, 2.0] {
        let mut app = app();
        app.ui.settings.open_settings_ui();
        let width = 1000.0 * scale;
        let height = 700.0 * scale;
        let (quads, glyphs) = paint(&mut app, width, height, scale);
        let accent = color(lux_ui::Theme::MONOKAI.accent);
        let [page_x, page_y, page_w, page_h] = page_rect(scale);
        let page_right = page_x + page_w;
        let page_bottom = page_y + page_h;
        let opaque = quads.iter().any(|quad| {
            let [x, y, w, h] = quad.rect;
            (x - page_x).abs() < 0.5
                && (y - page_y).abs() < 0.5
                && (w - page_w).abs() < 0.5
                && (h - page_h).abs() < 0.5
                && quad.color[3] > 0.99
                && !same_rgb(quad.color, accent)
        });
        assert!(
            opaque,
            "the settings page covers the editor with an opaque fill"
        );
        for quad in &quads {
            if !same_rgb(quad.color, accent) {
                continue;
            }
            let [x, y, w, h] = quad.rect;
            let inside = x >= page_x + 8.0 * scale
                && y >= page_y + 8.0 * scale
                && x + w <= page_right - 8.0 * scale
                && y + h <= page_bottom - 8.0 * scale;
            assert!(
                inside && w <= 48.0 * scale + 0.5 && h <= 26.0 * scale + 0.5,
                "accent quad {x},{y} {w}x{h} is a debug border, not a toggle"
            );
        }
        assert!(
            glyphs.iter().all(|glyph| !same_rgb(glyph.color, accent)),
            "settings labels and values stay off the accent color"
        );
    }
}

#[test]
fn settings_selection_stays_inside_the_card() {
    let mut app = app();
    app.ui.settings.open_settings_ui();
    let frame = bounds(&app);
    let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
    let margin = 8.0 * frame.scale;
    for row in &layout.rows {
        let Some(painted) = layout.painted_selection(row) else {
            continue;
        };
        assert!(
            layout.page.contains_rect(painted),
            "selection leaves the settings page"
        );
        assert!(painted.x >= layout.page.x + margin);
        assert!(painted.right() <= layout.page.right() - margin);
    }
    let first = layout
        .painted_selection(&layout.rows[0])
        .expect("first row");
    assert!(layout.page.contains_rect(first));
    assert!(first.y >= layout.page.y + margin);

    if let Some(panel) = app.ui.settings.panel.as_mut() {
        panel.scroll = 10_000.0;
        panel.selected_index = 0;
    }
    let (quads, _) = paint(
        &mut app,
        frame.window_width,
        frame.window_height,
        frame.scale,
    );
    let fill = selected_fill();
    let page = layout.page;
    for quad in quads.iter().filter(|quad| same_rgb(quad.color, fill)) {
        let [x, y, w, h] = quad.rect;
        let rect = layout::Rect { x, y, w, h };
        assert!(
            page.contains_rect(rect),
            "painted selection clips outside the card"
        );
        assert!(rect.x >= page.x + margin);
        assert!(rect.right() <= page.right() - margin);
    }
}

#[test]
fn settings_row_lays_out_label_description_and_value() {
    let mut app = app();
    app.ui.settings.open_settings_ui();
    let frame = bounds(&app);
    let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
    assert!(layout.rows.len() >= 12);
    let right = layout.rows[0].control.right();
    for row in &layout.rows {
        assert!((row.label.x - row.description.x).abs() < 0.1);
        assert!(row.description.y >= row.label.bottom() - 0.1);
        assert!(row.label.right() + 8.0 <= row.control.x);
        assert!((row.control.center_y() - row.row.center_y()).abs() < 0.6);
        assert!((row.control.right() - right).abs() < 0.1);
        assert!(row.label.x >= layout.body.x - 0.1);
        assert!(row.control.right() <= layout.body.right() + 0.1);
        assert!(row.description.bottom() <= row.row.bottom() + 0.1);
    }
    assert_eq!(layout::format_number(13.0), "13");
    assert_eq!(layout::format_number(1.4), "1.4");
    assert_eq!(layout::format_number(4.0), "4");

    let (quads, _) = paint(
        &mut app,
        frame.window_width,
        frame.window_height,
        frame.scale,
    );
    let plus = match layout.rows[0].kind {
        layout::ControlGeom::Stepper { plus, .. } => plus,
        _ => panic!("font size is a stepper"),
    };
    assert!(
        quads.iter().any(|quad| {
            let [x, y, w, h] = quad.rect;
            (x - plus.x).abs() < 0.6
                && (y - plus.y).abs() < 0.6
                && (w - plus.w).abs() < 0.6
                && (h - plus.h).abs() < 0.6
        }),
        "the stepper button is painted on the value rect"
    );
}

#[test]
fn settings_toggle_stepper_and_menu_change_the_stored_value() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app();
    app.ui.settings = super::SettingsState::new(dir.path().join("settings.json"));
    app.ui.settings.open_settings_ui();
    let frame = bounds(&app);

    let toggle = {
        let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
        match layout.rows[2].kind {
            layout::ControlGeom::Toggle { track, .. } => track,
            _ => panic!("ligatures is a toggle"),
        }
    };
    let (x, y) = (toggle.x + toggle.w * 0.5, toggle.y + toggle.h * 0.5);
    click(&mut app, x, y);
    assert!(matches!(
        app.ui.settings.panel.as_ref().unwrap().items[2].kind,
        SettingKind::Boolean { value: false }
    ));

    let plus = {
        let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
        match layout.rows[0].kind {
            layout::ControlGeom::Stepper { plus, .. } => plus,
            _ => panic!("font size is a stepper"),
        }
    };
    click(&mut app, plus.x + plus.w * 0.5, plus.y + plus.h * 0.5);
    match &app.ui.settings.panel.as_ref().unwrap().items[0].kind {
        SettingKind::Number { value, .. } => assert!((*value - 14.0).abs() < 0.01),
        _ => panic!("font size"),
    }

    reveal(&mut app, 10);
    let button = {
        let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
        match layout.rows[10].kind {
            layout::ControlGeom::Menu { button } => button,
            _ => panic!("theme is a menu"),
        }
    };
    click(&mut app, button.x + 8.0, button.y + button.h * 0.5);
    assert!(app.ui.settings.panel.as_ref().unwrap().menu.is_some());
    let option = {
        let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
        layout.menu.expect("menu").options[2]
    };
    click(&mut app, option.x + 6.0, option.y + option.h * 0.5);
    match &app.ui.settings.panel.as_ref().unwrap().items[10].kind {
        SettingKind::Enum { value, .. } => assert_eq!(value, "mariana"),
        _ => panic!("theme"),
    }
    assert!(app.ui.settings.panel.as_ref().unwrap().menu.is_none());
}

#[test]
fn settings_wheel_scrolls_the_page_and_not_the_editor() {
    let mut app = app();
    app.ui.settings.open_settings_ui();
    app.viewport.scroll_y = 40.0;
    assert!(crate::overlay_wheel::scroll_open_overlay(
        &mut app,
        ScrollDelta::Lines { x: 0.0, y: -3.0 }
    ));
    assert!(app.ui.settings.panel.as_ref().unwrap().scroll > 0.0);
    assert!((app.viewport.scroll_y - 40.0).abs() < 0.01);
}

#[test]
fn escape_closes_the_menu_before_the_page() {
    let mut app = app();
    app.ui.settings.open_settings_ui();
    let frame = bounds(&app);
    reveal(&mut app, 10);
    let button = {
        let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
        match layout.rows[10].kind {
            layout::ControlGeom::Menu { button } => button,
            _ => panic!("theme is a menu"),
        }
    };
    click(&mut app, button.x + 8.0, button.y + 4.0);
    assert!(app.ui.settings.panel.as_ref().unwrap().menu.is_some());
    app.handle_keyboard_event(&Ctx, &escape());
    assert!(app.ui.settings.panel.as_ref().unwrap().menu.is_none());
    app.handle_keyboard_event(&Ctx, &escape());
    assert!(app.ui.settings.panel.is_none());
}

fn reveal(app: &mut App, index: usize) {
    let frame = bounds(app);
    let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
    let row = &layout.rows[index];
    let dy = if row.row.y < layout.body.y {
        row.row.y - layout.body.y
    } else if row.row.bottom() > layout.body.bottom() {
        row.row.bottom() - layout.body.bottom()
    } else {
        0.0
    };
    if let Some(panel) = app.ui.settings.panel.as_mut() {
        panel.scroll = (panel.scroll + dy).max(0.0);
        panel.selected_index = index;
    }
    let layout = layout_settings(app.ui.settings.panel.as_ref().unwrap(), frame);
    if let Some(panel) = app.ui.settings.panel.as_mut() {
        panel.scroll = layout.scroll;
    }
}

fn click(app: &mut App, x: f32, y: f32) {
    let mut settings = std::mem::take(&mut app.ui.settings);
    settings.handle_click(app, x, y);
    app.ui.settings = settings;
}

fn escape() -> KeyEvent {
    KeyEvent {
        key: Key::Named(NamedKey::Escape),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    }
}

struct Dialogs;
impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
        lux_platform::dialog::SaveDecision::Discard
    }
}

static DIALOGS: Dialogs = Dialogs;

struct Ctx;
impl lux_platform::Clipboard for Ctx {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}
impl PlatformContext for Ctx {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(Dialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &DIALOGS
    }
    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        self
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, PlatformError> {
        panic!("unused")
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
    fn exit(&self) {}
}
