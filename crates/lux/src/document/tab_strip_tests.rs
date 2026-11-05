use std::cell::Cell;
use std::sync::Arc;

use lux_platform::dialog::SaveDecision;
use lux_platform::{
    Clipboard, FileDialogOptions, FileDialogs, MouseButton, PlatformContext, PlatformError,
    PlatformWindow, WindowAttributes,
};

use crate::app::App;
use crate::chrome::{drag_tab, scroll_tabs, tab_at};

struct Dialogs;
impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<std::path::PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
}

struct Ctx {
    dialogs: Arc<Dialogs>,
    exited: Cell<bool>,
}
impl Ctx {
    fn new() -> Self {
        Self {
            dialogs: Arc::new(Dialogs),
            exited: Cell::new(false),
        }
    }
}
impl Clipboard for Ctx {
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
        self.dialogs.clone()
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless".into()))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        self.dialogs.as_ref()
    }
    fn exit(&self) {
        self.exited.set(true);
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

#[test]
fn middle_click_closes_the_inactive_tab() {
    let mut app = app();
    let second = app.documents.tabs.open("second");
    let width = app.tab_width();
    app.mouse.position.y = 8.0;
    app.mouse.position.x = (width * 1.5) as f64;
    assert!(app.handle_mouse_click_on_chrome(&Ctx::new(), MouseButton::Middle));
    assert!(app.documents.tabs.tabs().iter().all(|tab| tab.id != second));
}

#[test]
fn overflow_scroll_moves_which_tab_the_pointer_hits() {
    let mut app = app();
    for index in 0..12 {
        app.documents.tabs.open(format!("f{index}.txt"));
    }
    let width = app.tab_width();
    assert!(width * app.documents.tabs.tabs().len() as f32 > app.viewport.width);
    let before = tab_at(&app, width * 0.5).unwrap().0;
    scroll_tabs(&mut app, -width * 2.0);
    let after = tab_at(&app, width * 0.5).unwrap().0;
    assert!(after > before);
    assert!(app.documents.tabs.scroll() > 0.0);
}

#[test]
fn dragging_a_tab_reorders_the_strip() {
    let mut app = app();
    app.documents.tabs.open("right");
    let left = app.documents.tabs.tabs()[0].id;
    let width = app.tab_width();
    app.documents.tab_drag = Some((left, width * 0.5));
    app.mouse.position.x = (width * 1.5) as f64;
    assert!(drag_tab(&mut app));
    assert_eq!(app.documents.tabs.tabs()[1].id, left);
}
