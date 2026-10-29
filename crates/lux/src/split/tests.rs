use super::*;

#[test]
fn split_editor_creates_secondary_pane_and_switches_focus() {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());

    assert!(app.ui.split.secondary.is_none());
    assert_eq!(app.ui.split.active_pane, 0);

    // Split editor right
    app.split_editor_right();
    assert!(app.ui.split.secondary.is_some());
    assert_eq!(app.ui.split.direction, SplitDirection::Horizontal);
    assert_eq!(app.ui.split.active_pane, 0);

    // Focus next split cycles to pane 1
    assert!(app.focus_next_split());
    assert_eq!(app.ui.split.active_pane, 1);

    // Focus next split cycles back to pane 0
    assert!(app.focus_next_split());
    assert_eq!(app.ui.split.active_pane, 0);

    // Split editor down
    app.split_editor_down();
    assert_eq!(app.ui.split.direction, SplitDirection::Vertical);

    // Close split
    assert!(app.close_active_split());
    assert!(app.ui.split.secondary.is_none());
    assert_eq!(app.ui.split.active_pane, 0);
}

#[test]
fn split_editor_syncs_same_buffer_edits() {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.insert_at_cursor("initial text");

    app.split_editor_right();
    let secondary = app.ui.split.secondary.as_ref().unwrap();
    assert_eq!(secondary.tab_id, app.documents.active_tab.unwrap());

    // Edit in current pane
    app.insert_at_cursor(" + edited");
    assert_eq!(app.buffer.text(), "initial text + edited");
}

#[test]
fn layouts_and_groups_lifecycle() {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());

    // Single layout initially
    assert_eq!(app.ui.split.preset, LayoutPreset::Single);
    assert_eq!(app.ui.split.num_panes(), 1);

    // 2 Columns
    app.set_layout(LayoutPreset::Columns2);
    assert_eq!(app.ui.split.num_panes(), 2);
    assert_eq!(app.ui.split.panes.len(), 2);

    // 3 Columns
    app.set_layout(LayoutPreset::Columns3);
    assert_eq!(app.ui.split.num_panes(), 3);
    assert_eq!(app.ui.split.panes.len(), 3);

    // 4 Columns
    app.set_layout(LayoutPreset::Columns4);
    assert_eq!(app.ui.split.num_panes(), 4);
    assert_eq!(app.ui.split.panes.len(), 4);

    // 2 Rows
    app.set_layout(LayoutPreset::Rows2);
    assert_eq!(app.ui.split.num_panes(), 2);

    // Grid 2x2
    app.set_layout(LayoutPreset::Grid2x2);
    assert_eq!(app.ui.split.num_panes(), 4);

    // Focus group 1..4
    assert!(app.focus_group(1));
    assert_eq!(app.ui.split.active_pane, 1);
    assert!(app.focus_group(3));
    assert_eq!(app.ui.split.active_pane, 3);
    assert!(!app.focus_group(4)); // out of bounds

    // Move to group
    assert!(app.move_to_group(2));
    assert_eq!(app.ui.split.active_pane, 2);

    // Back to Single
    app.set_layout(LayoutPreset::Single);
    assert_eq!(app.ui.split.num_panes(), 1);
    assert!(app.ui.split.panes.is_empty());
}

#[test]
fn geometry_computation_and_divider_hit_testing() {
    let split = SplitLayout {
        preset: LayoutPreset::Grid2x2,
        ..Default::default()
    };
    let rect = [0.0, 0.0, 800.0, 600.0];
    let geom = compute_layout_geometry(&split, rect, 1.0);
    assert_eq!(geom.pane_rects.len(), 4);
    assert_eq!(geom.dividers.len(), 2);

    // Vertical divider is at x = ~400
    let hit_v = hit_test_divider(&split, rect, 1.0, 400.0, 300.0);
    assert_eq!(hit_v, Some(DraggingDivider::Vertical(0)));

    // Horizontal divider is at y = ~300
    let hit_h = hit_test_divider(&split, rect, 1.0, 200.0, 300.0);
    assert_eq!(hit_h, Some(DraggingDivider::Horizontal(0)));

    // Pane 0 is top-left
    let pane_0 = hit_test_pane(&split, rect, 1.0, 100.0, 100.0);
    assert_eq!(pane_0, Some(0));

    // Pane 3 is bottom-right
    let pane_3 = hit_test_pane(&split, rect, 1.0, 600.0, 500.0);
    assert_eq!(pane_3, Some(3));
}
