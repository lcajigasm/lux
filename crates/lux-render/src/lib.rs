//! GPU rendering: `wgpu` context, surface management, drawing primitives.
//!
//! Deliberately doesn't depend on `lux-platform` — it only needs a
//! `raw-window-handle` target, so any future platform backend (UIKit,
//! Android/JNI) can hand this crate a surface without `lux-render` knowing
//! anything about windowing, IME, or input. `lux-ui` is what wires the two
//! together (see `docs/ARCHITECTURE.md`).

pub mod context;
pub mod error;
pub mod surface;

pub use context::GpuContext;
pub use error::RenderError;
pub use surface::Surface;
