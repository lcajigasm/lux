use super::*;
use crate::document::DocumentEntry;
use crate::input::dispatch::execute_keymap_command;
use crate::split::{
    DraggingDivider, LayoutPreset, compute_layout_geometry, hit_test_divider, hit_test_pane,
};
use lux_core::buffer::Buffer;
use lux_platform::{FileDialogOptions, FileDialogs, MouseButton, PlatformContext};
use std::path::PathBuf;
use std::sync::Arc;

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        vec![]
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
        lux_platform::dialog::SaveDecision::Discard
    }
}

static DUMMY_DIALOGS: DummyDialogs = DummyDialogs;

struct DummyContext;
impl lux_platform::Clipboard for DummyContext {
    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
        Ok(())
    }
}
impl PlatformContext for DummyContext {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(DummyDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &DUMMY_DIALOGS
    }
    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        self
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
        panic!("unused")
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
    fn exit(&self) {}
}

fn create_test_app_with_tabs(num_tabs: usize) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.viewport.width = 800.0;
    app.viewport.height = 600.0;
    for i in 1..num_tabs {
        let name = format!("tab_{}.rs", i);
        let tab_id = app.documents.tabs.open(&name);
        let buf = Buffer::from_bytes(format!("// content {}", i).as_bytes()).unwrap();
        app.documents
            .entries
            .push(DocumentEntry::new(tab_id, None, buf));
    }
    app
}

#[test]
fn test_layout_presets_and_single_return() {
    let mut app = create_test_app_with_tabs(4);
    assert_eq!(app.ui.split.preset, LayoutPreset::Single);
    assert!(!app.ui.split.is_active());

    let presets = [
        (LayoutPreset::Columns2, 2),
        (LayoutPreset::Columns3, 3),
        (LayoutPreset::Columns4, 4),
        (LayoutPreset::Rows2, 2),
        (LayoutPreset::Rows3, 3),
        (LayoutPreset::Grid2x2, 4),
    ];

    for (preset, expected_panes) in presets {
        app.set_layout(preset);
        assert_eq!(app.ui.split.preset, preset);
        assert!(app.ui.split.is_active());
        assert_eq!(app.ui.split.num_panes(), expected_panes);
        assert_eq!(app.ui.split.panes.len(), expected_panes);
        assert!(app.ui.split.secondary.is_some());
    }

    app.set_layout(LayoutPreset::Single);
    assert_eq!(app.ui.split.preset, LayoutPreset::Single);
    assert!(!app.ui.split.is_active());
    assert!(app.ui.split.secondary.is_none());
}

#[test]
fn test_focus_group_and_move_to_group() {
    let mut app = create_test_app_with_tabs(4);
    app.set_layout(LayoutPreset::Grid2x2);
    assert_eq!(app.ui.split.num_panes(), 4);

    assert!(app.focus_group(1));
    assert_eq!(app.ui.split.active_pane, 1);
    assert!(app.focus_group(3));
    assert_eq!(app.ui.split.active_pane, 3);
    assert!(app.focus_group(0));
    assert_eq!(app.ui.split.active_pane, 0);
    assert!(!app.focus_group(99)); // Out of bounds should return false

    let active_tab = app.documents.active_tab.unwrap();
    assert!(app.move_to_group(2));
    assert_eq!(app.ui.split.active_pane, 2);
    assert_eq!(app.ui.split.panes[2].tab_id, active_tab);

    assert!(app.move_to_group(0));
    assert_eq!(app.ui.split.active_pane, 0);
    assert_eq!(app.ui.split.panes[0].tab_id, active_tab);
}

