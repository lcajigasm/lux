//! Layered JSONC settings, keymaps, `.editorconfig`/`.gitattributes`
//! support, settings importers, typography settings, and schema-aware file
//! completion for Lux. `layering` (JSONC parsing, layered precedence, hot
//! reload), `keymap` (keymaps as data, presets, platform resolution,
//! conflict detection), `editorconfig`, `gitattributes` (line-ending
//! normalization), `import` (schema-aware Sublime/VS Code/JetBrains
//! settings importers), `font_settings` (typography with project/folder
//! overrides), and `schema`/`completion` (Phase 5's last deliverable —
//! see `completion`'s module docs for why file-editing completion was
//! chosen over a preferences UI panel) are all implemented.

pub mod completion;
pub mod editorconfig;
pub mod font_settings;
pub mod gitattributes;
pub mod import;
pub mod keymap;
pub mod layering;
pub mod schema;

pub use completion::CompletionItem;
pub use editorconfig::EditorConfig;
pub use font_settings::FontSettings;
pub use gitattributes::{EolNormalization, GitAttributes, LineEnding};
pub use import::{IgnoreReason, IgnoredSetting, ImportReport, ImportedSetting};
pub use keymap::{Chord, ChordParseError, ChordSequence, Conflict, KeyBinding, Keymap, Platform};
pub use layering::{ConfigWatcher, JsoncError, Layer, LayerError, LayeredConfig, ReloadEvent};
pub use schema::{PropertyKind, PropertySchema};
