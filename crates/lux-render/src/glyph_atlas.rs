//! Own glyph atlas: `etagere` packs regions, `swash` rasterizes into them.
//!
//! Design fixed by `docs/adr/0003-text-rendering-stack.md`: 4 horizontal
//! subpixel-phase buckets × 1 vertical bucket (glyphs are never repositioned
//! sub-pixel vertically, only horizontally, which is what LCD subpixel AA
//! actually needs), with one packed plane per glyph kind — `Monochrome`,
//! `Polychrome`, `Subpixel` — the same split `gpui` validated in production.
//! Real LRU eviction (below) means a plane running out of room is not the
//! end of the story: the coldest entry in that same plane is deallocated
//! and the request retried, in a loop, until it fits or the plane's own
//! cache is empty (in which case the glyph is simply too large for an empty
//! plane — a genuine case, not a bug).

use std::collections::HashMap;

use etagere::{AllocId, BucketedAtlasAllocator, size2};
use swash::{
    FontRef, GlyphId,
    scale::{Render, ScaleContext, Source, StrikeWith, image::Content},
    zeno::Vector,
};

use crate::text_rendering_mode::{TextRenderingMode, format_for};

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
    const ALL: [GlyphKind; 3] = [
        GlyphKind::Monochrome,
        GlyphKind::Polychrome,
        GlyphKind::Subpixel,
    ];

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

/// Identifies one rasterized glyph: a font, a glyph within it, a size, a
/// horizontal subpixel phase, and whether it was rasterized grayscale.
/// Two requests with the same key are guaranteed to produce the same
/// bitmap, which is what makes caching sound. The grayscale bit must be
/// part of the key — otherwise flipping `TextRenderingMode` at runtime
/// could return a stale bitmap rasterized under the previous mode.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct GlyphKey {
    font_key: u64,
    glyph_id: GlyphId,
    size_bits: u32,
    phase: u8,
    grayscale: bool,
}

