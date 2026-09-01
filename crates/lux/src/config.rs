//! Runtime wiring for `lux-config`: layered JSONC settings, the Sublime
//! keymap preset (plus Lux open/save-as overlays), and notify-based hot
//! reload with no polling.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use lux_config::font_settings::{self, FontSettings};
use lux_config::keymap::presets;
use lux_config::{
    Chord, ChordSequence, ConfigWatcher, KeyBinding, Keymap, Layer, LayeredConfig, Platform,
    ReloadEvent,
};
use lux_platform::{Key, Modifiers};
use lux_ui::editor_view::EditorViewMetrics;

use crate::app::{BLINK_INTERVAL, FONT_SIZE, LINE_HEIGHT};

/// Built-in defaults shipped in the binary. Flat dotted keys match
/// `lux_config::font_settings` and the published schema.
pub const DEFAULT_SETTINGS_JSONC: &str = r#"{
    // Lux built-in defaults (G03). Override in the user settings file.
    "editor.fontSize": 16,
    "editor.lineHeight": 1.4,
    "editor.fontLigatures": true
}"#;

/// Commands the desktop shell can actually run today. Bound chords for
/// anything else are not indexed (so they never pretend to work).
const WIRED_COMMANDS: &[&str] = &["save", "open", "save_as"];

/// Owns the watched [`LayeredConfig`], the single-chord → command index, and
/// the user settings path we print on boot.
pub struct RuntimeConfig {
    watcher: ConfigWatcher,
    /// Single-chord bindings only (multi-chord sequences need a future
    /// chord-chain state machine).
    commands: HashMap<Chord, String>,
    #[allow(dead_code)] // printed at bootstrap; retained for diagnostics/tests
    pub user_settings_path: PathBuf,
}

impl RuntimeConfig {
    /// Loads defaults + optional user layer, starts the notify watcher, and
    /// builds the keymap index. Returns `None` when bootstrap cannot proceed
    /// (caller keeps hardcoded fallbacks — honest, not simulated).
    pub fn bootstrap() -> Option<Self> {
        let platform = current_platform();
        let user_dir = user_config_dir();
        if let Err(err) = fs::create_dir_all(&user_dir) {
            eprintln!(
                "Lux: could not create config dir {}: {err}",
                user_dir.display()
            );
            return None;
        }
        let user_settings_path = user_dir.join("settings.jsonc");

        let config = match LayeredConfig::new(DEFAULT_SETTINGS_JSONC) {
            Ok(config) => Arc::new(Mutex::new(config)),
            Err(err) => {
                eprintln!("Lux: default settings failed to parse: {err}");
                return None;
            }
        };

        let watcher = match ConfigWatcher::new(
            Arc::clone(&config),
            [(Layer::User, user_settings_path.clone())],
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                eprintln!("Lux: config watcher failed to start: {err}");
                return None;
            }
        };

        let keymap = desktop_keymap();
        let commands = build_command_index(&keymap, platform);
        println!(
            "Lux: config watching {} (keymap: {})",
            user_settings_path.display(),
            keymap.name
        );

        Some(Self {
            watcher,
            commands,
            user_settings_path,
        })
    }

    pub fn lookup_command(&self, chord: &Chord) -> Option<&str> {
        self.commands.get(chord).map(String::as_str)
    }

    pub fn font_settings(&self) -> FontSettings {
        let guard = self.watcher.config().lock().unwrap();
        font_settings::resolve(&guard)
    }

    pub fn editor_metrics(&self) -> EditorViewMetrics {
        metrics_from_font_settings(&self.font_settings())
    }

    /// Drains notify-driven reload events. Returns `true` when a layer
    /// reloaded successfully (caller should re-apply editor metrics).
    pub fn drain_reloads(&mut self) -> bool {
        let mut applied = false;
        while let Some(event) = self.watcher.try_recv() {
            match event {
                ReloadEvent::Reloaded(layer) => {
                    println!("Lux: reloaded settings layer {layer:?}");
                    applied = true;
                }
                ReloadEvent::Failed(layer, err) => {
                    eprintln!("Lux: settings reload failed for {layer:?}: {err}");
                }
            }
        }
        applied
    }
}

fn desktop_keymap() -> Keymap {
    let mut keymap = presets::sublime();
    // Sublime's preset has `save` but not open / save_as — Lux overlays the
    // chords the shell already implements (same chords G02 hard-coded).
    keymap.bindings.extend([
        KeyBinding {
            command: "open".to_string(),
            key: "ctrl+o".parse().expect("static chord"),
            mac: Some("cmd+o".parse().expect("static chord")),
            win: None,
            linux: None,
            when: None,
        },
        KeyBinding {
            command: "save_as".to_string(),
            key: "ctrl+shift+s".parse().expect("static chord"),
            mac: Some("cmd+shift+s".parse().expect("static chord")),
            win: None,
            linux: None,
            when: None,
        },
    ]);
    keymap
}

fn build_command_index(keymap: &Keymap, platform: Platform) -> HashMap<Chord, String> {
    let mut map = HashMap::new();
    for binding in &keymap.bindings {
        if !WIRED_COMMANDS.contains(&binding.command.as_str()) {
            continue;
        }
        let sequence: &ChordSequence = binding.resolve(platform);
        if sequence.0.len() != 1 {
            continue;
        }
        map.insert(sequence.0[0].clone(), binding.command.clone());
    }
    map
}

