//! Own glyph atlas: `etagere` packs regions, `swash` rasterizes into them.
//!
//! Design fixed by `docs/adr/0003-text-rendering-stack.md`: 4 horizontal
//! subpixel-phase buckets × 1 vertical bucket (glyphs are never repositioned
//! sub-pixel vertically, only horizontally, which is what LCD subpixel AA
//! actually needs), with one packed plane per glyph kind — `Monochrome`,
//! `Polychrome`, `Subpixel` — the same split `gpui` validated in production.
//! This module owns rasterization and packing only. Uploading a plane's
//! bytes to a `wgpu::Texture`, evicting cold entries, and `TextRenderingMode`
//! are the rest of Phase 2's entregable 2 and are deliberately not here yet.

use std::collections::HashMap;

use etagere::{BucketedAtlasAllocator, size2};
use swash::{
    FontRef, GlyphId,
    scale::{Render, ScaleContext, Source, StrikeWith, image::Content},
    zeno::{Format, Vector},
};

/// Horizontal subpixel positions an atlas entry can be rasterized at. LCD
/// subpixel AA looks visibly wrong if a glyph's hinted shape doesn't account
/// for its fractional pen position, but caching one bitmap per exact fractional
/// offset would never hit — so positions are quantized into 4 buckets, same as
/// `gpui`'s atlas. There is deliberately no vertical equivalent (the "×1"):
/// baselines snap to whole pixels, so no vertical bucketing is needed.
pub const SUBPIXEL_PHASES: u8 = 4;

/// Quantizes a fractional horizontal pen offset (`x.fract()`, in `[0, 1)`)
/// into one of [`SUBPIXEL_PHASES`] buckets.
pub fn quantize_phase(x_fract: f32) -> u8 {
    let f = x_fract.rem_euclid(1.0);
    ((f * SUBPIXEL_PHASES as f32) as u8).min(SUBPIXEL_PHASES - 1)
}

/// The rasterization offset a given phase bucket corresponds to.
fn phase_offset(phase: u8) -> f32 {
    phase as f32 / SUBPIXEL_PHASES as f32
}

/// Which of the three atlas planes a rasterized glyph belongs to. Mirrors
/// `swash::scale::image::Content`, named to match the master prompt's
/// vocabulary (`monocromo`/`policromo`/`subpíxel`) since that's the public
/// name this crate exposes above `swash`.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum GlyphKind {
    /// 8-bit alpha mask: plain grayscale antialiased text.
    Monochrome,
    /// 32-bit RGBA bitmap: color glyphs (emoji, COLR/CBDT layers).
    Polychrome,
    /// 32-bit RGBA LCD subpixel mask.
    Subpixel,
}

impl GlyphKind {
    const ALL: [GlyphKind; 3] = [GlyphKind::Monochrome, GlyphKind::Polychrome, GlyphKind::Subpixel];

    fn from_content(content: Content) -> Self {
        match content {
            Content::Mask => GlyphKind::Monochrome,
            Content::Color => GlyphKind::Polychrome,
            Content::SubpixelMask => GlyphKind::Subpixel,
        }
    }

    fn index(self) -> usize {
        match self {
            GlyphKind::Monochrome => 0,
            GlyphKind::Polychrome => 1,
            GlyphKind::Subpixel => 2,
        }
    }

    fn bytes_per_pixel(self) -> u32 {
        match self {
            GlyphKind::Monochrome => 1,
            GlyphKind::Polychrome | GlyphKind::Subpixel => 4,
        }
    }
}

/// Identifies one rasterized glyph: a font, a glyph within it, a size, and a
/// horizontal subpixel phase. Two requests with the same key are guaranteed
/// to produce the same bitmap, which is what makes caching sound.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct GlyphKey {
    font_key: u64,
    glyph_id: GlyphId,
    size_bits: u32,
    phase: u8,
}

impl GlyphKey {
    /// `font_key` should be the requesting `FontRef`'s `key.value()` — unique
    /// per loaded font, so two different fonts never collide even if they
    /// happen to share a glyph id and size.
    pub fn new(font_key: u64, glyph_id: GlyphId, size_ppem: f32, x_fract: f32) -> Self {
        Self {
            font_key,
            glyph_id,
            size_bits: size_ppem.to_bits(),
            phase: quantize_phase(x_fract),
        }
    }
}