impl GlyphKey {
    /// `font_key` should be the requesting `FontRef`'s `key.value()` — unique
    /// per loaded font, so two different fonts never collide even if they
    /// happen to share a glyph id and size.
    pub fn new(
        font_key: u64,
        glyph_id: GlyphId,
        size_ppem: f32,
        x_fract: f32,
        mode: TextRenderingMode,
    ) -> Self {
        Self {
            font_key,
            glyph_id,
            size_bits: size_ppem.to_bits(),
            phase: quantize_phase(x_fract),
            grayscale: mode.is_grayscale(),
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

/// A cached entry plus what eviction needs: the `etagere` allocation it
/// occupies (to free on eviction) and a "last used" tick (to pick which
/// entry is coldest).
struct CacheEntry {
    entry: GlyphEntry,
    alloc_id: AllocId,
    last_used: u64,
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
/// never rasterized or packed twice, and real LRU eviction so a long-running
/// editor session never runs a plane out of room permanently.
pub struct GlyphAtlas {
    planes: [Plane; 3],
    cache: HashMap<GlyphKey, CacheEntry>,
    /// Monotonic tick, bumped on every access. Not wall-clock time — this
    /// only needs to order accesses relative to each other, and a counter
    /// can never go backwards or stall the way a clock could.
    clock: u64,
}

impl GlyphAtlas {
    /// Creates an atlas with one `side × side` plane per glyph kind.
    pub fn new(side: u32) -> Self {
        Self {
            planes: GlyphKind::ALL.map(|kind| Plane::new(side, kind.bytes_per_pixel())),
            cache: HashMap::new(),
            clock: 0,
        }
    }

    /// Returns the cached entry for `key`, rasterizing and packing it first
    /// if this is the first time it's been requested. Returns `None` only
    /// when the glyph has no visible ink (e.g. space) or is too large to
    /// ever fit even an empty plane — a full plane on its own is never a
    /// reason to fail, since [`Self::allocate_with_eviction`] evicts the
    /// coldest entry and retries.
    pub fn rasterize(
        &mut self,
        scale_context: &mut ScaleContext,
        font: FontRef,
        key: GlyphKey,
    ) -> Option<GlyphEntry> {
        self.clock += 1;
        let now = self.clock;

        if let Some(cached) = self.cache.get_mut(&key) {
            cached.last_used = now;
            return Some(cached.entry);
        }

        let size = f32::from_bits(key.size_bits);
        let mut scaler = scale_context.builder(font).size(size).hint(true).build();
        let offset = Vector::new(phase_offset(key.phase), 0.0);
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(format_for(key.grayscale))
        .offset(offset)
        .render(&mut scaler, key.glyph_id)?;

        if image.placement.width == 0 || image.placement.height == 0 {
            return None;
        }

        let kind = GlyphKind::from_content(image.content);
        let (alloc_id, atlas_x, atlas_y) =
            self.allocate_with_eviction(kind, image.placement.width, image.placement.height)?;
        self.planes[kind.index()].blit(
            atlas_x,
            atlas_y,
            image.placement.width,
            image.placement.height,
            &image.data,
        );

        let entry = GlyphEntry {
            kind,
            atlas_x,
            atlas_y,
            width: image.placement.width,
            height: image.placement.height,
            left: image.placement.left,
            top: image.placement.top,
        };
        self.cache.insert(
            key,
            CacheEntry {
                entry,
                alloc_id,
                last_used: now,
            },
        );
        Some(entry)
    }

    /// Allocates `width × height` in the plane for `kind`, evicting the
    /// coldest cached entry belonging to that same plane and retrying when
    /// it's full — in a loop, until the allocation succeeds or that plane's
    /// cache runs out of entries to evict (the glyph is simply too big for
    /// an empty plane).
    fn allocate_with_eviction(
        &mut self,
        kind: GlyphKind,
        width: u32,
        height: u32,
    ) -> Option<(AllocId, u32, u32)> {
        let idx = kind.index();
        let requested = size2(width as i32, height as i32);
        loop {
            if let Some(alloc) = self.planes[idx].allocator.allocate(requested) {
                return Some((
                    alloc.id,
                    alloc.rectangle.min.x as u32,
                    alloc.rectangle.min.y as u32,
                ));
            }

            let coldest = self
                .cache
                .iter()
                .filter(|(_, cached)| cached.entry.kind == kind)
                .min_by_key(|(_, cached)| cached.last_used)
                .map(|(key, _)| *key)?;
            let evicted = self
                .cache
                .remove(&coldest)
                .expect("coldest key was just found in this same cache");
            self.planes[idx].allocator.deallocate(evicted.alloc_id);
        }
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

    fn key(font: &FontRef, glyph_id: GlyphId, size: f32, x_fract: f32) -> GlyphKey {
        GlyphKey::new(
            font.key.value(),
            glyph_id,
            size,
            x_fract,
            TextRenderingMode::Subpixel,
        )
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
        let k = key(&font, glyph_id, 32.0, 0.0);
        let mut atlas = GlyphAtlas::new(256);
        let mut ctx = ScaleContext::new();

        assert!(!atlas.is_cached(&k));
        let first = atlas
            .rasterize(&mut ctx, font, k)
            .expect("Ahem glyphs are solid ink");
        assert!(atlas.is_cached(&k));
        let second = atlas
            .rasterize(&mut ctx, font, k)
            .expect("cache hit must still return the entry");
        assert_eq!(
            first, second,
            "same key must yield the exact same packed location"
        );
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
            let k = key(&font, glyph_id, 32.0, x_fract);
            let entry = atlas
                .rasterize(&mut ctx, font, k)
                .expect("Ahem glyphs are solid ink");
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
        let k = key(&font, glyph_id, 32.0, 0.0);
        let mut atlas = GlyphAtlas::new(256);
        let mut ctx = ScaleContext::new();

        let entry = atlas.rasterize(&mut ctx, font, k).unwrap();
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
        let center = px(
            entry.atlas_x + entry.width / 2,
            entry.atlas_y + entry.height / 2,
        );
        assert!(
            center.iter().any(|&b| b != 0),
            "expected solid ink at the glyph's center, got {center:?}"
        );
    }

    #[test]
    fn grayscale_mode_packs_into_the_monochrome_plane_via_rasterize() {
        let font = ahem();
        let glyph_id = glyph_for(&font, 'A');
        let k = GlyphKey::new(
            font.key.value(),
            glyph_id,
            32.0,
            0.0,
            TextRenderingMode::Grayscale,
        );
        let mut atlas = GlyphAtlas::new(256);
        let mut ctx = ScaleContext::new();

        let entry = atlas
            .rasterize(&mut ctx, font, k)
            .expect("Ahem glyphs are solid ink");
        assert_eq!(entry.kind, GlyphKind::Monochrome);

        let bytes = atlas.plane_bytes(GlyphKind::Monochrome);
        assert!(
            bytes.iter().any(|&b| b != 0),
            "monochrome plane must contain packed ink"
        );
    }

    #[test]
    fn grayscale_and_subpixel_requests_for_the_same_glyph_are_cached_separately() {
        let font = ahem();
        let glyph_id = glyph_for(&font, 'A');
        let subpixel_key = key(&font, glyph_id, 32.0, 0.0);
        let grayscale_key = GlyphKey::new(
            font.key.value(),
            glyph_id,
            32.0,
            0.0,
            TextRenderingMode::Grayscale,
        );
        assert_ne!(
            subpixel_key, grayscale_key,
            "the rendering mode must be part of the cache key"
        );

        let mut atlas = GlyphAtlas::new(256);
        let mut ctx = ScaleContext::new();
        let subpixel_entry = atlas.rasterize(&mut ctx, font, subpixel_key).unwrap();
        let grayscale_entry = atlas.rasterize(&mut ctx, font, grayscale_key).unwrap();
        assert_eq!(subpixel_entry.kind, GlyphKind::Subpixel);
        assert_eq!(grayscale_entry.kind, GlyphKind::Monochrome);
        assert!(atlas.is_cached(&subpixel_key));
        assert!(atlas.is_cached(&grayscale_key));
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
        assert_eq!(
            at(0, 0),
            &[0, 0, 0, 0],
            "outside the blit target must stay untouched"
        );
    }

    #[test]
    fn requesting_a_second_glyph_evicts_the_coldest_entry_when_the_plane_is_full() {
        let font = ahem();
        let a = glyph_for(&font, 'A');
        let b = glyph_for(&font, 'B');
        let key_a = key(&font, a, 32.0, 0.0);
        let key_b = key(&font, b, 32.0, 0.0);
        let mut ctx = ScaleContext::new();

        // Ahem's glyphs are uniform solid squares, so probing one glyph's
        // pixel size in a roomy atlas tells us exactly how large a plane
        // needs to be to fit exactly one glyph and no more, regardless of
        // this machine's hinting/rounding — no dimension is hardcoded.
        let mut probe = GlyphAtlas::new(256);
        let probe_entry = probe
            .rasterize(&mut ctx, font, key_a)
            .expect("Ahem glyphs are solid ink");
        let side = probe_entry.width.max(probe_entry.height);

        let mut atlas = GlyphAtlas::new(side);
        let first = atlas.rasterize(&mut ctx, font, key_a);
        assert!(first.is_some());
        assert!(atlas.is_cached(&key_a));

        let second = atlas.rasterize(&mut ctx, font, key_b);
        assert!(
            second.is_some(),
            "a full plane must evict its coldest entry and retry, not fail"
        );
        assert!(
            !atlas.is_cached(&key_a),
            "the evicted glyph must no longer report as cached"
        );
        assert!(atlas.is_cached(&key_b));
    }

    #[test]
    fn a_glyph_larger_than_an_empty_plane_returns_none_instead_of_looping_forever() {
        let font = ahem();
        let glyph_id = glyph_for(&font, 'A');
        let k = key(&font, glyph_id, 32.0, 0.0);
        let mut ctx = ScaleContext::new();

        // A 1x1 plane can never fit any real Ahem glyph at this size, and
        // there is nothing cached yet to evict — this must terminate with
        // `None`, not loop forever.
        let mut atlas = GlyphAtlas::new(1);
        assert!(atlas.rasterize(&mut ctx, font, k).is_none());
    }
}
