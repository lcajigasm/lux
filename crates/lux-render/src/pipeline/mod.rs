//! Real `wgpu` quad and text render pipelines — Phase 2 deliverable 2's
//! remaining pieces (contexto `wgpu`, superficie/resize, and the glyph
//! atlas were earlier slices of this same deliverable, already closed).
//!
//! [`quad::QuadPipeline`] draws solid-color rectangles (background,
//! selection, cursor, indent guides, column rulers). [`text::TextPipeline`]
//! draws instanced, textured glyph quads sampling
//! [`crate::GlyphAtlas`]'s three planes, compositing each of the three
//! [`crate::GlyphKind`] variants correctly (see `text.rs`'s own doc
//! comment for how subpixel LCD compositing works without dual-source
//! blending). Neither pipeline knows anything about `lux-core` text or
//! `lux-ui` layout — they draw whatever rects/instances a caller (`lux-ui`,
//! eventually) hands them, the same "own only the GPU-facing half"
//! separation `GpuContext`/`Surface` already established.

pub mod quad;
pub mod text;

pub use quad::{QuadInstance, QuadPipeline};
pub use text::{GlyphInstance, TextPipeline};
