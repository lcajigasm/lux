//! Layered JSONC settings and keymaps for Lux. `layering` (JSONC parsing,
//! layered precedence, hot reload) and `keymap` (keymaps as data, presets,
//! platform resolution, conflict detection) are implemented;
//! `.editorconfig`/`.gitattributes` support, settings importers, and a
//! preferences surface are separate Phase 5 deliverables, not yet started.

pub mod keymap;
pub mod layering;

pub use keymap::{Chord, ChordParseError, ChordSequence, Conflict, KeyBinding, Keymap, Platform};
pub use layering::{ConfigWatcher, JsoncError, Layer, LayerError, LayeredConfig, ReloadEvent};