#[test]
fn test_divider_hit_test_and_mouse_drag_resize() {
    let mut app = create_test_app_with_tabs(2);
    app.set_layout(LayoutPreset::Columns2);
    app.ui.split.ratio = 0.5;

    let ctx = DummyContext;
    let top = app.tab_bar_height();
    let bounds = [0.0, top, 800.0, 600.0];
    let geom = compute_layout_geometry(&app.ui.split, bounds, 1.0);
    assert_eq!(geom.dividers.len(), 1);

    let hit = hit_test_divider(&app.ui.split, bounds, 1.0, 400.0, top + 300.0);
    assert_eq!(hit, Some(DraggingDivider::Vertical(0)));

    // Position mouse directly on divider and click
    app.mouse.position = dpi::PhysicalPosition::new(400.0, top as f64 + 300.0);
    let handled = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(handled);
    assert_eq!(
        app.ui.split.dragging_divider,
        Some(DraggingDivider::Vertical(0))
    );

    // Simulate drag to x = 600.0 (ratio = 600/800 = 0.75)
    app.handle_divider_drag(600.0, top + 300.0);
    assert!((app.ui.split.ratio - 0.75).abs() < 0.05);

    // End drag
    app.end_divider_drag();
    assert!(app.ui.split.dragging_divider.is_none());

    // Test ratio clamping at bounds
    app.ui.split.dragging_divider = Some(DraggingDivider::Vertical(0));
    app.handle_divider_drag(20.0, top + 300.0);
    assert_eq!(app.ui.split.ratio, 0.1);

    app.handle_divider_drag(780.0, top + 300.0);
    assert_eq!(app.ui.split.ratio, 0.9);
    app.end_divider_drag();

    // Test horizontal divider dragging in Rows2
    app.set_layout(LayoutPreset::Rows2);
    app.ui.split.ratio_y = 0.5;
    let hit_h = hit_test_divider(&app.ui.split, bounds, 1.0, 400.0, top + 300.0);
    assert_eq!(hit_h, Some(DraggingDivider::Horizontal(0)));

    app.ui.split.dragging_divider = Some(DraggingDivider::Horizontal(0));
    app.handle_divider_drag(400.0, top + 450.0);
    assert!((app.ui.split.ratio_y - 0.75).abs() < 0.05);
    app.end_divider_drag();

    // Test double click on divider resets ratio to 0.5
    app.set_layout(LayoutPreset::Columns2);
    app.ui.split.ratio = 0.8;
    app.mouse.last_click = None;
    let div_x = 800.0 * 0.8;
    app.mouse.position = dpi::PhysicalPosition::new(div_x, top as f64 + 300.0);
    let handled1 = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(handled1);
    assert_eq!(app.ui.split.ratio, 0.8);
    let handled2 = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(handled2);
    assert_eq!(app.ui.split.ratio, 0.5);
    assert!(app.ui.split.dragging_divider.is_none());
}

#[test]
fn test_pane_click_focus_switching() {
    let mut app = create_test_app_with_tabs(3);
    app.set_layout(LayoutPreset::Columns2);
    app.ui.split.ratio = 0.5;
    app.ui.split.active_pane = 0;

    let ctx = DummyContext;
    let top = app.tab_bar_height();
    let bounds = [0.0, top, 800.0, 600.0];
    assert_eq!(
        hit_test_pane(&app.ui.split, bounds, 1.0, 600.0, top + 300.0),
        Some(1)
    );
    assert_eq!(
        hit_test_pane(&app.ui.split, bounds, 1.0, 200.0, top + 300.0),
        Some(0)
    );

    // Click on right pane (x = 600.0, y = 300.0)
    app.mouse.position = dpi::PhysicalPosition::new(600.0, top as f64 + 300.0);
    let handled = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(
        !handled,
        "pane click transfers focus without being consumed by chrome"
    );
    assert_eq!(app.ui.split.active_pane, 1);

    // Click on left pane (x = 200.0, y = 300.0)
    app.mouse.position = dpi::PhysicalPosition::new(200.0, top as f64 + 300.0);
    let handled = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(
        !handled,
        "pane click transfers focus without being consumed by chrome"
    );
    assert_eq!(app.ui.split.active_pane, 0);
}

#[test]
fn test_layout_dispatch_actions() {
    let mut app = create_test_app_with_tabs(4);

    execute_keymap_command(&mut app, "set_layout_columns_2", None, None);
    assert_eq!(app.ui.split.preset, LayoutPreset::Columns2);

    execute_keymap_command(&mut app, "set_layout_rows_2", None, None);
    assert_eq!(app.ui.split.preset, LayoutPreset::Rows2);

    execute_keymap_command(&mut app, "set_layout_grid_4", None, None);
    assert_eq!(app.ui.split.preset, LayoutPreset::Grid2x2);

    execute_keymap_command(&mut app, "focus_group_3", None, None);
    assert_eq!(app.ui.split.active_pane, 2);

    execute_keymap_command(&mut app, "move_to_group_4", None, None);
    assert_eq!(app.ui.split.active_pane, 3);

    execute_keymap_command(&mut app, "set_layout_single", None, None);
    assert_eq!(app.ui.split.preset, LayoutPreset::Single);
    assert!(!app.ui.split.is_active());
}

