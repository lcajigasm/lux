//! Keymaps as data (Phase 5 deliverable 2, narrowed scope): a chord/binding
//! model shared by all three shipped presets (Sublime Text, VS Code,
//! JetBrains — see `presets.rs`), per-platform resolution (`mac`/`win`/
//! `linux` overrides, mirroring VS Code's own real `keybindings.json`
//! shape), chord chaining (`ChordSequence`, exercised by two of VS Code's
//! real default two-chord bindings), and conflict detection with a visible
//! diagnostic (`conflicts.rs`).
//!
//! Not in scope here: actually dispatching a key event to a command — no
//! input/dispatch layer exists yet for this to hook into, wiring that up
//! is a future integration's job, not this data model's; and a real
//! `when`-clause expression language (`conflicts.rs` explains why treating
//! it as opaque is a deliberate simplification, not an oversight).

pub mod binding;
pub mod chord;
pub mod conflicts;
pub mod presets;

pub use binding::{KeyBinding, Keymap, Platform};
pub use chord::{Chord, ChordParseError, ChordSequence};
pub use conflicts::{Conflict, conflicts};
