//! "About Lux" modal dialog state, key/mouse interactions, and tests (ST-FIX-02).

use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey, PlatformContext};

use crate::app::App;

/// State for the floating "About Lux" modal dialog.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AboutDialogState {
    pub(crate) version: &'static str,
    pub(crate) close_hovered: bool,
    pub(crate) card_rect: [f32; 4],
    pub(crate) close_btn_rect: [f32; 4],
}

impl Default for AboutDialogState {
    fn default() -> Self {
        Self::new()
    }
}

impl AboutDialogState {
    pub(crate) fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            close_hovered: false,
            card_rect: [0.0, 0.0, 0.0, 0.0],
            close_btn_rect: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Handles keyboard events while the About modal is open.
pub(crate) fn handle_about_key(
    app: &mut App,
    _ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    if app.ui.dialogs.about.is_none() {
        return false;
    }
    if event.state == ElementState::Pressed
        && matches!(
            &event.key,
            Key::Named(NamedKey::Escape) | Key::Named(NamedKey::Enter)
        )
    {
        app.ui.dialogs.about = None;
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
    // Consume all keyboard events while the modal is active to prevent
    // modifying the document underneath.
    true
}

/// Handles left mouse clicks for the About modal.
///
/// Clicking the Close button or clicking outside the card dismisses the dialog.
/// Clicking inside the card keeps the dialog open and absorbs the event.
pub(crate) fn click_about(app: &mut App, mouse_x: f32, mouse_y: f32) -> bool {
    let Some(about) = app.ui.dialogs.about.as_ref() else {
        return false;
    };

    let [bx, by, bw, bh] = about.close_btn_rect;
    let [cx, cy, cw, ch] = about.card_rect;

    let inside_btn = mouse_x >= bx && mouse_x <= bx + bw && mouse_y >= by && mouse_y <= by + bh;
    let inside_card = mouse_x >= cx && mouse_x <= cx + cw && mouse_y >= cy && mouse_y <= cy + ch;

    if inside_btn || !inside_card {
        app.ui.dialogs.about = None;
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
    true
}

/// Updates hover state for the About dialog Close button.
pub(crate) fn hover_about(app: &mut App, mouse_x: f32, mouse_y: f32) -> bool {
    let Some(about) = app.ui.dialogs.about.as_mut() else {
        return false;
    };

    let [bx, by, bw, bh] = about.close_btn_rect;
    let inside_btn = mouse_x >= bx && mouse_x <= bx + bw && mouse_y >= by && mouse_y <= by + bh;

    if about.close_hovered != inside_btn {
        about.close_hovered = inside_btn;
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec())
    }

    #[test]
    fn test_about_initial_state() {
        let state = AboutDialogState::new();
        assert_eq!(state.version, env!("CARGO_PKG_VERSION"));
        assert!(!state.close_hovered);
    }

    #[test]
    fn test_about_key_dismiss() {
        let mut app = test_app();
        app.ui.dialogs.about = Some(AboutDialogState::new());
        assert!(app.ui.dialogs.about.is_some());

        struct DummyCtx;
        impl PlatformContext for DummyCtx {
            fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
                std::sync::Arc::new(|| {})
            }
            fn background_dialogs(&self) -> std::sync::Arc<dyn lux_platform::FileDialogs> {
                struct DummyDialogs;
                impl lux_platform::FileDialogs for DummyDialogs {
                    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
                        lux_platform::dialog::SaveDecision::Cancel
                    }
                    fn pick_file(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Option<std::path::PathBuf> {
                        None
                    }
                    fn pick_files(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Vec<std::path::PathBuf> {
                        Vec::new()
                    }
                    fn pick_folder(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Option<std::path::PathBuf> {
                        None
                    }
                    fn save_file(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Option<std::path::PathBuf> {
                        None
                    }
                }
                std::sync::Arc::new(DummyDialogs)
            }
            fn create_window(
                &self,
                _: lux_platform::WindowAttributes,
            ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError>
            {
                unimplemented!()
            }
            fn clipboard(&self) -> &dyn lux_platform::Clipboard {
                struct DummyClipboard;
                impl lux_platform::Clipboard for DummyClipboard {
                    fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
                        Ok(String::new())
                    }
                    fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
                        Ok(())
                    }
                }
                &DummyClipboard
            }
            fn file_dialogs(&self) -> &dyn lux_platform::FileDialogs {
                struct DummyDialogs;
                impl lux_platform::FileDialogs for DummyDialogs {
                    fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
                        lux_platform::dialog::SaveDecision::Cancel
                    }
                    fn pick_file(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Option<std::path::PathBuf> {
                        None
                    }
                    fn pick_files(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Vec<std::path::PathBuf> {
                        Vec::new()
                    }
                    fn pick_folder(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Option<std::path::PathBuf> {
                        None
                    }
                    fn save_file(
                        &self,
                        _: &lux_platform::FileDialogOptions,
                    ) -> Option<std::path::PathBuf> {
                        None
                    }
                }
                &DummyDialogs
            }
            fn exit(&self) {}
            fn request_wakeup(&self, _: std::time::Instant) {}
        }
        let ctx = DummyCtx;

        let esc_event = KeyEvent {
            state: ElementState::Pressed,
            key: Key::Named(NamedKey::Escape),
            text: None,
            repeat: false,
        };

        let handled = handle_about_key(&mut app, &ctx, &esc_event);
        assert!(handled);
        assert!(app.ui.dialogs.about.is_none());
    }

    #[test]
    fn test_about_click_hit_testing() {
        let mut app = test_app();
        let mut about = AboutDialogState::new();
        about.card_rect = [100.0, 100.0, 400.0, 240.0];
        about.close_btn_rect = [250.0, 300.0, 100.0, 30.0];
        app.ui.dialogs.about = Some(about);

        // Click inside card (not button) -> remains open
        let handled = click_about(&mut app, 150.0, 150.0);
        assert!(handled);
        assert!(app.ui.dialogs.about.is_some());

        // Click close button -> dismissed
        let handled = click_about(&mut app, 260.0, 310.0);
        assert!(handled);
        assert!(app.ui.dialogs.about.is_none());

        // Re-open and click outside card -> dismissed
        let mut about = AboutDialogState::new();
        about.card_rect = [100.0, 100.0, 400.0, 240.0];
        about.close_btn_rect = [250.0, 300.0, 100.0, 30.0];
        app.ui.dialogs.about = Some(about);

        let handled = click_about(&mut app, 10.0, 10.0);
        assert!(handled);
        assert!(app.ui.dialogs.about.is_none());
    }

    #[test]
    fn test_about_hover() {
        let mut app = test_app();
        let mut about = AboutDialogState::new();
        about.close_btn_rect = [250.0, 300.0, 100.0, 30.0];
        app.ui.dialogs.about = Some(about);

        // Hover over button
        let handled = hover_about(&mut app, 260.0, 310.0);
        assert!(handled);
        assert!(app.ui.dialogs.about.as_ref().unwrap().close_hovered);

        // Hover outside button
        let handled = hover_about(&mut app, 100.0, 100.0);
        assert!(handled);
        assert!(!app.ui.dialogs.about.as_ref().unwrap().close_hovered);
    }
}
