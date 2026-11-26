//! Settings configuration, schema validation, autocompletion, hot-reloading,
//! preferences UI panel, and external settings importers (Goal G22).

mod catalog;
pub(crate) mod importers;
pub(crate) mod layout;
mod page_input;
mod panel;
mod wheel;

pub(crate) use page_input::{handle_click, handle_key};
pub(crate) use panel::{SettingKind, SettingsPanelState};
pub(crate) use wheel::scroll_settings;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lux_config::layering::watch::ReloadEvent;
use lux_config::layering::{ConfigWatcher, Layer, LayeredConfig};
use lux_lsp::async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, DiagnosticSeverity, Position, Range,
};
use lux_platform::PlatformContext;
use serde_json::Value;

use crate::app::App;
use crate::lsp::{CompletionPopupState, DiagnosticItem};

pub(crate) const DEFAULT_SETTINGS_JSON: &str = r#"{
  "$schema": "https://lux.dev/schema/settings.json",
  "editor.fontSize": 13.0,
  "editor.lineHeight": 1.4,
  "editor.fontLigatures": true,
  "editor.tabWidth": 4,
  "editor.indentSize": 4,
  "editor.indentStyle": "space",
  "editor.trimTrailingWhitespace": false,
  "editor.trimTrailingWhitespacePreserveCursor": false,
  "editor.insertFinalNewline": false,
  "editor.wordWrap": false,
  "editor.distractionFreeColumnWidth": 80,
  "workbench.colorTheme": "monokai",
  "workbench.keymap": "sublime",
  "workbench.icon": "corner"
}"#;

pub(crate) fn default_settings_dir() -> PathBuf {
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "windows")]
    let dir = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| base.join(".lux"));
    #[cfg(not(target_os = "windows"))]
    let dir = base.join(".config").join("lux");
    dir
}

pub(crate) fn default_settings_path() -> PathBuf {
    default_settings_dir().join("settings.json")
}

/// `editor.fontFamily` from the user settings file, read once before the
/// event loop so the first frame already uses that face (ADR 0020).
pub(crate) fn startup_font_family() -> Option<String> {
    let text = fs::read_to_string(default_settings_path()).ok()?;
    let value = lux_config::layering::jsonc::parse(&text).ok()?;
    value
        .get(lux_config::font_settings::FONT_FAMILY_KEY)?
        .as_str()
        .map(str::to_string)
}

pub(crate) struct SettingsState {
    pub(crate) config: Arc<Mutex<LayeredConfig>>,
    pub(crate) watcher: Option<ConfigWatcher>,
    pub(crate) settings_path: PathBuf,
    pub(crate) panel: Option<SettingsPanelState>,
    pub(crate) schemes: Vec<crate::color_scheme::LoadedScheme>,
    pub(crate) palette_uses: HashMap<String, u64>,
    pub(crate) palette_clock: u64,
}

#[cfg(test)]
thread_local! {
    /// How many times this thread built a `SettingsState`, which touches
    /// the disk. Tests assert that UI handlers never do.
    pub(crate) static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new(default_settings_path())
    }
}

impl SettingsState {
    pub(crate) fn new(settings_path: PathBuf) -> Self {
        #[cfg(test)]
        BUILT.with(|built| built.set(built.get() + 1));
        if let Some(parent) = settings_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if !settings_path.exists() {
            let _ = fs::write(&settings_path, DEFAULT_SETTINGS_JSON);
        }

        let layered = LayeredConfig::new(DEFAULT_SETTINGS_JSON)
            .expect("default settings JSON must parse correctly");
        let config = Arc::new(Mutex::new(layered));

        let watcher =
            ConfigWatcher::new(Arc::clone(&config), [(Layer::User, settings_path.clone())]).ok();

        let mut schemes = crate::color_scheme::bundled_schemes().to_vec();
        if let Some(dir) = settings_path.parent() {
            schemes.extend(crate::color_scheme::load_user_schemes(&dir.join("themes")));
        }

        Self {
            config,
            watcher,
            settings_path,
            panel: None,
            schemes,
            palette_uses: HashMap::new(),
            palette_clock: 0,
        }
    }

