//! `KeyBinding` and `Keymap`: the data shape presets are loaded into.
//! `KeyBinding`'s `key`/`mac`/`win`/`linux` fields mirror VS Code's actual
//! `keybindings.json` schema (a default `key`, optionally overridden per
//! platform) rather than inventing a new one — real prior art for "keymaps
//! per platform" that Sublime's and JetBrains' presets are expressed
//! through just as well, since nothing here is VS Code-specific.

use serde::Deserialize;

use super::chord::ChordSequence;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
}

/// One binding as authored in a preset's `.jsonc` file: a command, its
/// default chord sequence, optional per-platform overrides, and an opaque
/// `when` context expression (stored, not evaluated — a real condition
/// language is out of this deliverable's scope; carrying the string through
/// is enough for [`super::conflicts`] to tell two bindings on the same
/// chord apart when they're scoped to different contexts).
#[derive(Debug, Clone, Deserialize)]
pub struct KeyBinding {
    pub command: String,
    pub key: ChordSequence,
    #[serde(default)]
    pub mac: Option<ChordSequence>,
    #[serde(default)]
    pub win: Option<ChordSequence>,
    #[serde(default)]
    pub linux: Option<ChordSequence>,
    #[serde(default)]
    pub when: Option<String>,
}

impl KeyBinding {
    /// The chord sequence that actually applies on `platform`: the
    /// platform-specific override if the binding has one, otherwise `key`.
    pub fn resolve(&self, platform: Platform) -> &ChordSequence {
        let override_for = match platform {
            Platform::Mac => &self.mac,
            Platform::Windows => &self.win,
            Platform::Linux => &self.linux,
        };
        override_for.as_ref().unwrap_or(&self.key)
    }
}

/// A named collection of bindings — one per preset (Sublime, VS Code,
/// JetBrains), but not tied to any of them specifically.
#[derive(Debug, Clone)]
pub struct Keymap {
    pub name: String,
    pub bindings: Vec<KeyBinding>,
}

impl Keymap {
    /// The chord sequence bound to `command` on `platform`, if any. Presets
    /// are small enough (a few dozen bindings) that a linear scan is the
    /// whole story — no index worth building for this.
    pub fn binding_for(&self, command: &str, platform: Platform) -> Option<&ChordSequence> {
        self.bindings
            .iter()
            .find(|b| b.command == command)
            .map(|b| b.resolve(platform))
    }

    /// The command bound to `chord` on `platform`, if any.
    pub fn command_for(&self, chord: &ChordSequence, platform: Platform) -> Option<&str> {
        self.bindings
            .iter()
            .find(|b| b.resolve(platform) == chord)
            .map(|b| b.command.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(command: &str, key: &str, mac: Option<&str>) -> KeyBinding {
        KeyBinding {
            command: command.to_string(),
            key: key.parse().unwrap(),
            mac: mac.map(|s| s.parse().unwrap()),
            win: None,
            linux: None,
            when: None,
        }
    }

    #[test]
    fn resolve_falls_back_to_key_when_no_platform_override_exists() {
        let b = binding("save", "ctrl+s", None);
        assert_eq!(b.resolve(Platform::Windows).to_string(), "ctrl+s");
        assert_eq!(b.resolve(Platform::Mac).to_string(), "ctrl+s");
    }

    #[test]
    fn resolve_prefers_the_platform_override_when_present() {
        let b = binding("save", "ctrl+s", Some("cmd+s"));
        assert_eq!(b.resolve(Platform::Mac).to_string(), "cmd+s");
        assert_eq!(b.resolve(Platform::Windows).to_string(), "ctrl+s");
        assert_eq!(b.resolve(Platform::Linux).to_string(), "ctrl+s");
    }

    #[test]
    fn binding_for_finds_by_command_and_resolves_the_platform() {
        let keymap = Keymap {
            name: "test".to_string(),
            bindings: vec![binding("save", "ctrl+s", Some("cmd+s"))],
        };
        assert_eq!(
            keymap
                .binding_for("save", Platform::Mac)
                .unwrap()
                .to_string(),
            "cmd+s"
        );
        assert!(keymap.binding_for("nonexistent", Platform::Mac).is_none());
    }

    #[test]
    fn command_for_finds_by_chord_and_resolves_the_platform() {
        let keymap = Keymap {
            name: "test".to_string(),
            bindings: vec![binding("save", "ctrl+s", Some("cmd+s"))],
        };
        let mac_chord: ChordSequence = "cmd+s".parse().unwrap();
        let win_chord: ChordSequence = "ctrl+s".parse().unwrap();
        assert_eq!(keymap.command_for(&mac_chord, Platform::Mac), Some("save"));
        assert_eq!(
            keymap.command_for(&win_chord, Platform::Windows),
            Some("save")
        );
        assert!(keymap.command_for(&win_chord, Platform::Mac).is_none());
    }
}
