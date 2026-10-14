//! Supporting state structs for the Lux desktop application.

use lux_core::position::Position;
use lux_platform::{PlatformWindow, WindowId};
use lux_render::{GlyphAtlas, GpuContext, QuadPipeline, Surface, TextPipeline};
use swash::scale::ScaleContext;

/// Everything that needs a live GPU device/surface/window — created once,
/// in `resumed`, and gone if the app is ever suspended.
pub(crate) struct GpuWindow {
    pub(crate) window: &'static dyn PlatformWindow,
    pub(crate) id: WindowId,
    pub(crate) gpu: GpuContext,
    pub(crate) surface: Surface<'static>,
    pub(crate) quads: QuadPipeline,
    pub(crate) text: TextPipeline,
}

pub(crate) struct MouseState {
    pub(crate) position: dpi::PhysicalPosition<f64>,
    pub(crate) selection_anchor: Option<Position>,
    pub(crate) last_click: Option<(std::time::Instant, usize)>,
}

pub(crate) struct SyntaxState {
    pub(crate) document: Option<lux_syntax::Document>,
    pub(crate) source: String,
    pub(crate) theme: &'static lux_syntax::theme::SyntaxTheme,
}

pub(crate) struct AtlasState {
    pub(crate) atlas: GlyphAtlas,
    pub(crate) dirty: bool,
    pub(crate) scale_cx: ScaleContext,
}