/// Where a rasterized glyph landed: its plane, its packed rectangle in that
/// plane, and its placement offset relative to the glyph's pen origin.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GlyphEntry {
    pub kind: GlyphKind,
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

/// One packed texture backing: `etagere` tracks free space, `buffer` is the
/// CPU-side pixel storage a future upload step hands to `wgpu`.
struct Plane {
    allocator: BucketedAtlasAllocator,
    buffer: Vec<u8>,
    side: u32,
    bytes_per_pixel: u32,
}

impl Plane {
    fn new(side: u32, bytes_per_pixel: u32) -> Self {
        Self {
            allocator: BucketedAtlasAllocator::new(size2(side as i32, side as i32)),
            buffer: vec![0u8; (side * side * bytes_per_pixel) as usize],
            side,
            bytes_per_pixel,
        }
    }

    /// Copies a tightly packed `width × height` bitmap into the plane buffer
    /// at `(x, y)`, row by row (the plane's own row stride is `side`, wider
    /// than the glyph's, so rows can't be copied in one slice).
    fn blit(&mut self, x: u32, y: u32, width: u32, height: u32, pixels: &[u8]) {
        let bpp = self.bytes_per_pixel as usize;
        let row_bytes = width as usize * bpp;
        for row in 0..height {
            let src = &pixels[row as usize * row_bytes..(row as usize + 1) * row_bytes];
            let dst_start = ((y + row) * self.side + x) as usize * bpp;
            self.buffer[dst_start..dst_start + row_bytes].copy_from_slice(src);
        }
    }
}

/// A glyph atlas: three `etagere`-packed planes fed by `swash` rasterization,
/// with a lookup cache keyed by [`GlyphKey`] so the same glyph/size/phase is
/// never rasterized or packed twice.
pub struct GlyphAtlas {
    planes: [Plane; 3],
    cache: HashMap<GlyphKey, GlyphEntry>,
}

impl GlyphAtlas {
    /// Creates an atlas with one `side × side` plane per glyph kind.
    pub fn new(side: u32) -> Self {
        Self {
            planes: GlyphKind::ALL.map(|kind| Plane::new(side, kind.bytes_per_pixel())),
            cache: HashMap::new(),
        }
    }

    /// Returns the cached entry for `key`, rasterizing and packing it first
    /// if this is the first time it's been requested. Returns `None` for a
    /// glyph with no visible ink (e.g. space) or once a plane has run out of
    /// room — eviction to reclaim that room is a later loop, not this one.
    pub fn rasterize(
        &mut self,
        scale_context: &mut ScaleContext,
        font: FontRef,
        key: GlyphKey,
    ) -> Option<GlyphEntry> {
        if let Some(entry) = self.cache.get(&key) {
            return Some(*entry);
        }

        let size = f32::from_bits(key.size_bits);
        let mut scaler = scale_context.builder(font).size(size).hint(true).build();
        let offset = Vector::new(phase_offset(key.phase), 0.0);
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::subpixel_bgra())
        .offset(offset)
        .render(&mut scaler, key.glyph_id)?;

        if image.placement.width == 0 || image.placement.height == 0 {
            return None;
        }

        let kind = GlyphKind::from_content(image.content);
        let plane = &mut self.planes[kind.index()];
        let allocation = plane
            .allocator
            .allocate(size2(image.placement.width as i32, image.placement.height as i32))?;
        let atlas_x = allocation.rectangle.min.x as u32;
        let atlas_y = allocation.rectangle.min.y as u32;
        plane.blit(atlas_x, atlas_y, image.placement.width, image.placement.height, &image.data);

