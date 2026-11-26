//! Settings importers from other editors (VS Code, Sublime Text).
//!
//! The source file is read and mapped on a worker, and the mapped values are
//! written into the user settings file by the same serial worker that
//! serves the settings page. The UI thread only starts the job and shows the
//! report when it lands.

use std::fs;
use std::path::PathBuf;

use lux_config::import::ImportReport;
use lux_config::layering::{Layer, LayeredConfig};
use lux_platform::PlatformContext;

use crate::app::App;
use crate::ui_io::Done;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    VsCode,
    Sublime,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::VsCode => "VS Code",
            Source::Sublime => "Sublime Text",
        }
    }

    fn map(self, content: &str) -> ImportReport {
        let report = match self {
            Source::VsCode => lux_config::import::vscode::import(content),
            Source::Sublime => lux_config::import::sublime::import(content),
        };
        report.unwrap_or_default()
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// Where `source` keeps its user settings on this platform.
pub(crate) fn source_path(source: Source) -> PathBuf {
    let home = home();
    #[cfg(target_os = "windows")]
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or(home.clone());
    match source {
        #[cfg(target_os = "macos")]
        Source::VsCode => home.join("Library/Application Support/Code/User/settings.json"),
        #[cfg(target_os = "windows")]
        Source::VsCode => appdata.join("Code/User/settings.json"),
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        Source::VsCode => home.join(".config/Code/User/settings.json"),
        #[cfg(target_os = "macos")]
        Source::Sublime => home.join(
            "Library/Application Support/Sublime Text/Packages/User/Preferences.sublime-settings",
        ),
        #[cfg(target_os = "windows")]
        Source::Sublime => appdata.join("Sublime Text/Packages/User/Preferences.sublime-settings"),
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        Source::Sublime => {
            home.join(".config/sublime-text/Packages/User/Preferences.sublime-settings")
        }
    }
}

/// Imports `source`'s user settings from its usual location.
pub(crate) fn import(app: &mut App, ctx: &dyn PlatformContext, source: Source) {
    import_from(app, Some(ctx), source, source_path(source));
}

/// Imports from `path`. A missing or unreadable file imports nothing and
/// says so: no stand-in values are ever written.
pub(crate) fn import_from(
    app: &mut App,
    ctx: Option<&dyn PlatformContext>,
    source: Source,
    path: PathBuf,
) {
    let settings_path = app.ui.settings.settings_path.clone();
    let config = app.ui.settings.config.clone();
    crate::ui_io::spawn_serial(app, ctx, move || {
        let label = source.label();
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                return Some(Done::Imported(format!(
                    "No {label} settings imported: cannot read {}: {error}",
                    path.display()
                )));
            }
        };
        let report = source.map(&content);
        let pairs = report
            .imported
            .iter()
            .map(|item| (item.lux_key.to_string(), item.value.clone()))
            .collect::<Vec<_>>();
        if !pairs.is_empty() {
            super::write_user_values(&settings_path, pairs);
            let _ = LayeredConfig::reload_layer_shared(&config, Layer::User);
        }
        Some(Done::Imported(format!(
            "Imported {} settings from {label} ({} ignored)",
            report.imported.len(),
            report.ignored.len()
        )))
    });
}
