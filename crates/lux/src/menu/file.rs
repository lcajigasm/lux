//! File menu actions that are not document requests.

use lux_platform::PlatformContext;

use super::recent;
use crate::app::App;

pub(crate) fn open_recent(app: &mut App, ctx: &dyn PlatformContext, index: usize) {
    let Some(path) = recent::path_at(index) else {
        return;
    };
    app.request_document(ctx, crate::document::Request::Open(Some(path)));
}

pub(crate) fn open_new_window(app: &mut App) {
    if let Err(error) = spawn_new_window() {
        app.documents.message = Some(format!("Cannot open a new window: {error}"));
    }
}

pub(crate) fn new_window_command() -> Result<std::process::Command, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut command = std::process::Command::new(exe);
    command
        .arg("--new-window")
        .env_remove("LUX_SOCKET")
        .stdin(std::process::Stdio::null());
    Ok(command)
}

fn spawn_new_window() -> Result<(), String> {
    // The test binary is not a second editor. The real binary spawns it.
    #[cfg(test)]
    {
        let command = new_window_command()?;
        let _ = command.get_program();
        Ok(())
    }
    #[cfg(not(test))]
    {
        let mut command = new_window_command()?;
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use lux_platform::dialog::SaveDecision;
    use lux_platform::{FileDialogOptions, FileDialogs, PlatformContext};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct Ctx;

    impl PlatformContext for Ctx {
        fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
            Arc::new(|| {})
        }
        fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
            Arc::new(Ctx)
        }
        fn create_window(
            &self,
            _: lux_platform::WindowAttributes,
        ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
            Err(lux_platform::PlatformError::Window("headless".into()))
        }
        fn clipboard(&self) -> &dyn lux_platform::Clipboard {
            self
        }
        fn file_dialogs(&self) -> &dyn FileDialogs {
            self
        }
        fn exit(&self) {}
        fn request_wakeup(&self, _: std::time::Instant) {}
    }

    impl lux_platform::Clipboard for Ctx {
        fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
            Ok(String::new())
        }
        fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
            Ok(())
        }
    }

    impl FileDialogs for Ctx {
        fn confirm_save(&self, _: &str) -> SaveDecision {
            SaveDecision::Cancel
        }
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
    }

    #[test]
    fn new_window_command_clears_the_socket_and_passes_the_flag() {
        let command = new_window_command().unwrap();
        let args: Vec<_> = command.get_args().map(|arg| arg.to_owned()).collect();
        assert_eq!(args, vec![std::ffi::OsString::from("--new-window")]);
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "LUX_SOCKET" && value.is_none())
        );
    }

    #[test]
    fn file_menu_new_window_and_clear_recent_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("recent.json");
        let kept = dir.path().join("kept.txt");
        let _recent = super::super::recent::begin_test(&store);
        super::super::recent::note(Some(&kept));
        super::super::recent::sync();
        assert_eq!(super::super::recent::entries(), vec![kept]);

        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        let ctx = Ctx;
        super::super::chords::clear_claim();
        let before = app.documents.message.clone();
        super::super::handle_menu_action(&mut app, &ctx, super::super::MenuAction::NewWindow);
        assert_eq!(app.documents.message, before);
        super::super::chords::clear_claim();
        super::super::handle_menu_action(&mut app, &ctx, super::super::MenuAction::ClearRecent);
        super::super::recent::sync();
        assert!(super::super::recent::entries().is_empty());
    }
}
