//! Settings configuration, schema validation, autocompletion, hot-reloading,
//! preferences UI panel, and external settings importers (Goal G22).

mod catalog;
mod importers;
pub(crate) mod layout;
mod panel;
mod wheel;

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
  "workbench.keymap": "sublime"
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

pub(crate) struct SettingsState {
    pub(crate) config: Arc<Mutex<LayeredConfig>>,
    pub(crate) watcher: Option<ConfigWatcher>,
    pub(crate) settings_path: PathBuf,
    pub(crate) panel: Option<SettingsPanelState>,
    pub(crate) schemes: Vec<crate::color_scheme::LoadedScheme>,
    pub(crate) palette_uses: HashMap<String, u64>,
    pub(crate) palette_clock: u64,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new(default_settings_path())
    }
}

impl SettingsState {
    pub(crate) fn new(settings_path: PathBuf) -> Self {
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

    pub(crate) fn poll_watcher(&mut self, app: &mut App) {
        let Some(watcher) = &self.watcher else {
            return;
        };
        let mut reloaded = false;
        while let Some(event) = watcher.try_recv() {
            match event {
                ReloadEvent::Reloaded(_) => reloaded = true,
                ReloadEvent::Failed(_, _) => {}
            }
        }
        if reloaded {
            self.apply_to_app(app);
        }
    }

    pub(crate) fn apply_to_app(&self, app: &mut App) {
        let config = self.config.lock().unwrap();
        let resolved = config.resolved();

        let font_size = resolved["editor.fontSize"].as_f64().unwrap_or(13.0);
        let line_height = resolved["editor.lineHeight"].as_f64().unwrap_or(1.4);
        let (size, line) = physical_text_metrics(font_size, line_height, app.scale_factor);
        app.editor_view.set_text_dimensions(size, line);
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

    pub(crate) fn validate_buffer(&self, app: &mut App, path: &Path, text: &str) {
        if !self.is_settings_path(path) {
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
        app.ui.lsp.diagnostics = items;
        app.scheduler.invalidate_all();
    }

    pub(crate) fn complete_buffer(&self, app: &mut App, text: &str, byte_offset: usize) -> bool {
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

    pub(crate) fn open_settings_json(&self, app: &mut App, ctx: &dyn PlatformContext) {
        if !self.settings_path.exists() {
            let _ = fs::write(&self.settings_path, DEFAULT_SETTINGS_JSON);
        }
        app.request_document(
            ctx,
            crate::document::Request::Open(Some(self.settings_path.clone())),
        );
    }

    pub(crate) fn set_string_setting(&mut self, key: &str, value: &str) {
        let content = fs::read_to_string(&self.settings_path).unwrap_or_default();
        let mut map = match serde_json::from_str::<Value>(&content) {
            Ok(Value::Object(m)) => m,
            _ => serde_json::Map::new(),
        };
        map.insert(key.to_string(), Value::String(value.to_string()));
        if let Ok(json_str) = serde_json::to_string_pretty(&Value::Object(map)) {
            let _ = fs::write(&self.settings_path, json_str);
        }
        let _ = self.config.lock().unwrap().reload_layer(Layer::User);
    }

    pub(crate) fn import_vscode(&mut self, app: &mut App) -> String {
        importers::import_vscode(self, app)
    }

    pub(crate) fn import_sublime(&mut self, app: &mut App) -> String {
        importers::import_sublime(self, app)
    }

    pub(crate) fn import_jetbrains(&mut self, app: &mut App) -> String {
        importers::import_jetbrains(self, app)
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

impl App {
    pub(crate) fn apply_user_settings(&mut self) {
        let settings = std::mem::take(&mut self.ui.settings);
        settings.apply_to_app(self);
        self.ui.settings = settings;
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod visual_tests;