#[test]
fn test_close_split() {
    let mut app = create_test_app_with_tabs(2);
    app.split_editor_right();
    assert!(app.ui.split.is_active());

    assert!(app.close_active_split());
    assert!(!app.ui.split.is_active());
    assert_eq!(app.ui.split.preset, LayoutPreset::Single);
}

#[test]
fn test_split_click_focus_and_interactive_editing() {
    let mut app = create_test_app_with_tabs(3);
    app.insert_at_cursor("left initial");

    app.set_layout(LayoutPreset::Columns2);
    app.ui.split.ratio = 0.5;
    assert_eq!(app.ui.split.active_pane, 0);

    let ctx = DummyContext;
    let top = app.tab_bar_height();

    // 1. Click in right pane (pane 1) at x = 600.0, y = top + 20.0
    app.mouse.position = dpi::PhysicalPosition::new(600.0, top as f64 + 20.0);
    let handled = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(!handled, "click in pane must not be swallowed by chrome");
    assert_eq!(
        app.ui.split.active_pane, 1,
        "active pane must transfer to pane 1"
    );

    app.begin_mouse_selection();

    // 2. Edit right pane (type text)
    app.insert_at_cursor("RIGHT_EDITED ");
    assert!(app.buffer.text().contains("RIGHT_EDITED "));

    // 3. Delete backward in right pane
    for _ in 0..7 {
        app.handle_key(&KeyEvent {
            state: ElementState::Pressed,
            key: Key::Named(NamedKey::Backspace),
            text: None,
            repeat: false,
            physical: None,
        });
    }
    assert!(app.buffer.text().contains("RIGHT_ "));

    // 4. Click back in left pane (pane 0) at x = 40.0, y = top + 20.0 (at start of text)
    app.mouse.position = dpi::PhysicalPosition::new(40.0, top as f64 + 20.0);
    let handled_left = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(!handled_left);
    assert_eq!(
        app.ui.split.active_pane, 0,
        "active pane must transfer back to pane 0"
    );
    app.begin_mouse_selection();

    // 5. Edit left pane
    app.insert_at_cursor("LEFT_EDITED ");
    assert!(app.buffer.text().starts_with("LEFT_EDITED left initial"));

    // 6. Verify right pane's entry retained its edits
    let right_tab_id = app.ui.split.panes[1].tab_id;
    let right_entry = app
        .documents
        .entries
        .iter()
        .find(|e| e.tab_id == right_tab_id)
        .unwrap();
    assert!(right_entry.buffer.text().contains("RIGHT_ "));
}

