//! Layered settings resolution (Phase 5 deliverable, narrowed scope: JSONC
//! parsing, layered precedence, hot reload — schema publication and
//! line/column *validation* against that schema are separate, later work;
//! `jsonc`'s errors already carry line/column for the parse-failure case
//! this scope does cover).
//!
//! Five layers, lowest precedence first: [`Layer::Default`] (built into the
//! binary, no file), [`Layer::User`], [`Layer::Project`], [`Layer::Folder`],
//! [`Layer::Language`]. Each file-backed layer is optional — a layer with
//! no file contributes nothing. Resolution is a recursive JSON object
//! merge: later layers override earlier ones key by key, recursing into
//! nested objects; a non-object value (scalar or array) in a higher layer
//! fully replaces whatever the lower layer had at that key rather than
//! trying to merge array contents — an array replacing an array is what
//! every settings system with this kind of layering (VS Code included)
//! actually does, since splicing array elements across layers has no sane
//! general definition.
//!
//! "A malformed configuration never leaves the editor unusable or loses
//! settings" (this phase's acceptance criterion) is why a failed parse
//! doesn't clear that layer: [`LayeredConfig::reload_layer`] keeps the
//! layer's last successfully parsed value and returns the parse error
//! separately, so a typo in one settings file degrades to "that layer is
//! stale until fixed" rather than "settings are gone" or "editor won't
//! start". Resolving *which* file backs the `Project`/`Folder`/`Language`
//! layers for a given editing context (which project, which workspace
//! folder, which language id) is a future integration's job, not this
//! module's — it just merges whatever paths it's given, in the fixed
//! precedence order.

pub mod jsonc;
pub mod watch;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub use jsonc::JsoncError;
pub use watch::{ConfigWatcher, ReloadEvent};

/// Precedence order is declaration order — `derive(Ord)` on a
/// variant-only enum orders by position, so `Layer::Default < Layer::User <
/// ... < Layer::Language`, and merging in [`Layer::ALL`] order gives later
/// layers priority over earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    Default,
    User,
    Project,
    Folder,
    Language,
}

impl Layer {
    /// Every layer, lowest precedence first. `Default` is included for
    /// completeness even though it's never file-backed and never appears
    /// as a key in `LayeredConfig`'s internal map.
    pub const ALL: [Layer; 5] = [
        Layer::Default,
        Layer::User,
        Layer::Project,
        Layer::Folder,
        Layer::Language,
    ];
}

/// One file-backed layer's state. `value` is `None` until the first
/// successful load, so "never loaded" and "loaded but contributes nothing"
/// (an empty document) both round-trip correctly through the merge.
#[derive(Debug, Clone, Default)]
struct LayerState {
    path: Option<PathBuf>,
    value: Option<Value>,
}

/// A layer reload attempt that could not use the file it was pointed at.
#[derive(Debug)]
pub enum LayerError {
    Io(io::Error),
    Parse(JsoncError),
}

