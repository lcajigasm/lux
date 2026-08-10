//! Layered JSONC settings, keymaps, and `.editorconfig`/`.gitattributes`
//! support for Lux. `layering` (JSONC parsing, layered precedence, hot
//! reload), `keymap` (keymaps as data, presets, platform resolution,
//! conflict detection), `editorconfig`, and `gitattributes` (line-ending
//! normalization) are implemented; settings importers and a preferences
//! surface are separate Phase 5 deliverables, not yet started.

pub mod editorconfig;
pub mod gitattributes;
pub mod keymap;
pub mod layering;

pub use editorconfig::EditorConfig;
pub use gitattributes::{EolNormalization, GitAttributes, LineEnding};
pub use keymap::{Chord, ChordParseError, ChordSequence, Conflict, KeyBinding, Keymap, Platform};
pub use layering::{ConfigWatcher, JsoncError, Layer, LayerError, LayeredConfig, ReloadEvent};
