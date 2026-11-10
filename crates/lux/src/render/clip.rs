//! CPU scissor for editor quads and glyphs.
//!
//! The quad and text pipelines draw one instanced batch and have no GPU
//! scissor. Clipping here is what keeps a scrolled line inside the text
//! viewport: a glyph that starts in the viewport but bears left is cut,
//! including its atlas UV, so the clipped part is not stretched.

use lux_render::GlyphInstance;

/// Intersection of `rect` with `bounds`, both `[x, y, width, height]`.
pub(crate) fn clip_rect(rect: [f32; 4], bounds: [f32; 4]) -> Option<[f32; 4]> {
    let [rx, ry, rw, rh] = rect;
    let [bx, by, bw, bh] = bounds;
    let x1 = rx.max(bx);
    let y1 = ry.max(by);
    let x2 = (rx + rw).min(bx + bw);
    let y2 = (ry + rh).min(by + bh);
    if x2 > x1 && y2 > y1 {
        Some([x1, y1, x2 - x1, y2 - y1])
    } else {
        None
    }
}

/// Cuts `glyph` to `bounds` and the matching UV rectangle. Returns false
/// when nothing remains.
pub(crate) fn clip_glyph(glyph: &mut GlyphInstance, bounds: [f32; 4]) -> bool {
    let [x, y, w, h] = glyph.rect;
    if w <= f32::EPSILON || h <= f32::EPSILON {
        return false;
    }
    let Some(clipped) = clip_rect(glyph.rect, bounds) else {
        return false;
    };
    let [x1, y1, w1, h1] = clipped;
    let [u0, v0, u1, v1] = glyph.uv_rect;
    let du = u1 - u0;
    let dv = v1 - v0;
    glyph.uv_rect = [
        u0 + du * (x1 - x) / w,
        v0 + dv * (y1 - y) / h,
        u1 - du * ((x + w) - (x1 + w1)) / w,
        v1 - dv * ((y + h) - (y1 + h1)) / h,
    ];
    glyph.rect = clipped;
    true
}

pub(crate) fn clip_glyphs(glyphs: &mut Vec<GlyphInstance>, start: usize, bounds: [f32; 4]) {
    let mut index = start;
    while index < glyphs.len() {
        if clip_glyph(&mut glyphs[index], bounds) {
            index += 1;
        } else {
            glyphs.swap_remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_render::{GlyphInstance, GlyphKind};

    #[test]
    fn a_glyph_that_crosses_the_text_edge_keeps_only_the_visible_part() {
        let mut glyph = GlyphInstance::new(
            [-10.0, 4.0, 30.0, 10.0],
            [0.0, 0.2, 1.0, 0.8],
            [1.0, 1.0, 1.0, 1.0],
            GlyphKind::Monochrome,
        );
        assert!(clip_glyph(&mut glyph, [0.0, 0.0, 100.0, 100.0]));
        assert_eq!(glyph.rect, [0.0, 4.0, 20.0, 10.0]);
        let [u0, _, u1, _] = glyph.uv_rect;
        assert!((u0 - (10.0 / 30.0)).abs() < 1e-5);
        assert!((u1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_glyph_fully_left_of_the_text_edge_is_dropped() {
        let mut glyphs = vec![GlyphInstance::new(
            [-40.0, 0.0, 10.0, 10.0],
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            GlyphKind::Monochrome,
        )];
        clip_glyphs(&mut glyphs, 0, [0.0, 0.0, 100.0, 50.0]);
        assert!(glyphs.is_empty());
    }
}
