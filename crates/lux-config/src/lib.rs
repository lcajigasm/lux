//! Layered JSONC settings for Lux. `layering` (this phase's scope: JSONC
//! parsing, layered precedence, hot reload) is implemented; keymaps,
//! `.editorconfig`/`.gitattributes` support, settings importers, and a
//! preferences surface are separate Phase 5 deliverables, not yet started.

pub mod layering;

pub use layering::{ConfigWatcher, JsoncError, Layer, LayerError, LayeredConfig, ReloadEvent};
