//! Settings configuration, schema validation, autocompletion, hot-reloading,
//! preferences UI panel, and external settings importers (Goal G22).

mod importers;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lux_config::layering::watch::ReloadEvent;
use lux_config::layering::{ConfigWatcher, Layer, LayeredConfig};
use lux_lsp::async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, DiagnosticSeverity, Position, Range,
};
use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{Key, NamedKey, PlatformContext};
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SettingKind {
    Number {
        value: f64,
        options: &'static [f64],
    },
    Boolean {
        value: bool,
    },
    Enum {
        value: String,
        options: &'static [&'static str],
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SettingItem {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
    pub(crate) kind: SettingKind,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SettingsPanelState {
    pub(crate) selected_index: usize,
    pub(crate) items: Vec<SettingItem>,
}

pub(crate) struct SettingsState {
    pub(crate) config: Arc<Mutex<LayeredConfig>>,
    pub(crate) watcher: Option<ConfigWatcher>,
    pub(crate) settings_path: PathBuf,
    pub(crate) panel: Option<SettingsPanelState>,
    pub(crate) schemes: Vec<crate::color_scheme::LoadedScheme>,
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

        let mut schemes = vec![crate::color_scheme::bundled_monokai()];
        if let Some(dir) = settings_path.parent() {
            schemes.extend(crate::color_scheme::load_user_schemes(&dir.join("themes")));
        }

        Self {
            config,
            watcher,
            settings_path,
            panel: None,
            schemes,
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

        if let Some(size) = resolved["editor.fontSize"].as_f64() {
            app.editor_view.metrics_mut().font_size = size as f32;
        }
        if let Some(lh) = resolved["editor.lineHeight"].as_f64() {
            app.editor_view.metrics_mut().line_height = lh as f32;
        }
        if let Some(lig) = resolved["editor.fontLigatures"].as_bool() {
            app.editor_view.metrics_mut().ligatures = lig;
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

        app.ui.lsp.completion_popup = Some(CompletionPopupState {
            items: lsp_items,
            selected_index: 0,
            rect: [0.0; 4],
        });
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

    pub(crate) fn open_settings_ui(&mut self) {
        if self.panel.is_some() {
            self.panel = None;
            return;
        }

        let config = self.config.lock().unwrap();
        let r = config.resolved();

        let font_size = r["editor.fontSize"].as_f64().unwrap_or(13.0);
        let line_height = r["editor.lineHeight"].as_f64().unwrap_or(1.4);
        let font_ligatures = r["editor.fontLigatures"].as_bool().unwrap_or(true);
        let tab_width = r["editor.tabWidth"].as_f64().unwrap_or(4.0);
        let indent_style = r["editor.indentStyle"]
            .as_str()
            .unwrap_or("space")
            .to_string();
        let trim_ws = r["editor.trimTrailingWhitespace"]
            .as_bool()
            .unwrap_or(false);
        let preserve_cursor = r["editor.trimTrailingWhitespacePreserveCursor"]
            .as_bool()
            .unwrap_or(false);
        let final_nl = r["editor.insertFinalNewline"].as_bool().unwrap_or(false);
        let word_wrap = r["editor.wordWrap"].as_bool().unwrap_or(false);
        let df_col = r["editor.distractionFreeColumnWidth"]
            .as_f64()
            .unwrap_or(80.0);
        let theme = r["workbench.colorTheme"]
            .as_str()
            .unwrap_or("monokai")
            .to_string();
        let keymap = r["workbench.keymap"]
            .as_str()
            .unwrap_or("sublime")
            .to_string();

        let items = vec![
            SettingItem {
                key: "editor.fontSize",
                label: "Font Size",
                description: "Typeface size in points (10.0 to 24.0)",
                kind: SettingKind::Number {
                    value: font_size,
                    options: &[10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 24.0],
                },
            },
            SettingItem {
                key: "editor.lineHeight",
                label: "Line Height",
                description: "Multiplier of font size (1.2 to 2.0)",
                kind: SettingKind::Number {
                    value: line_height,
                    options: &[1.2, 1.3, 1.4, 1.5, 1.6, 1.8, 2.0],
                },
            },
            SettingItem {
                key: "editor.fontLigatures",
                label: "Font Ligatures",
                description: "Enable or disable programming ligatures",
                kind: SettingKind::Boolean {
                    value: font_ligatures,
                },
            },
            SettingItem {
                key: "editor.tabWidth",
                label: "Tab Size",
                description: "Number of columns a tab character occupies",
                kind: SettingKind::Number {
                    value: tab_width,
                    options: &[2.0, 4.0, 8.0],
                },
            },
            SettingItem {
                key: "editor.indentStyle",
                label: "Indent Style",
                description: "Indentation character (space or tab)",
                kind: SettingKind::Enum {
                    value: indent_style,
                    options: &["space", "tab"],
                },
            },
            SettingItem {
                key: "editor.trimTrailingWhitespace",
                label: "Trim Trailing Whitespace",
                description: "Remove whitespace at end of lines on save",
                kind: SettingKind::Boolean { value: trim_ws },
            },
            SettingItem {
                key: "editor.trimTrailingWhitespacePreserveCursor",
                label: "Preserve Whitespace On Caret",
                description: "Keep trailing whitespace on lines with active cursors",
                kind: SettingKind::Boolean {
                    value: preserve_cursor,
                },
            },
            SettingItem {
                key: "editor.insertFinalNewline",
                label: "Insert Final Newline",
                description: "Ensure file ends with a trailing newline",
                kind: SettingKind::Boolean { value: final_nl },
            },
            SettingItem {
                key: "editor.wordWrap",
                label: "Word Wrap",
                description: "Wrap lines exceeding the viewport",
                kind: SettingKind::Boolean { value: word_wrap },
            },
            SettingItem {
                key: "editor.distractionFreeColumnWidth",
                label: "Distraction Free Column Width",
                description: "Character width of centered text in distraction free mode",
                kind: SettingKind::Number {
                    value: df_col,
                    options: &[72.0, 80.0, 90.0, 100.0, 120.0],
                },
            },
            SettingItem {
                key: "workbench.colorTheme",
                label: "Color Theme",
                description: "Visual color palette. \"system\" follows the OS appearance",
                kind: SettingKind::Enum {
                    value: theme,
                    options: &[
                        "system",
                        "monokai",
                        "mariana",
                        "breakers",
                        "celeste",
                        "lux-dark",
                        "lux-light",
                        "high-contrast",
                    ],
                },
            },
            SettingItem {
                key: "workbench.keymap",
                label: "Keymap Preset",
                description: "Keyboard shortcut mapping profile",
                kind: SettingKind::Enum {
                    value: keymap,
                    options: &["sublime", "vscode", "jetbrains"],
                },
            },
        ];

        self.panel = Some(SettingsPanelState {
            selected_index: 0,
            items,
        });
    }

    pub(crate) fn update_setting_value(&mut self, app: &mut App, key: &str, val: Value) {
        let mut map: serde_json::Map<String, Value> = match fs::read_to_string(&self.settings_path)
        {
            Ok(s) => lux_config::layering::jsonc::parse(&s)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default(),
            Err(_) => serde_json::Map::new(),
        };

        map.insert(key.to_string(), val);
        let obj = Value::Object(map);
        if let Ok(json_str) = serde_json::to_string_pretty(&obj) {
            let _ = fs::write(&self.settings_path, json_str);
        }

        let _ = self.config.lock().unwrap().reload_layer(Layer::User);
        self.apply_to_app(app);
    }

    pub(crate) fn cycle_current_setting(&mut self, app: &mut App, forward: bool) {
        let Some(panel) = self.panel.as_mut() else {
            return;
        };
        let Some(item) = panel.items.get_mut(panel.selected_index) else {
            return;
        };

        let new_json_val = match &mut item.kind {
            SettingKind::Boolean { value } => {
                *value = !*value;
                Value::Bool(*value)
            }
            SettingKind::Number { value, options } => {
                let cur_idx = options
                    .iter()
                    .position(|&o| (o - *value).abs() < 0.01)
                    .unwrap_or(0);
                let next_idx = if forward {
                    (cur_idx + 1) % options.len()
                } else {
                    (cur_idx + options.len() - 1) % options.len()
                };
                *value = options[next_idx];
                serde_json::json!(*value)
            }
            SettingKind::Enum { value, options } => {
                let cur_idx = options.iter().position(|&o| o == value).unwrap_or(0);
                let next_idx = if forward {
                    (cur_idx + 1) % options.len()
                } else {
                    (cur_idx + options.len() - 1) % options.len()
                };
                *value = options[next_idx].to_string();
                Value::String(value.clone())
            }
        };

        let key = item.key;
        self.update_setting_value(app, key, new_json_val);
    }

    pub(crate) fn handle_key(&mut self, app: &mut App, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }
        let Some(panel) = self.panel.as_mut() else {
            return false;
        };

        match &event.key {
            Key::Named(NamedKey::Escape) => {
                self.panel = None;
                app.scheduler.invalidate_all();
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                if !panel.items.is_empty() {
                    panel.selected_index = (panel.selected_index + 1) % panel.items.len();
                    app.scheduler.invalidate_all();
                }
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                if !panel.items.is_empty() {
                    panel.selected_index =
                        (panel.selected_index + panel.items.len() - 1) % panel.items.len();
                    app.scheduler.invalidate_all();
                }
                true
            }
            Key::Named(NamedKey::Enter)
            | Key::Named(NamedKey::Space)
            | Key::Named(NamedKey::ArrowRight) => {
                self.cycle_current_setting(app, true);
                true
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.cycle_current_setting(app, false);
                true
            }
            _ => true,
        }
    }

    pub(crate) fn handle_click(&mut self, app: &mut App, mouse_x: f32, mouse_y: f32) -> bool {
        let Some(panel) = self.panel.as_mut() else {
            return false;
        };
        let scale = app.scale_factor;
        let width = (app.viewport.width * 0.7).clamp(420.0 * scale, 760.0 * scale);
        let left = (app.viewport.width - width) * 0.5;
        let top = app.tab_bar_height() + 16.0 * scale;
        let row_height = 42.0 * scale;
        let header_height = 56.0 * scale;

        let total_height = header_height + (panel.items.len() as f32 * row_height) + 12.0 * scale;
        if mouse_x < left || mouse_x > left + width || mouse_y < top || mouse_y > top + total_height
        {
            self.panel = None;
            app.scheduler.invalidate_all();
            return true;
        }

        let body_y = mouse_y - (top + header_height);
        if body_y >= 0.0 {
            let clicked_idx = (body_y / row_height) as usize;
            if clicked_idx < panel.items.len() {
                panel.selected_index = clicked_idx;
                self.cycle_current_setting(app, true);
                return true;
            }
        }
        true
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

#[cfg(test)]
mod tests;
