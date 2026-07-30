//! How glyphs get rasterized — Phase 2's `TextRenderingMode` deliverable,
//! split out of `glyph_atlas.rs` only to stay under the 600-line file
//! limit (`scripts/check-limits.sh`); it's part of the same atlas design,
//! not a separate concern.

use swash::zeno::Format;

/// Full LCD subpixel AA, plain grayscale, or "whatever this platform's
/// native text looks like."
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum TextRenderingMode {
    /// `Subpixel` today on every platform this runs on (only macOS exists
    /// yet — see `docs/adr/0003-text-rendering-stack.md`). Windows/Linux
    /// don't have a validated native-matching default of their own: there's
    /// no environment here to measure one against (this entregable's
    /// receipt documents the gap). Not a per-platform branch yet, just one
    /// fixed choice standing in for one.
    #[default]
    PlatformDefault,
    /// Full LCD subpixel antialiasing (`swash::zeno::Format::subpixel_bgra()`).
    Subpixel,
    /// Plain 8-bit grayscale antialiasing (`Format::Alpha`) — no subpixel
    /// fringing, the right choice on a screen where LCD subpixel geometry
    /// doesn't apply (rotated displays, some external monitors) or when a
    /// user just prefers it.
    Grayscale,
}

impl TextRenderingMode {
    pub(crate) fn is_grayscale(self) -> bool {
        matches!(self, TextRenderingMode::Grayscale)
    }
}

pub(crate) fn format_for(grayscale: bool) -> Format {
    if grayscale {
        Format::Alpha
    } else {
        Format::subpixel_bgra()
    }
}
