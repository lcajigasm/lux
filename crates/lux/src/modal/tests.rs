use std::path::PathBuf;
use std::sync::Arc;

use lux_platform::{FileDialogs, PlatformContext};

use super::*;
use crate::app::App;
use crate::goto::{GotoItem, GotoItemKind};
use crate::snippets::SnippetPickerState;
use crate::snippets::registry::Snippet;

struct DummyDialogs;
impl FileDialogs for DummyDialogs {
    fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<PathBuf> {
        vec![]
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

fn create_test_app() -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.viewport.width = 1000.0;
    app.viewport.height = 800.0;
    app
}

#[test]
fn test_no_modal_open_returns_false() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    assert!(!is_any_modal_open(&app));
    assert!(!handle_modal_click(&mut app, &ctx, 500.0, 200.0));
    assert!(!handle_modal_hover(&mut app, 500.0, 200.0));
}

#[test]
fn test_palette_outside_click_dismisses_and_consumes() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    app.toggle_command_palette();
    assert!(app.ui.command_palette.is_some());

    // Click outside top-left (10, 10)
    let consumed = handle_modal_click(&mut app, &ctx, 10.0, 10.0);
    assert!(consumed, "outside click must be consumed");
    assert!(app.ui.command_palette.is_none(), "modal must be dismissed");
}

#[test]
fn test_palette_row_click_and_hover() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    app.toggle_command_palette();
    assert!(app.ui.command_palette.is_some());

    // Rows top = 32 + 52 + 46 + 2 = 132. Row height = 36.
    // Row 0 center = 150. Row 1 center = 186.
    let hovered = handle_modal_hover(&mut app, 500.0, 186.0);
    assert!(hovered);
    assert_eq!(app.ui.command_palette.as_ref().unwrap().selected_index, 1);

    // Test click on Row 1
    let clicked = handle_modal_click(&mut app, &ctx, 500.0, 186.0);
    assert!(clicked);
    assert!(app.ui.command_palette.is_none());
}

#[test]
fn test_goto_anything_outside_click_restores_snapshot() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    let tab1 = app.documents.active_tab.unwrap();
    let tab2 = app.documents.tabs.open("Second Tab");
    let buf = lux_core::buffer::Buffer::from_bytes(b"second tab").unwrap();
    app.documents
        .entries
        .push(crate::document::DocumentEntry::new(tab2, None, buf));

    assert_eq!(app.documents.active_tab, Some(tab1));

    app.open_goto_anything("");
    assert!(app.ui.goto_anything.is_some());

    // Switch tab during preview
    assert!(app.switch_to_tab(tab2));
    assert_eq!(app.documents.active_tab, Some(tab2));

    // Outside click should restore active_tab back to tab1
    let consumed = handle_modal_click(&mut app, &ctx, 10.0, 10.0);
    assert!(consumed);
    assert!(app.ui.goto_anything.is_none());
    assert_eq!(app.documents.active_tab, Some(tab1));
}

#[test]
fn test_goto_anything_row_click_and_hover() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    app.open_goto_anything("");
    if let Some(goto) = app.ui.goto_anything.as_mut() {
        goto.items = vec![
            GotoItem {
                label: "file_a.rs".into(),
                detail: "".into(),
                path: "file_a.rs".into(),
                line: Some(10),
                col: Some(5),
                kind: GotoItemKind::File {
                    line: Some(10),
                    col: Some(5),
                },
            },
            GotoItem {
                label: "file_b.rs".into(),
                detail: "".into(),
                path: "file_b.rs".into(),
                line: Some(20),
                col: Some(1),
                kind: GotoItemKind::File {
                    line: Some(20),
                    col: Some(1),
                },
            },
        ];
    }

    // Rows top = 132. Row 1 midpoint = 186.
    let hovered = handle_modal_hover(&mut app, 500.0, 186.0);
    assert!(hovered);
    assert_eq!(app.ui.goto_anything.as_ref().unwrap().selected_index, 1);

    let clicked = handle_modal_click(&mut app, &ctx, 500.0, 186.0);
    assert!(clicked);
    assert!(app.ui.goto_anything.is_none());
}