impl std::fmt::Display for LayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerError::Io(err) => write!(f, "{err}"),
            LayerError::Parse(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for LayerError {}

/// Settings resolved across all five layers, kept up to date as
/// file-backed layers reload. Not thread-safe on its own —
/// [`watch::ConfigWatcher`] wraps one in a mutex for its background reload
/// thread.
#[derive(Debug, Clone)]
pub struct LayeredConfig {
    default: Value,
    layers: BTreeMap<Layer, LayerState>,
    merged: Value,
}

impl LayeredConfig {
    /// `default_jsonc` is the built-in defaults document. Parsing it is the
    /// one layer failure this can't degrade gracefully from (there's no
    /// previous value to fall back to for a layer that's never
    /// successfully loaded), so it's a hard error here rather than folded
    /// into `reload_layer`'s keep-the-last-good-value handling.
    pub fn new(default_jsonc: &str) -> Result<Self, JsoncError> {
        let default = jsonc::parse(default_jsonc)?;
        let merged = default.clone();
        Ok(LayeredConfig {
            default,
            layers: BTreeMap::new(),
            merged,
        })
    }

    /// Points `layer` at `path` and loads it immediately.
    ///
    /// # Panics
    /// If `layer` is [`Layer::Default`] — it's fixed at construction time,
    /// not file-backed.
    pub fn set_layer_path(&mut self, layer: Layer, path: PathBuf) -> Result<(), LayerError> {
        assert_ne!(layer, Layer::Default, "the default layer has no file");
        self.layers.entry(layer).or_default().path = Some(path);
        self.reload_layer(layer)
    }

    /// Clears `layer`'s source and contribution to the merge.
    ///
    /// # Panics
    /// If `layer` is [`Layer::Default`].
    pub fn clear_layer(&mut self, layer: Layer) {
        assert_ne!(layer, Layer::Default, "the default layer can't be cleared");
        self.layers.remove(&layer);
        self.recompute();
    }

    /// Path currently backing `layer`, if any.
    pub fn layer_path(&self, layer: Layer) -> Option<&Path> {
        self.layers.get(&layer).and_then(|s| s.path.as_deref())
    }

    /// Re-reads `layer`'s file from disk and reparses it — the operation
    /// [`watch::ConfigWatcher`] drives on every filesystem change.
    ///
    /// A missing file is not an error: it's the same as the layer never
    /// having been set, so it simply contributes nothing (settings files
    /// are optional). A file that exists but fails to parse *is* an error,
    /// but the layer's previous value (if any) is left in place instead of
    /// cleared, so the merged result never regresses because of one bad
    /// edit — the acceptance criterion this exists for.
    ///
    /// # Panics
    /// If `layer` is [`Layer::Default`].
    pub fn reload_layer(&mut self, layer: Layer) -> Result<(), LayerError> {
        assert_ne!(
            layer,
            Layer::Default,
            "the default layer has no file to reload"
        );
        let Some(path) = self.layers.get(&layer).and_then(|s| s.path.clone()) else {
            return Ok(());
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                self.layers.get_mut(&layer).unwrap().value = None;
                self.recompute();
                return Ok(());
            }
            Err(err) => return Err(LayerError::Io(err)),
        };
        match jsonc::parse(&text) {
            Ok(value) => {
                self.layers.get_mut(&layer).unwrap().value = Some(value);
                self.recompute();
                Ok(())
            }
            Err(err) => Err(LayerError::Parse(err)),
        }
    }

    /// The merged view: `Default` overridden by every set layer, in
    /// precedence order.
    pub fn resolved(&self) -> &Value {
        &self.merged
    }

    fn recompute(&mut self) {
        let mut merged = self.default.clone();
        for layer in Layer::ALL {
            if let Some(value) = self.layers.get(&layer).and_then(|s| s.value.as_ref()) {
                merge(&mut merged, value);
            }
        }
        self.merged = merged;
    }
}

