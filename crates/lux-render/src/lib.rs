//! GPU rendering: `wgpu` context, surface management, drawing primitives.
//!
//! Deliberately doesn't depend on `lux-platform` — it only needs a
//! `raw-window-handle` target, so any future platform backend (UIKit,
//! Android/JNI) can hand this crate a surface without `lux-render` knowing
//! anything about windowing, IME, or input. `lux-ui` is what wires the two
//! together (see `docs/ARCHITECTURE.md`).

pub mod context;
pub mod error;
pub mod glyph_atlas;
pub mod pipeline;
pub mod schedule;
pub mod surface;
mod tab_stops;
pub mod text_layout;
pub mod text_rendering_mode;

pub use context::GpuContext;
pub use error::RenderError;
pub use glyph_atlas::{GlyphAtlas, GlyphEntry, GlyphKey, GlyphKind};
pub use pipeline::{GlyphInstance, QuadInstance, QuadPipeline, TextPipeline};
pub use schedule::{DirtyRect, RepaintScheduler};
pub use surface::Surface;
pub use text_layout::{ClusterGeometry, LineLayout, LineStyle, TextLayoutContext};
pub use text_rendering_mode::TextRenderingMode;