#[test]
fn test_theme_picker_click_and_hover() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    app.open_theme_picker();
    assert!(app.ui.theme_picker.is_some());

    // Modal top = 84. Input = 46. Rows top = 132. Row height = 36.
    // Row 1 midpoint = 186. Row 0 midpoint = 150.
    let hovered = handle_modal_hover(&mut app, 500.0, 186.0);
    assert!(hovered);
    assert_eq!(app.ui.theme_picker.as_ref().unwrap().selected_index, 1);

    // Outside click restores original theme
    let consumed = handle_modal_click(&mut app, &ctx, 10.0, 10.0);
    assert!(consumed);
    assert!(app.ui.theme_picker.is_none());

    // Reopen and click row 0 (midpoint 94.0) to confirm theme
    app.open_theme_picker();
    let clicked = handle_modal_click(&mut app, &ctx, 500.0, 150.0);
    assert!(clicked);
    assert!(app.ui.theme_picker.is_none());
}

#[test]
fn test_snippet_picker_click_and_outside() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    app.ui.snippets.picker = Some(SnippetPickerState::new(vec![
        Snippet {
            trigger: "fn".into(),
            name: "Function".into(),
            content: "fn test() {}".into(),
            description: "A test function".into(),
            scope: None,
        },
        Snippet {
            trigger: "st".into(),
            name: "Struct".into(),
            content: "struct Foo {}".into(),
            description: "A test struct".into(),
            scope: None,
        },
    ]));

    // Hover row 1
    let hovered = handle_modal_hover(&mut app, 500.0, 119.0);
    assert!(hovered);
    assert_eq!(app.ui.snippets.picker.as_ref().unwrap().selected_index, 1);

    // Click outside dismisses
    let consumed = handle_modal_click(&mut app, &ctx, 10.0, 10.0);
    assert!(consumed);
    assert!(app.ui.snippets.picker.is_none());

    // Reopen and click row 0
    app.ui.snippets.picker = Some(SnippetPickerState::new(vec![Snippet {
        trigger: "fn".into(),
        name: "Function".into(),
        content: "fn test() {}".into(),
        description: "A test function".into(),
        scope: None,
    }]));
    let clicked = handle_modal_click(&mut app, &ctx, 500.0, 93.0);
    assert!(clicked);
    assert!(app.ui.snippets.picker.is_none());
}

#[test]
fn test_build_picker_click_and_outside() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    app.ui.build.picker = Some(crate::build::BuildPickerState {
        variants: vec![crate::build::BuildVariant {
            name: "Test Build".into(),
            command: "cargo".into(),
            args: vec!["check".into()],
            file_regex: None,
        }],
        query: String::new(),
        selected: 0,
    });

    let consumed = handle_modal_click(&mut app, &ctx, 10.0, 10.0);
    assert!(consumed);
    assert!(app.ui.build.picker.is_none());
}

#[test]
fn test_project_picker_click_and_outside() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    app.ui.project.picker = Some(crate::project::ProjectSwitchState {
        projects: vec![std::path::PathBuf::from("/tmp/proj")],
        query: String::new(),
        selected: 0,
    });

    let consumed = handle_modal_click(&mut app, &ctx, 10.0, 10.0);
    assert!(consumed);
    assert!(app.ui.project.picker.is_none());
}

#[test]
fn test_project_search_field_click_and_outside() {
    let mut app = create_test_app();
    let ctx = DummyContext;
    crate::project_search::open_project_find(&mut app);
    assert!(app.ui.project_search.is_some());

    // Top = 32 + 24 = 56. Where field is at 56 + 38 = 94..118.
    let clicked_field = handle_modal_click(&mut app, &ctx, 500.0, 106.0);
    assert!(clicked_field);
    assert_eq!(
        app.ui.project_search.as_ref().unwrap().focused_field,
        crate::project_search::SearchField::Where
    );

    // Outside click dismisses
    let consumed = handle_modal_click(&mut app, &ctx, 10.0, 10.0);
    assert!(consumed);
    assert!(app.ui.project_search.is_none());
}