#[test]
fn test_ctrl_numbers_focus_and_move_shortcuts() {
    let mut app = create_test_app_with_tabs(4);
    app.set_layout(LayoutPreset::Grid2x2);
    assert_eq!(app.ui.split.active_pane, 0);

    let ctx = DummyContext;

    // Test Ctrl+2 focuses pane 1
    let event_ctrl_2 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("2".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    app.modifiers.control = true;
    app.modifiers.meta = false;
    app.modifiers.shift = false;
    app.modifiers.alt = false;
    app.handle_keyboard_event(&ctx, &event_ctrl_2);
    assert_eq!(app.ui.split.active_pane, 1, "Ctrl+2 must focus pane 1");

    // Test Ctrl+3 focuses pane 2
    let event_ctrl_3 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("3".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    app.handle_keyboard_event(&ctx, &event_ctrl_3);
    assert_eq!(app.ui.split.active_pane, 2, "Ctrl+3 must focus pane 2");

    // Test Ctrl+4 focuses pane 3
    let event_ctrl_4 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("4".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    app.handle_keyboard_event(&ctx, &event_ctrl_4);
    assert_eq!(app.ui.split.active_pane, 3, "Ctrl+4 must focus pane 3");

    // Test Ctrl+1 focuses pane 0
    let event_ctrl_1 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("1".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    app.handle_keyboard_event(&ctx, &event_ctrl_1);
    assert_eq!(app.ui.split.active_pane, 0, "Ctrl+1 must focus pane 0");

    // Test Ctrl+Shift+2 moves active file to group 2
    let active_tab = app.documents.active_tab.unwrap();
    let event_ctrl_shift_2 = KeyEvent {
        state: ElementState::Pressed,
        key: Key::Character("2".into()),
        text: None,
        repeat: false,
        physical: None,
    };
    app.modifiers.shift = true;
    app.handle_keyboard_event(&ctx, &event_ctrl_shift_2);
    assert_eq!(
        app.ui.split.active_pane, 1,
        "Ctrl+Shift+2 must transfer focus to pane 1"
    );
    assert_eq!(
        app.ui.split.panes[1].tab_id, active_tab,
        "pane 1 must now hold the moved tab"
    );
    assert_eq!(app.documents.active_tab, Some(active_tab));
}

#[test]
fn test_split_visual_focus_indicator_and_static_cursors() {
    let mut app = create_test_app_with_tabs(2);
    app.set_layout(LayoutPreset::Columns2);
    app.ui.split.ratio = 0.5;
    app.ui.split.active_pane = 0;

    let mut quad_instances = Vec::new();
    let mut glyph_instances = Vec::new();

    let top = app.tab_bar_height();
    let bounds = [0.0, top, 800.0, 600.0];
    let geom = compute_layout_geometry(&app.ui.split, bounds, 1.0);

    let params = crate::render::split::SplitRenderParams {
        split: &app.ui.split,
        documents: &app.documents,
        primary_rect: bounds,
        scale_factor: 1.0,
        stroke: 1.0,
        font_size: 14.0,
        line_height: 20.0,
        font: app.font,
        active_syntax: app.syntax.as_ref(),
        active_buffer: &app.buffer,
        active_cursors: &app.cursors,
        active_fold_state: &app.fold_state,
        active_bookmarks: &app.bookmarks,
        elapsed: std::time::Duration::from_millis(100),
        active_scroll: (0.0, 0.0),
        theme: &app.chrome.theme,
        syntax_theme: lux_syntax::theme::by_name(app.chrome.theme.name)
            .unwrap_or(&lux_syntax::theme::LUX_DARK),
    };

    let mut gutter_quads = Vec::new();
    let mut gutter_glyphs = Vec::new();
    crate::render::split::render_split_layout(
        &params,
        &mut app.editor_view,
        &mut app.atlas_state,
        &mut quad_instances,
        &mut glyph_instances,
        &mut gutter_quads,
        &mut gutter_glyphs,
    );

    let accent_col = crate::render::paint::color(app.chrome.theme.accent);

    let p0_rect = geom.pane_rects[0];
    let p1_rect = geom.pane_rects[1];

    // Pane 0 (active) must have the top accent border
    let p0_has_accent_top = quad_instances.iter().any(|q| {
        (q.rect[0] - p0_rect[0]).abs() < 1.0
            && (q.rect[1] - p0_rect[1]).abs() < 1.0
            && (q.rect[2] - p0_rect[2]).abs() < 1.0
            && q.color == accent_col
    });
    assert!(
        p0_has_accent_top,
        "active pane 0 must have accent top border"
    );

    // Pane 1 (inactive) must NOT have the top accent border
    let p1_has_accent_top = quad_instances.iter().any(|q| {
        (q.rect[0] - p1_rect[0]).abs() < 1.0
            && (q.rect[1] - p1_rect[1]).abs() < 1.0
            && (q.rect[2] - p1_rect[2]).abs() < 1.0
            && q.color == accent_col
    });
    assert!(
        !p1_has_accent_top,
        "inactive pane 1 must not have accent top border"
    );

    // Inactive pane 1 must render a static caret with alpha 0.65
    let p1_has_static_caret = quad_instances.iter().any(|q| {
        q.rect[0] >= p1_rect[0]
            && q.rect[0] <= p1_rect[0] + p1_rect[2]
            && (q.color[3] - 0.65).abs() < 0.05
    });
    assert!(
        p1_has_static_caret,
        "inactive pane 1 must render static caret"
    );
}
