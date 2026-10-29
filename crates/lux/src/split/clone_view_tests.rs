//! Unit tests for cloned views across split panes (ST-SPLIT-03).

use crate::app::App;
use crate::split::LayoutPreset;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::{
    Clipboard, FileDialogOptions, FileDialogs, PlatformContext, PlatformError, PlatformWindow,
    ScrollDelta, WindowAttributes,
};
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
impl Clipboard for DummyContext {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
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
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("dummy".to_string()))
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn fixture_font() -> Vec<u8> {
    include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec()
}

#[test]
fn test_clone_view_to_group_creates_columns_and_copies_state() {
    let mut app = App::new(fixture_font());
    app.insert_at_cursor("hello world\nsecond line");
    assert_eq!(app.ui.split.preset, LayoutPreset::Single);
    assert_eq!(app.ui.split.num_panes(), 1);

    let original_tab = app.documents.active_tab.unwrap();

    // Clone view to group (target None defaults to next group: 1)
    let cloned = app.clone_view_to_group(None);
    assert!(cloned);
    assert_eq!(app.ui.split.preset, LayoutPreset::Columns2);
    assert_eq!(app.ui.split.num_panes(), 2);
    assert_eq!(app.ui.split.active_pane, 1);

    // Both pane 0 and pane 1 share the same tab_id
    assert_eq!(app.ui.split.panes[0].tab_id, original_tab);
    assert_eq!(app.ui.split.panes[1].tab_id, original_tab);

    // Buffer content is identical (shared underlying buffer)
    assert_eq!(app.buffer.text(), "hello world\nsecond line");
}

#[test]
fn test_cloned_views_have_independent_cursors_and_selections() {
    let mut app = App::new(fixture_font());
    app.insert_at_cursor("first line\nsecond line\nthird line");

    // Position cursor at start in pane 0
    let start_pos = Position::document_start();
    app.cursors = MultiCursor::new(Selection::cursor(start_pos));

    // Clone to group 1
    assert!(app.clone_view_to_group(Some(1)));
    assert_eq!(app.ui.split.active_pane, 1);

    // In pane 1, move cursor to second line
    let line2_pos = Position::from_char(&app.buffer, 11);
    app.cursors = MultiCursor::new(Selection::cursor(line2_pos));
    assert_eq!(app.cursors.primary().head.char, 11);

    // Focus back to pane 0
    assert!(app.focus_group(0));
    assert_eq!(app.ui.split.active_pane, 0);
    // Cursor in pane 0 is still at start (char 0)
    assert_eq!(app.cursors.primary().head.char, 0);

    // Select "first" in pane 0
    let sel_end = Position::from_char(&app.buffer, 5);
    app.cursors = MultiCursor::new(Selection::new(start_pos, sel_end));
    assert!(!app.cursors.primary().is_empty());

    // Focus pane 1 again
    assert!(app.focus_group(1));
    assert_eq!(app.ui.split.active_pane, 1);
    // Pane 1's cursor is at char 11 and its selection is empty
    assert_eq!(app.cursors.primary().head.char, 11);
    assert!(app.cursors.primary().is_empty());
}

#[test]
fn test_cloned_views_independent_scroll() {
    let mut app = App::new(fixture_font());
    app.insert_at_cursor("test content");

    assert!(app.clone_view_to_group(Some(1)));
    assert_eq!(app.ui.split.active_pane, 1);

    // Pane 1 initial scroll
    app.viewport.scroll_y = 100.0;
    app.ui.split.panes[1].viewport.scroll_y = 100.0;

    // Pane 0 initial scroll
    app.ui.split.panes[0].viewport.scroll_y = 20.0;

    // Set mouse position over pane 0 (left column) and scroll
    app.mouse.position.x = 50.0;
    app.mouse.position.y = 50.0;
    app.scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 });

    // Pane 0 scrolled by lines (positive change)
    assert!(app.ui.split.panes[0].viewport.scroll_y > 20.0);
    // Active pane 1'y scroll is unchanged at 100.0
    assert_eq!(app.viewport.scroll_y, 100.0);
    assert_eq!(app.ui.split.panes[1].viewport.scroll_y, 100.0);
}