/// Recursive JSON object merge: `patch`'s keys override `base`'s, recursing
/// when both sides are objects at the same key; anything else (a scalar, an
/// array, or a type mismatch between the two) is a full replacement.
fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, patch_value) in patch_map {
                match base_map.get_mut(key) {
                    Some(base_value) => merge(base_value, patch_value),
                    None => {
                        base_map.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::thread;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-config-layering-test-{label}-{:?}-{}",
            thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn default_only_config_resolves_to_the_default_document() {
        let config = LayeredConfig::new(r#"{ "editor": { "fontSize": 12 } }"#).unwrap();
        assert_eq!(config.resolved()["editor"]["fontSize"], 12);
    }

    #[test]
    fn higher_layer_overrides_lower_layer_at_the_same_key() {
        let dir = temp_dir("override");
        let mut config = LayeredConfig::new(r#"{ "fontSize": 12 }"#).unwrap();
        config
            .set_layer_path(
                Layer::User,
                write(&dir, "user.jsonc", r#"{ "fontSize": 14 }"#),
            )
            .unwrap();
        assert_eq!(config.resolved()["fontSize"], 14);

        config
            .set_layer_path(
                Layer::Project,
                write(&dir, "project.jsonc", r#"{ "fontSize": 16 }"#),
            )
            .unwrap();
        assert_eq!(config.resolved()["fontSize"], 16);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nested_objects_merge_recursively_instead_of_being_replaced_wholesale() {
        let dir = temp_dir("nested");
        let mut config = LayeredConfig::new(
            r#"{ "editor": { "fontSize": 12, "tabSize": 4, "ligatures": true } }"#,
        )
        .unwrap();
        config
            .set_layer_path(
                Layer::User,
                write(&dir, "user.jsonc", r#"{ "editor": { "fontSize": 14 } }"#),
            )
            .unwrap();

        // Overridden key changes...
        assert_eq!(config.resolved()["editor"]["fontSize"], 14);
        // ...but sibling keys the user layer never mentioned survive from
        // the default layer, because the merge recurses into the object
        // instead of replacing it outright.
        assert_eq!(config.resolved()["editor"]["tabSize"], 4);
        assert_eq!(config.resolved()["editor"]["ligatures"], true);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn arrays_are_replaced_wholesale_not_merged_element_by_element() {
        let dir = temp_dir("arrays");
        let mut config = LayeredConfig::new(r#"{ "excludes": ["a", "b", "c"] }"#).unwrap();
        config
            .set_layer_path(
                Layer::User,
                write(&dir, "user.jsonc", r#"{ "excludes": ["x"] }"#),
            )
            .unwrap();

        assert_eq!(config.resolved()["excludes"], serde_json::json!(["x"]));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn precedence_is_default_user_project_folder_language() {
        let dir = temp_dir("precedence");
        let mut config = LayeredConfig::new(r#"{ "value": "default" }"#).unwrap();
        config
            .set_layer_path(
                Layer::User,
                write(&dir, "user.jsonc", r#"{ "value": "user" }"#),
            )
            .unwrap();
        config
            .set_layer_path(
                Layer::Project,
                write(&dir, "project.jsonc", r#"{ "value": "project" }"#),
            )
            .unwrap();
        config
            .set_layer_path(
                Layer::Folder,
                write(&dir, "folder.jsonc", r#"{ "value": "folder" }"#),
            )
            .unwrap();
        config
            .set_layer_path(
                Layer::Language,
                write(&dir, "language.jsonc", r#"{ "value": "language" }"#),
            )
            .unwrap();
        assert_eq!(config.resolved()["value"], "language");

        // Peel layers off from the top and confirm the next one down takes
        // over each time — proves the order is enforced, not just that the
        // topmost layer happens to win.
        config.clear_layer(Layer::Language);
        assert_eq!(config.resolved()["value"], "folder");
        config.clear_layer(Layer::Folder);
        assert_eq!(config.resolved()["value"], "project");
        config.clear_layer(Layer::Project);
        assert_eq!(config.resolved()["value"], "user");
        config.clear_layer(Layer::User);
        assert_eq!(config.resolved()["value"], "default");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn malformed_layer_keeps_the_previous_good_value_and_reports_the_error() {
        let dir = temp_dir("malformed");
        let mut config = LayeredConfig::new(r#"{ "value": "default" }"#).unwrap();
        let path = write(&dir, "user.jsonc", r#"{ "value": "good" }"#);
        config.set_layer_path(Layer::User, path.clone()).unwrap();
        assert_eq!(config.resolved()["value"], "good");

        fs::write(&path, "{ this is not valid JSON").unwrap();
        let err = config.reload_layer(Layer::User).unwrap_err();
        assert!(matches!(err, LayerError::Parse(_)));
        // The whole point: a bad edit degrades to "stale", never to
        // "settings disappeared" or a broken merge.
        assert_eq!(config.resolved()["value"], "good");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_layer_file_contributes_nothing_without_erroring() {
        let dir = temp_dir("missing");
        let mut config = LayeredConfig::new(r#"{ "value": "default" }"#).unwrap();
        config
            .set_layer_path(Layer::User, dir.join("does-not-exist.jsonc"))
            .unwrap();
        assert_eq!(config.resolved()["value"], "default");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reload_after_the_file_is_deleted_falls_back_silently() {
        let dir = temp_dir("deleted");
        let mut config = LayeredConfig::new(r#"{ "value": "default" }"#).unwrap();
        let path = write(&dir, "user.jsonc", r#"{ "value": "user" }"#);
        config.set_layer_path(Layer::User, path.clone()).unwrap();
        assert_eq!(config.resolved()["value"], "user");

        fs::remove_file(&path).unwrap();
        config.reload_layer(Layer::User).unwrap();
        assert_eq!(config.resolved()["value"], "default");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_layer_removes_its_contribution_and_its_path() {
        let dir = temp_dir("clear");
        let mut config = LayeredConfig::new(r#"{ "value": "default" }"#).unwrap();
        let path = write(&dir, "user.jsonc", r#"{ "value": "user" }"#);
        config.set_layer_path(Layer::User, path).unwrap();
        assert!(config.layer_path(Layer::User).is_some());

        config.clear_layer(Layer::User);
        assert_eq!(config.resolved()["value"], "default");
        assert!(config.layer_path(Layer::User).is_none());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn malformed_default_document_is_a_hard_error() {
        let err = LayeredConfig::new("{ not json").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("line"));
    }
}