pub fn chord_from_key(modifiers: &Modifiers, key: &Key) -> Option<Chord> {
    if !(modifiers.control || modifiers.alt || modifiers.meta) {
        return None;
    }
    let Key::Character(ch) = key else {
        return None;
    };
    Some(Chord {
        ctrl: modifiers.control,
        alt: modifiers.alt,
        shift: modifiers.shift,
        cmd: modifiers.meta,
        key: ch.to_lowercase(),
    })
}

pub fn metrics_from_font_settings(fonts: &FontSettings) -> EditorViewMetrics {
    let font_size = fonts
        .size
        .filter(|n| n.is_finite() && *n > 0.0)
        .map(|n| n as f32)
        .unwrap_or(FONT_SIZE);
    let line_height = match fonts.line_height {
        Some(raw) if raw.is_finite() && raw > 0.0 => {
            // Same heuristic VS Code uses: values below 8 are multipliers.
            if raw < 8.0 {
                font_size * (raw as f32)
            } else {
                raw as f32
            }
        }
        _ => {
            if fonts.size.is_some() {
                font_size * (LINE_HEIGHT / FONT_SIZE)
            } else {
                LINE_HEIGHT
            }
        }
    };
    EditorViewMetrics {
        line_height,
        font_size,
        ligatures: fonts.ligatures.unwrap_or(true),
        column_rulers: vec![80],
        blink_interval: BLINK_INTERVAL,
    }
}

fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

pub fn user_config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("lux");
    }
    if let Some(home) = std::env::var_os("HOME") {
        #[cfg(target_os = "macos")]
        {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("lux");
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                return PathBuf::from(appdata).join("lux");
            }
            return PathBuf::from(home)
                .join("AppData")
                .join("Roaming")
                .join("lux");
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            return PathBuf::from(home).join(".config").join("lux");
        }
    }
    PathBuf::from(".config").join("lux")
}

#[cfg(test)]
fn command_index_for_tests(platform: Platform) -> HashMap<Chord, String> {
    build_command_index(&desktop_keymap(), platform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn sublime_save_and_lux_open_resolve_on_linux() {
        let index = command_index_for_tests(Platform::Linux);
        let save: Chord = "ctrl+s".parse().unwrap();
        let open: Chord = "ctrl+o".parse().unwrap();
        let save_as: Chord = "ctrl+shift+s".parse().unwrap();
        assert_eq!(index.get(&save).map(String::as_str), Some("save"));
        assert_eq!(index.get(&open).map(String::as_str), Some("open"));
        assert_eq!(index.get(&save_as).map(String::as_str), Some("save_as"));
        let find: Chord = "ctrl+f".parse().unwrap();
        assert!(!index.contains_key(&find));
    }

    #[test]
    fn font_settings_drive_editor_metrics() {
        let config = LayeredConfig::new(
            r#"{ "editor.fontSize": 20, "editor.lineHeight": 1.5, "editor.fontLigatures": false }"#,
        )
        .unwrap();
        let fonts = font_settings::resolve(&config);
        let metrics = metrics_from_font_settings(&fonts);
        assert_eq!(metrics.font_size, 20.0);
        assert_eq!(metrics.line_height, 30.0);
        assert!(!metrics.ligatures);
    }

    #[test]
    fn absolute_line_height_values_are_not_treated_as_multipliers() {
        let fonts = FontSettings {
            family: None,
            size: Some(12.0),
            line_height: Some(22.0),
            ligatures: None,
        };
        let metrics = metrics_from_font_settings(&fonts);
        assert_eq!(metrics.line_height, 22.0);
    }

    #[test]
    fn watched_user_settings_hot_reload_updates_font_size() {
        let dir = std::env::temp_dir().join(format!(
            "lux-g03-reload-{}-{:?}",
            std::process::id(),
            thread::current().id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.jsonc");
        fs::write(&path, r#"{ "editor.fontSize": 14 }"#).unwrap();

        let config = Arc::new(Mutex::new(
            LayeredConfig::new(DEFAULT_SETTINGS_JSONC).unwrap(),
        ));
        let watcher =
            ConfigWatcher::new(Arc::clone(&config), [(Layer::User, path.clone())]).unwrap();
        assert_eq!(
            font_settings::resolve(&config.lock().unwrap()).size,
            Some(14.0)
        );

        let tmp = dir.join("settings.jsonc.tmp");
        fs::write(&tmp, r#"{ "editor.fontSize": 22 }"#).unwrap();
        fs::rename(&tmp, &path).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw = false;
        while Instant::now() < deadline {
            match watcher.try_recv() {
                Some(ReloadEvent::Reloaded(Layer::User)) => {
                    saw = true;
                    break;
                }
                Some(ReloadEvent::Failed(_, err)) => panic!("reload failed: {err}"),
                Some(ReloadEvent::Reloaded(_)) => {}
                None => thread::sleep(Duration::from_millis(20)),
            }
        }
        assert!(saw, "expected Reloaded(User) within timeout");
        assert_eq!(
            font_settings::resolve(&config.lock().unwrap()).size,
            Some(22.0)
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn chord_from_key_maps_ctrl_s() {
        let modifiers = Modifiers {
            control: true,
            shift: false,
            alt: false,
            meta: false,
        };
        let key = Key::Character("s".into());
        let chord = chord_from_key(&modifiers, &key).unwrap();
        assert_eq!(chord, "ctrl+s".parse().unwrap());
    }
}
