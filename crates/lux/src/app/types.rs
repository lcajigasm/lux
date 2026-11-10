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
    /// Set once a cursor event has landed inside the view. Until then the
    /// position stays at the origin, which is also the tab strip, so a wheel
    /// event must not be read as "the pointer is on the tabs".
    pub(crate) placed: bool,
    pub(crate) selection_anchor: Option<Position>,
    pub(crate) last_click: Option<(std::time::Instant, usize)>,
}

pub(crate) struct SyntaxState {
    pub(crate) highlighter: lux_syntax::BackgroundHighlighter,
    pub(crate) language: lux_syntax::LanguageId,
    pub(crate) theme: &'static lux_syntax::theme::SyntaxTheme,
    pub(crate) tokens: Vec<lux_syntax::Token>,
    pub(crate) fold_ranges: Vec<lux_core::folding::FoldRange>,
    pub(crate) version: usize,
}

pub(crate) struct AtlasState {
    pub(crate) atlas: GlyphAtlas,
    pub(crate) dirty: bool,
    pub(crate) scale_cx: ScaleContext,
}
