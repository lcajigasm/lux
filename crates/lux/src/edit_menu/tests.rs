use super::*;
use crate::app::App;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::MouseButton;
use lux_platform::dialog::{FileDialogs, SaveDecision};
use lux_platform::{Clipboard, FileDialogOptions, PlatformContext, PlatformError};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

fn app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = lux_core::buffer::Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

struct Clip(RefCell<String>);

impl Clipboard for Clip {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(self.0.borrow().clone())
    }
    fn write_text(&self, text: &str) -> Result<(), PlatformError> {
        *self.0.borrow_mut() = text.to_owned();
        Ok(())
    }
}

struct Dialogs;

impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
}

struct Ctx {
    clip: Clip,
}

impl PlatformContext for Ctx {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(Dialogs)
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, PlatformError> {
        panic!("unused")
    }
    fn clipboard(&self) -> &dyn Clipboard {
        &self.clip
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &Dialogs
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn ctx() -> Ctx {
    Ctx {
        clip: Clip(RefCell::new(String::new())),
    }
}

fn click(app: &mut App, ctx: &Ctx, button: MouseButton, x: f32, y: f32) {
    app.mouse.position = dpi::PhysicalPosition::new(x as f64, y as f64);
    app.mouse.placed = true;
    if !take_menu_click(app, ctx, button) {
        press_editor(app, ctx, button);
    }
}

fn row(menu: &BufferContextMenu, action: EditMenuAction) -> EditMenuRow {
    menu.rows
        .iter()
        .copied()
        .find(|row| matches!(row, EditMenuRow::Action { action: found, .. } if *found == action))
        .unwrap_or_else(|| panic!("missing {}", action.label()))
}

#[test]
fn right_click_and_ctrl_click_open_the_menu_and_move_the_caret() {
    let mut app = app("alpha beta gamma");
    app.cursors = lux_core::multicursor::MultiCursor::new(Selection::new(
        Position::from_char(&app.buffer, 0),
        Position::from_char(&app.buffer, 5),
    ));
    let host = ctx();
    click(&mut app, &host, MouseButton::Right, 220.0, 48.0);
    let menu = app
        .mouse
        .context_menu
        .clone()
        .expect("right click opens the menu");
    assert!(menu.rows.iter().any(|row| matches!(
        row,
        EditMenuRow::Action {
            action: EditMenuAction::GotoDefinition,
            ..
        }
    )));
    assert!(menu.rows.iter().any(|row| matches!(
        row,
        EditMenuRow::Action {
            action: EditMenuAction::StageHunk,
            enabled: false,
        }
    )));
    assert!(
        menu.rows.iter().all(|row| match row {
            EditMenuRow::Action { action, .. } => action.label() != "Revert Hunk",
            EditMenuRow::Separator => true,
        }),
        "no revert-hunk command exists"
    );
    assert!(matches!(
        row(&menu, EditMenuAction::CommandPalette),
        EditMenuRow::Action { enabled: true, .. }
    ));
    assert!(app.cursors.primary().is_empty() || app.cursors.primary().head.char > 5);
    app.mouse.context_menu = None;
    app.modifiers.control = true;
    click(&mut app, &host, MouseButton::Left, 220.0, 48.0);
    assert!(
        app.mouse.context_menu.is_some(),
        "ctrl+click opens the menu"
    );
}

#[test]
fn command_click_does_not_open_the_menu() {
    let mut app = app("alpha beta");
    let host = ctx();
    app.modifiers.meta = true;
    app.modifiers.control = true;
    click(&mut app, &host, MouseButton::Left, 40.0, 40.0);
    assert!(app.mouse.context_menu.is_none());
}

#[test]
fn right_click_inside_a_selection_keeps_it_and_cut_is_enabled() {
    let mut app = app("alpha beta gamma");
    let selection = Selection::new(
        Position::from_char(&app.buffer, 0),
        Position::from_char(&app.buffer, 5),
    );
    app.cursors = lux_core::multicursor::MultiCursor::new(selection);
    let host = ctx();
    click(&mut app, &host, MouseButton::Right, 40.0, 40.0);
    assert_eq!(app.cursors.primary().char_range(), 0..5);
    let menu = app.mouse.context_menu.as_ref().unwrap();
    assert!(matches!(
        row(menu, EditMenuAction::Cut),
        EditMenuRow::Action { enabled: true, .. }
    ));
    assert!(matches!(
        row(menu, EditMenuAction::Copy),
        EditMenuRow::Action { enabled: true, .. }
    ));
}

#[test]
fn cut_and_copy_are_disabled_without_a_selection() {
    let mut app = app("alpha");
    let host = ctx();
    click(&mut app, &host, MouseButton::Right, 80.0, 40.0);
    let menu = app.mouse.context_menu.as_ref().unwrap();
    assert!(matches!(
        row(menu, EditMenuAction::Cut),
        EditMenuRow::Action { enabled: false, .. }
    ));
    assert!(matches!(
        row(menu, EditMenuAction::Copy),
        EditMenuRow::Action { enabled: false, .. }
    ));
    let rects = menu.row_rects(1.0);
    let cut = rects
        .iter()
        .zip(menu.rows.iter())
        .find(|(_, row)| {
            matches!(
                row,
                EditMenuRow::Action {
                    action: EditMenuAction::Cut,
                    ..
                }
            )
        })
        .unwrap()
        .0;
    click(
        &mut app,
        &host,
        MouseButton::Left,
        cut[0] + 8.0,
        cut[1] + 4.0,
    );
    assert!(app.mouse.context_menu.is_some(), "a disabled row stays put");
    assert_eq!(app.buffer.text(), "alpha");
}

#[test]
fn double_click_selects_the_word_and_a_drag_extends_by_words() {
    let mut app = app("alpha beta gamma");
    let host = ctx();
    click(&mut app, &host, MouseButton::Left, 48.0, 40.0);
    click(&mut app, &host, MouseButton::Left, 48.0, 40.0);
    let word = app.cursors.primary().char_range();
    assert!(word.end > word.start, "double click selects a run");
    let selected = &app.buffer.text()[word.clone()];
    assert!(!selected.contains(' '), "{selected}");
    app.mouse.position = dpi::PhysicalPosition::new(180.0, 40.0);
    app.update_mouse_selection();
    let dragged = app.cursors.primary().char_range();
    assert!(dragged.end >= word.end);
    assert!(dragged.start <= word.start);
    let text = &app.buffer.text()[dragged];
    assert!(
        text.contains(' ') || text.len() > selected.len(),
        "drag should reach another word, got {text}"
    );
}

#[test]
fn command_palette_row_opens_the_palette() {
    let mut app = app("alpha");
    let host = ctx();
    click(&mut app, &host, MouseButton::Right, 80.0, 40.0);
    let menu = app.mouse.context_menu.clone().unwrap();
    let rects = menu.row_rects(app.scale_factor);
    let palette = rects
        .iter()
        .zip(menu.rows.iter())
        .find(|(_, row)| {
            matches!(
                row,
                EditMenuRow::Action {
                    action: EditMenuAction::CommandPalette,
                    ..
                }
            )
        })
        .unwrap()
        .0;
    click(
        &mut app,
        &host,
        MouseButton::Left,
        palette[0] + 12.0,
        palette[1] + 6.0,
    );
    assert!(app.ui.command_palette.is_some());
    assert!(app.mouse.context_menu.is_none());
}