        let entry = GlyphEntry {
            kind,
            atlas_x,
            atlas_y,
            width: image.placement.width,
            height: image.placement.height,
            left: image.placement.left,
            top: image.placement.top,
        };
        self.cache.insert(key, entry);
        Some(entry)
    }

    /// The packed CPU-side bitmap for one plane, in row-major
    /// `side × side × bytes_per_pixel` layout — what a future upload step
    /// hands to `wgpu::Queue::write_texture`.
    pub fn plane_bytes(&self, kind: GlyphKind) -> &[u8] {
        &self.planes[kind.index()].buffer
    }

    pub fn plane_side(&self, kind: GlyphKind) -> u32 {
        self.planes[kind.index()].side
    }

    pub fn is_cached(&self, key: &GlyphKey) -> bool {
        self.cache.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Ahem test font (public domain, see `tests/fixtures/README.md`):
    /// every glyph is a solid square filling its em box, so rasterizing any
    /// glyph id is guaranteed to produce real, non-empty, predictable ink —
    /// no dependency on font-rendering taste or hinting quality.
    const AHEM_TTF: &[u8] = include_bytes!("../tests/fixtures/ahem.ttf");

    fn ahem() -> FontRef<'static> {
        FontRef::from_index(AHEM_TTF, 0).expect("ahem.ttf must parse as a valid font")
    }

    fn glyph_for(font: &FontRef, ch: char) -> GlyphId {
        swash::Charmap::from_font(font).map(ch)
    }

    #[test]
    fn quantize_phase_covers_all_four_buckets_across_the_unit_range() {
        assert_eq!(quantize_phase(0.0), 0);
        assert_eq!(quantize_phase(0.24), 0);
        assert_eq!(quantize_phase(0.25), 1);
        assert_eq!(quantize_phase(0.49), 1);
        assert_eq!(quantize_phase(0.5), 2);
        assert_eq!(quantize_phase(0.74), 2);
        assert_eq!(quantize_phase(0.75), 3);
        assert_eq!(quantize_phase(0.999), 3);
    }

    #[test]
    fn quantize_phase_wraps_negative_and_out_of_range_fractions() {
        assert_eq!(quantize_phase(-0.1), quantize_phase(0.9));
        assert_eq!(quantize_phase(1.0), quantize_phase(0.0));
    }

    #[test]
    fn rasterizing_the_same_key_twice_hits_the_cache_instead_of_repacking() {
        let font = ahem();
        let glyph_id = glyph_for(&font, 'A');
        let key = GlyphKey::new(font.key.value(), glyph_id, 32.0, 0.0);
        let mut atlas = GlyphAtlas::new(256);
        let mut ctx = ScaleContext::new();

        assert!(!atlas.is_cached(&key));
        let first = atlas.rasterize(&mut ctx, font, key).expect("Ahem glyphs are solid ink");
        assert!(atlas.is_cached(&key));
        let second = atlas.rasterize(&mut ctx, font, key).expect("cache hit must still return the entry");
        assert_eq!(first, second, "same key must yield the exact same packed location");
    }

    #[test]
    fn different_subpixel_phases_of_the_same_glyph_pack_distinct_atlas_slots() {
        let font = ahem();
        let glyph_id = glyph_for(&font, 'A');
        let mut atlas = GlyphAtlas::new(256);
        let mut ctx = ScaleContext::new();

        let mut positions = Vec::new();
        for phase in 0..SUBPIXEL_PHASES {
            let x_fract = phase as f32 / SUBPIXEL_PHASES as f32;
            let key = GlyphKey::new(font.key.value(), glyph_id, 32.0, x_fract);
            let entry = atlas.rasterize(&mut ctx, font, key).expect("Ahem glyphs are solid ink");
            positions.push((entry.atlas_x, entry.atlas_y));
        }
        let mut unique = positions.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            SUBPIXEL_PHASES as usize,
            "all 4 horizontal subpixel phases must occupy distinct atlas rectangles: {positions:?}"
        );
    }

    #[test]
    fn rasterized_glyph_is_packed_into_the_subpixel_plane_by_default() {
        let font = ahem();
        let glyph_id = glyph_for(&font, 'A');
        let key = GlyphKey::new(font.key.value(), glyph_id, 32.0, 0.0);
        let mut atlas = GlyphAtlas::new(256);
        let mut ctx = ScaleContext::new();

        let entry = atlas.rasterize(&mut ctx, font, key).unwrap();
        assert_eq!(entry.kind, GlyphKind::Subpixel);
        assert!(entry.width > 0 && entry.height > 0);

        let bytes = atlas.plane_bytes(GlyphKind::Subpixel);
        let side = atlas.plane_side(GlyphKind::Subpixel);
        let px = |x: u32, y: u32| {
            let i = ((y * side + x) * 4) as usize;
            &bytes[i..i + 4]
        };
        // Ahem's glyphs are solid ink; the packed rectangle's interior must
        // not be all zero, or rasterization silently produced nothing.
        let center = px(entry.atlas_x + entry.width / 2, entry.atlas_y + entry.height / 2);
        assert!(center.iter().any(|&b| b != 0), "expected solid ink at the glyph's center, got {center:?}");
    }

    #[test]
    fn requesting_alpha_format_directly_packs_into_the_monochrome_plane() {
        // `rasterize()` always asks for subpixel format; this test exercises
        // the Monochrome plane's packing/blit path directly, the same way
        // `rasterize()` would if `TextRenderingMode::Grayscale` requested
        // `Format::Alpha` instead (that mode selection is a later loop).
        let font = ahem();
        let glyph_id = glyph_for(&font, 'A');
        let mut ctx = ScaleContext::new();
        let mut scaler = ctx.builder(font).size(32.0).hint(true).build();
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, glyph_id)
            .expect("Ahem glyphs are solid ink");
        assert_eq!(image.content, Content::Mask);
        assert_eq!(GlyphKind::from_content(image.content), GlyphKind::Monochrome);

        let mut plane = Plane::new(256, GlyphKind::Monochrome.bytes_per_pixel());
        plane.blit(0, 0, image.placement.width, image.placement.height, &image.data);
        assert!(
            plane.buffer.iter().any(|&b| b != 0),
            "monochrome plane must contain packed ink"
        );
    }

    #[test]
    fn polychrome_plane_packs_four_byte_rgba_pixels() {
        // No color font is bundled as a test fixture (Ahem has no COLR/CBDT
        // layers), so this exercises the Polychrome plane's packing/blit path
        // directly with synthetic RGBA data instead of a real color glyph.
        let mut plane = Plane::new(64, GlyphKind::Polychrome.bytes_per_pixel());
        let pixel = [10u8, 20, 30, 255];
        let rgba: Vec<u8> = pixel.iter().copied().cycle().take(2 * 2 * 4).collect();
        plane.blit(3, 5, 2, 2, &rgba);

        let side = plane.side;
        let at = |x: u32, y: u32| {
            let i = ((y * side + x) * 4) as usize;
            &plane.buffer[i..i + 4]
        };
        assert_eq!(at(3, 5), &pixel);
        assert_eq!(at(4, 6), &pixel);
        assert_eq!(at(0, 0), &[0, 0, 0, 0], "outside the blit target must stay untouched");
    }

    #[test]
    fn plane_runs_out_of_room_and_reports_it_instead_of_overwriting() {
        let font = ahem();
        let a = glyph_for(&font, 'A');
        let b = glyph_for(&font, 'B');
        let key_a = GlyphKey::new(font.key.value(), a, 32.0, 0.0);
        let key_b = GlyphKey::new(font.key.value(), b, 32.0, 0.0);
        let mut ctx = ScaleContext::new();

        // Ahem's glyphs are uniform solid squares, so probing one glyph's
        // pixel size in a roomy atlas tells us exactly how large a plane
        // needs to be to fit exactly one glyph and no more, regardless of
        // this machine's hinting/rounding — no dimension is hardcoded.
        let mut probe = GlyphAtlas::new(256);
        let probe_entry = probe.rasterize(&mut ctx, font, key_a).expect("Ahem glyphs are solid ink");
        let side = probe_entry.width.max(probe_entry.height);

        let mut atlas = GlyphAtlas::new(side);
        let first = atlas.rasterize(&mut ctx, font, key_a);
        let second = atlas.rasterize(&mut ctx, font, key_b);
        assert!(first.is_some());
        assert!(second.is_none(), "a full plane must return None, never silently corrupt packing");
    }
}
