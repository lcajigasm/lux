//! The Dock icon. `corner` is the default mark. `classic` is the previous
//! icon, still shipped beside it. On macOS the choice calls
//! `NSApplication.setApplicationIconImage`, so it shows without a restart.

use std::cell::Cell;

pub(crate) const CORNER: &str = "corner";
pub(crate) const CLASSIC: &str = "classic";

thread_local! {
    static REQUESTED: Cell<&'static str> = const { Cell::new(CORNER) };
    static DOCK_SET: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn canonical(value: Option<&str>) -> &'static str {
    if value == Some(CLASSIC) {
        CLASSIC
    } else {
        CORNER
    }
}

pub(crate) fn requested() -> &'static str {
    REQUESTED.with(Cell::get)
}

/// The token written by `--probe`. On macOS it stays `missing` until
/// `setApplicationIconImage` has run for the current choice.
pub(crate) fn probe_token() -> &'static str {
    let applied = DOCK_SET.with(Cell::get);
    #[cfg(target_os = "macos")]
    {
        if applied { requested() } else { "missing" }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = applied;
        requested()
    }
}

pub(crate) fn apply(value: Option<&str>) {
    let name = canonical(value);
    REQUESTED.with(|cell| cell.set(name));
    DOCK_SET.with(|cell| cell.set(set_dock(name)));
}

#[cfg(target_os = "macos")]
fn file_name(name: &str) -> &'static str {
    if name == CLASSIC {
        "lux-classic.icns"
    } else {
        "lux.icns"
    }
}

fn set_dock(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::set(name)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = name;
        false
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::file_name;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{NSBundle, NSString};
    use std::path::PathBuf;

    pub(super) fn set(name: &str) -> bool {
        let Some(path) = icon_file(name) else {
            eprintln!("Lux: app icon {name} was not in the bundle");
            return false;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            eprintln!("Lux: app icon {name} needs the main thread");
            return false;
        };
        let text = path.to_string_lossy().into_owned();
        let ns_path = NSString::from_str(&text);
        let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &ns_path) else {
            eprintln!("Lux: app icon {} could not be read", path.display());
            return false;
        };
        let app = NSApplication::sharedApplication(mtm);
        // `None` would clear a custom image. This call always passes one.
        unsafe { app.setApplicationIconImage(Some(&image)) };
        true
    }

    fn icon_file(name: &str) -> Option<PathBuf> {
        let file = file_name(name);
        if let Some(path) = bundle_resource(file) {
            return Some(path);
        }
        let exe = std::env::current_exe().ok()?;
        let resource = exe.parent()?.parent()?.join("Resources").join(file);
        resource.is_file().then_some(resource)
    }

    fn bundle_resource(file: &str) -> Option<PathBuf> {
        let (stem, ext) = file.rsplit_once('.')?;
        let bundle = NSBundle::mainBundle();
        let path = bundle.pathForResource_ofType(
            Some(&NSString::from_str(stem)),
            Some(&NSString::from_str(ext)),
        )?;
        let path = PathBuf::from(path.to_string());
        path.is_file().then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::settings::SettingsState;
    use serde_json::json;

    fn new_app() -> App {
        App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec())
    }

    #[test]
    fn the_icon_setting_switches_and_persists() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let mut app = new_app();
        app.ui.settings = SettingsState::new(path.clone());
        app.apply_user_settings();
        assert_eq!(requested(), CORNER);
        let created = std::fs::read_to_string(&path).unwrap();
        assert!(
            created.contains("\"workbench.icon\": \"corner\""),
            "{created}"
        );

        let mut settings = std::mem::take(&mut app.ui.settings);
        settings.update_setting_value(&mut app, "workbench.icon", json!(CLASSIC));
        app.ui.settings = settings;
        assert_eq!(requested(), CLASSIC);
        if !cfg!(target_os = "macos") {
            assert_eq!(probe_token(), CLASSIC);
        }
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("\"workbench.icon\": \"classic\""), "{saved}");

        let mut next = new_app();
        SettingsState::new(path).apply_to_app(&mut next);
        assert_eq!(requested(), CLASSIC);

        apply(Some("gold"));
        assert_eq!(requested(), CORNER);
        assert!(!lux_config::validate(r#"{"workbench.icon": "gold"}"#).is_empty());
        assert!(lux_config::validate(r#"{"workbench.icon": "corner"}"#).is_empty());
    }
}