#[test]
fn test_cloned_views_realtime_edit_and_cursor_shift() {
    let mut app = App::new(fixture_font());
    app.insert_at_cursor("abcdef");

    assert!(app.clone_view_to_group(Some(1)));
    assert_eq!(app.ui.split.active_pane, 1);

    // In pane 1, position cursor at offset 4 ('e')
    let pos4 = Position::from_char(&app.buffer, 4);
    app.cursors = MultiCursor::new(Selection::cursor(pos4));

    // Focus pane 0
    assert!(app.focus_group(0));
    assert_eq!(app.ui.split.active_pane, 0);

    // In pane 0, cursor is at offset 0
    let pos0 = Position::from_char(&app.buffer, 0);
    app.cursors = MultiCursor::new(Selection::cursor(pos0));

    // Insert "123" at offset 0 in pane 0
    app.insert_at_cursor("123");
    assert_eq!(app.buffer.text(), "123abcdef");

    // Inactive pane 1's cursor should have shifted from offset 4 to 4 + 3 = 7
    let pane1_cursor = app.ui.split.panes[1].cursors.primary();
    assert_eq!(pane1_cursor.head.char, 7);

    // Delete "123" in pane 0 (select offset 0..3 and delete)
    let pos3 = Position::from_char(&app.buffer, 3);
    app.cursors = MultiCursor::new(Selection::new(pos0, pos3));
    app.delete_at_cursors(false);
    assert_eq!(app.buffer.text(), "abcdef");

    // Inactive pane 1's cursor should have shifted back from 7 to 4
    let pane1_cursor_after = app.ui.split.panes[1].cursors.primary();
    assert_eq!(pane1_cursor_after.head.char, 4);
}

#[test]
fn test_cloned_views_undo_redo_sync() {
    let mut app = App::new(fixture_font());
    let ctx = DummyContext;
    app.insert_at_cursor("initial");

    assert!(app.clone_view_to_group(Some(1)));

    // In pane 1, cursor at end (7)
    assert_eq!(app.cursors.primary().head.char, 7);

    // Insert more text in pane 1
    app.insert_at_cursor(" + added");
    assert_eq!(app.buffer.text(), "initial + added");

    // Switch to pane 0
    assert!(app.focus_group(0));
    assert_eq!(app.buffer.text(), "initial + added");

    // Undo in pane 0
    crate::palette::commands::execute_command(&mut app, &ctx, "edit.undo");
    assert_eq!(app.buffer.text(), "initial");

    // Redo in pane 0
    crate::palette::commands::execute_command(&mut app, &ctx, "edit.redo");
    assert_eq!(app.buffer.text(), "initial + added");
}

#[test]
fn test_new_view_into_file_shares_buffer_id() {
    let mut app = App::new(fixture_font());
    app.insert_at_cursor("shared buffer content");

    let initial_tab = app.documents.active_tab.unwrap();
    let initial_bid = app.documents.active_buffer_id().unwrap();

    assert!(app.new_view_into_file());
    assert_eq!(app.documents.tabs.tabs().len(), 2);

    let new_tab = app.documents.active_tab.unwrap();
    assert_ne!(initial_tab, new_tab);

    // Both tabs share the same buffer_id
    assert_eq!(
        app.documents.buffer_id_for_tab(initial_tab),
        Some(initial_bid)
    );
    assert_eq!(app.documents.buffer_id_for_tab(new_tab), Some(initial_bid));

    // Edit in new view
    app.insert_at_cursor(" [edited in view 2]");
    app.sync_active_document_to_entry();

    // Verify first tab entry received the sync
    let entry0 = app
        .documents
        .entries
        .iter()
        .find(|e| e.tab_id == initial_tab)
        .unwrap();
    assert_eq!(
        entry0.buffer.text(),
        "shared buffer content [edited in view 2]"
    );
}

#[test]
fn test_palette_and_dispatcher_clone_view_commands() {
    let mut app = App::new(fixture_font());
    let ctx = DummyContext;
    app.insert_at_cursor("palette clone test");

    // Execute via command palette
    crate::palette::commands::execute_command(&mut app, &ctx, "file.clone_view_to_group");
    assert_eq!(app.ui.split.preset, LayoutPreset::Columns2);
    assert_eq!(app.ui.split.active_pane, 1);

    // Execute new view into file via command palette
    crate::palette::commands::execute_command(&mut app, &ctx, "file.new_view_into_file");
    assert_eq!(app.documents.tabs.tabs().len(), 2);
}