    /// Drains the settings-file watcher. True when it reloaded a layer.
    fn watcher_reloaded(&self) -> bool {
        let Some(watcher) = &self.watcher else {
            return false;
        };
        let mut reloaded = false;
        while let Some(event) = watcher.try_recv() {
            match event {
                ReloadEvent::Reloaded(_) => reloaded = true,
                ReloadEvent::Failed(_, _) => {}
            }
        }
        reloaded
    }

    #[cfg(test)]
    pub(crate) fn apply_to_app(&self, app: &mut App) {
        Self::apply_config(&self.config, app);
    }

    /// Applies the resolved configuration. Takes the shared handle rather
    /// than `&self`, so callers need not swap `app.ui.settings` out: the
    /// `Default` used by `mem::take` builds a full state with disk I/O.
    fn apply_config(config: &Mutex<LayeredConfig>, app: &mut App) {
        let config = config.lock().unwrap();
        let resolved = config.resolved();

        let font_size = resolved["editor.fontSize"].as_f64().unwrap_or(13.0);
        let line_height = resolved["editor.lineHeight"].as_f64().unwrap_or(1.4);
        let (size, line) = physical_text_metrics(font_size, line_height, app.scale_factor);
        app.editor_view.set_text_dimensions(size, line);
        app.apply_editor_font(resolved["editor.fontFamily"].as_str());
        if let Some(lig) = resolved["editor.fontLigatures"].as_bool() {
            app.editor_view.metrics_mut().ligatures = lig;
        }
        if let Some(wrap) = resolved["editor.wordWrap"].as_bool() {
            app.editor_view.metrics_mut().word_wrap = wrap;
            if wrap {
                app.viewport.scroll_x = 0.0;
            }
        }
        if let Some(rulers) = resolved["editor.columnRulers"].as_array() {
            app.editor_view.metrics_mut().column_rulers = rulers
                .iter()
                .filter_map(|r| r.as_u64().map(|n| n as usize))
                .collect();
        }

        if let Some(theme_name) = resolved["workbench.colorTheme"]
            .as_str()
            .or_else(|| resolved["theme"].as_str())
        {
            app.apply_color_theme_setting(theme_name);
        }

        if let Some(keymap_name) = resolved["workbench.keymap"]
            .as_str()
            .or_else(|| resolved["keymap"].as_str())
        {
            match keymap_name {
                "vscode" => app.keymap = lux_config::keymap::presets::vscode(),
                "jetbrains" => app.keymap = lux_config::keymap::presets::jetbrains(),
                _ => app.keymap = lux_config::keymap::presets::sublime(),
            }
        }

        crate::app_icon::apply(resolved["workbench.icon"].as_str());

        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }

    pub(crate) fn is_settings_path(&self, path: &Path) -> bool {
        path == self.settings_path
            || path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "settings.json" || n == "settings.jsonc")
    }

    pub(crate) fn validate_buffer(app: &mut App, path: &Path, text: &str) {
        if !app.ui.settings.is_settings_path(path) {
            return;
        }
        let diagnostics = lux_config::validate(text);
        let items: Vec<DiagnosticItem> = diagnostics
            .into_iter()
            .map(|d| {
                let line = (d.line.saturating_sub(1)) as u32;
                let col = (d.column.saturating_sub(1)) as u32;
                DiagnosticItem {
                    range: Range {
                        start: Position {
                            line,
                            character: col,
                        },
                        end: Position {
                            line,
                            character: col + 1,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: d.message,
                }
            })
            .collect();

        if let Ok(url) = lux_lsp::async_lsp::lsp_types::Url::from_file_path(path) {
            app.ui.lsp.current_url = Some(url.clone());
            if let Some(entry) = app
                .ui
                .lsp
                .all_diagnostics
                .iter_mut()
                .find(|(u, _)| *u == url)
            {
                entry.1 = items.clone();
            } else {
                app.ui.lsp.all_diagnostics.push((url, items.clone()));
            }
        }
        crate::app::view_bind::set_buffer_diagnostics(app, items);
        app.scheduler.invalidate_all();
    }

    pub(crate) fn complete_buffer(app: &mut App, text: &str, byte_offset: usize) -> bool {
        let items = lux_config::completion::complete(text, byte_offset);
        if items.is_empty() {
            return false;
        }
        let lsp_items: Vec<CompletionItem> = items
            .into_iter()
            .map(|item| CompletionItem {
                label: item.label,
                detail: Some(item.detail),
                insert_text: Some(item.insert_text),
                kind: Some(CompletionItemKind::PROPERTY),
                ..Default::default()
            })
            .collect();

        app.ui.lsp.completion_popup = Some(CompletionPopupState::showing(lsp_items));
        app.scheduler.invalidate_all();
        true
    }
}

/// `editor.lineHeight` below 8 is a multiplier of the font size, matching
/// the settings page. 8 and above is a logical pixel height. Both are then
/// multiplied by the display scale. Writing the multiplier straight into
/// the pixel line height blanks the editor after a theme change and makes
/// every line of a tall file visible at once.
pub(crate) fn physical_text_metrics(font_size: f64, line_height: f64, scale: f32) -> (f32, f32) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let size = if font_size.is_finite() && font_size > 0.0 {
        font_size as f32
    } else {
        crate::app::FONT_SIZE
    };
    let logical_line = if (0.5..8.0).contains(&line_height) {
        size * line_height as f32
    } else if line_height.is_finite() && line_height >= 8.0 {
        line_height as f32
    } else {
        size * 1.4
    };
    (size * scale, logical_line * scale)
}

