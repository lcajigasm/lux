//! Layered JSONC settings, keymaps, `.editorconfig`/`.gitattributes`
//! support, settings importers, and typography settings for Lux.
//! `layering` (JSONC parsing, layered precedence, hot reload), `keymap`
//! (keymaps as data, presets, platform resolution, conflict detection),
//! `editorconfig`, `gitattributes` (line-ending normalization), `import`
//! (schema-aware Sublime/VS Code/JetBrains settings importers), and
//! `font_settings` (typography with project/folder overrides) are
//! implemented; a preferences surface is the one remaining Phase 5
//! deliverable, not yet started.

pub mod editorconfig;
pub mod font_settings;
pub mod gitattributes;
pub mod import;
pub mod keymap;
pub mod layering;

pub use editorconfig::EditorConfig;
pub use font_settings::FontSettings;
pub use gitattributes::{EolNormalization, GitAttributes, LineEnding};
pub use import::{IgnoreReason, IgnoredSetting, ImportReport, ImportedSetting};
pub use keymap::{Chord, ChordParseError, ChordSequence, Conflict, KeyBinding, Keymap, Platform};
pub use layering::{ConfigWatcher, JsoncError, Layer, LayerError, LayeredConfig, ReloadEvent};
