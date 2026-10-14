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
    assert!(handled);
    assert_eq!(app.ui.split.active_pane, 1);

    // Click on left pane (x = 200.0, y = 300.0)
    app.mouse.position = dpi::PhysicalPosition::new(200.0, top as f64 + 300.0);
    let handled = app.handle_mouse_click_on_chrome(&ctx, MouseButton::Left);
    assert!(handled);
    assert_eq!(app.ui.split.active_pane, 0);
}

#[test]
fn test_layout_dispatch_actions() {
    let mut app = create_test_app_with_tabs(4);

    execute_keymap_command(&mut app, "set_layout_columns_2", None);
    assert_eq!(app.ui.split.preset, LayoutPreset::Columns2);

    execute_keymap_command(&mut app, "set_layout_rows_2", None);
    assert_eq!(app.ui.split.preset, LayoutPreset::Rows2);

    execute_keymap_command(&mut app, "set_layout_grid_4", None);
    assert_eq!(app.ui.split.preset, LayoutPreset::Grid2x2);

    execute_keymap_command(&mut app, "focus_group_3", None);
    assert_eq!(app.ui.split.active_pane, 2);

    execute_keymap_command(&mut app, "move_to_group_4", None);
    assert_eq!(app.ui.split.active_pane, 3);

    execute_keymap_command(&mut app, "set_layout_single", None);
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
