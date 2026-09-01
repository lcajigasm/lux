//! Shared glyph/quad color helpers used by editor and chrome paint.

use lux_render::{GlyphInstance, GlyphKey, TextRenderingMode};
use swash::FontRef;

use crate::app::ATLAS_SIDE;

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_glyph(
    atlas: &mut lux_render::GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut swash::scale::ScaleContext,
    font: FontRef,
    cluster: &lux_render::ClusterGeometry,
    origin_x: f32,
    origin_y: f32,
    font_size: f32,
    color: [f32; 4],
    out: &mut Vec<GlyphInstance>,
) {
    let Some(glyph_id) = cluster.glyph_id else {
        return;
    };
    // TrueType/OpenType glyph indices are defined as 16-bit by the font
    // format spec itself; `parley::Glyph::id` is a `u32` for API-level
    // headroom, but no real font can produce a value this cast would
    // truncate.
    let key = GlyphKey::new(
        font.key.value(),
        glyph_id as u16,
        font_size,
        cluster.x.fract(),
        TextRenderingMode::Subpixel,
    );
    let was_cached = atlas.is_cached(&key);
    let Some(entry) = atlas.rasterize(scale_cx, font, key) else {
        return; // no visible ink (e.g. a space) — nothing to draw
    };
    if !was_cached {
        *atlas_dirty = true;
    }
    let side = ATLAS_SIDE as f32;
    let uv = [
        entry.atlas_x as f32 / side,
        entry.atlas_y as f32 / side,
        (entry.atlas_x + entry.width) as f32 / side,
        (entry.atlas_y + entry.height) as f32 / side,
    ];
    out.push(GlyphInstance::new(
        [
            origin_x + cluster.x,
            origin_y,
            entry.width as f32,
            entry.height as f32,
        ],
        uv,
        color,
        entry.kind,
    ));
}

pub(crate) fn color(c: lux_ui::Color) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}