/// Opens the user settings file, creating it with defaults first. The
/// create runs on a worker; the document worker then reads the file.
pub(crate) fn open_settings_json(app: &mut App, ctx: &dyn PlatformContext) {
    let path = app.ui.settings.settings_path.clone();
    crate::ui_io::spawn(app, Some(ctx), move || {
        if !path.exists() {
            let _ = fs::write(&path, DEFAULT_SETTINGS_JSON);
        }
        Some(crate::ui_io::Done::Open(path))
    });
}

/// Writes `key` into the user settings file on a worker. See
/// [`set_settings`].
pub(crate) fn set_string_setting(app: &mut App, key: &str, value: &str) {
    set_setting(app, key, Value::String(value.to_string()));
}

pub(crate) fn set_setting(app: &mut App, key: &str, value: Value) {
    set_settings(app, vec![(key.to_string(), value)]);
}

/// Writes `pairs` into the user settings file and reloads the user layer on
/// a worker. Writes run in call order. The app applies the new values when
/// the worker reports back; nothing here touches the disk on the UI thread.
pub(crate) fn set_settings(app: &mut App, pairs: Vec<(String, Value)>) {
    let path = app.ui.settings.settings_path.clone();
    let config = app.ui.settings.config.clone();
    crate::ui_io::spawn_serial(app, None, move || {
        if let Err(error) = write_user_values(&path, &pairs) {
            return Some(crate::ui_io::Done::SettingsFailed(error));
        }
        // Reads outside the lock: `apply_config` on the UI thread takes
        // the same mutex and must never wait on the disk.
        let _ = LayeredConfig::reload_layer_shared(&config, Layer::User);
        Some(crate::ui_io::Done::SettingsSaved)
    });
}

/// Reloads the user layer from disk on a worker, then applies it. Used
/// after the settings file was saved from an editor tab.
pub(crate) fn reload_user_settings(app: &mut App) {
    let config = app.ui.settings.config.clone();
    crate::ui_io::spawn_serial(app, None, move || {
        let _ = LayeredConfig::reload_layer_shared(&config, Layer::User);
        Some(crate::ui_io::Done::SettingsSaved)
    });
}

/// Read-modify-write of the user settings file. Worker threads only.
///
/// Edits the JSONC text in place, so comments and formatting survive. A
/// file that cannot be read or does not parse is left untouched and the
/// error is returned for the user: replacing it would lose every other
/// setting. A missing file starts from an empty object.
pub(crate) fn write_user_values(path: &Path, pairs: &[(String, Value)]) -> Result<(), String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let edited = lux_config::layering::jsonc::set_top_level(&text, pairs)
        .map_err(|error| format!("{} was not changed: {error}", path.display()))?;
    fs::write(path, edited).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

impl App {
    pub(crate) fn apply_user_settings(&mut self) {
        let config = self.ui.settings.config.clone();
        SettingsState::apply_config(&config, self);
    }

    /// Applies the user layer when the settings-file watcher reloaded it.
    pub(crate) fn poll_settings_watcher(&mut self) {
        if self.ui.settings.watcher_reloaded() {
            self.apply_user_settings();
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod visual_tests;
