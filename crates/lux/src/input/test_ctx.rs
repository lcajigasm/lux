//! A headless `PlatformContext` for tests that drive the real top-level
//! keyboard path (`App::handle_keyboard_event`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use lux_platform::event::KeyEvent;
use lux_platform::{
    Clipboard, ElementState, FileDialogs, Key, Modifiers, PlatformContext, PlatformError,
};

use crate::app::App;

pub(crate) struct NoDialogs;

impl FileDialogs for NoDialogs {
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

static NO_DIALOGS: NoDialogs = NoDialogs;

/// Holds an in-memory clipboard; everything else is inert.
#[derive(Default)]
pub(crate) struct TestCtx(RefCell<String>);

impl Clipboard for TestCtx {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(self.0.borrow().clone())
    }
    fn write_text(&self, text: &str) -> Result<(), PlatformError> {
        *self.0.borrow_mut() = text.to_owned();
        Ok(())
    }
}

impl PlatformContext for TestCtx {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        Arc::new(NoDialogs)
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        &NO_DIALOGS
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless test context".into()))
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
    fn exit(&self) {}
}

pub(crate) fn app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = lux_core::buffer::Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

/// Presses `key` with `modifiers` through the window's keyboard handler.
pub(crate) fn press(app: &mut App, ctx: &TestCtx, key: Key, modifiers: Modifiers) {
    crate::menu::clear_claim();
    app.modifiers = modifiers;
    let text = match &key {
        Key::Character(text) => Some(text.clone()),
        _ => None,
    };
    app.handle_keyboard_event(
        ctx,
        &KeyEvent {
            key,
            text,
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
    );
    app.modifiers = Modifiers::default();
}

/// Cmd on macOS, Ctrl elsewhere.
pub(crate) fn primary(shift: bool) -> Modifiers {
    Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        ..Default::default()
    }
}

pub(crate) fn ctrl() -> Modifiers {
    Modifiers {
        control: true,
        ..Default::default()
    }
}

pub(crate) fn chr(text: &str) -> Key {
    Key::Character(text.into())
}
