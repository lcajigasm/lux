//! Color chips on the Settings theme menu.
//!
//! Each bundled theme shows its background, foreground, accent, and keyword
//! color. A name that is not a bundled scheme draws nothing.

use lux_render::QuadInstance;

use crate::color_scheme::{LoadedScheme, scheme_named};
use crate::render::paint::color;
use crate::settings::layout::Rect;

pub(crate) fn paint_bundled(name: &str, rect: Rect, scale: f32, quads: &mut Vec<QuadInstance>) {
    paint(
        crate::color_scheme::bundled_schemes(),
        name,
        rect,
        scale,
        quads,
    );
}

pub(crate) fn paint(
    schemes: &[LoadedScheme],
    name: &str,
    rect: Rect,
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) {
    let Some(scheme) = scheme_named(schemes, name) else {
        return;
    };
    let keyword = scheme.syntax.color_for_scope("keyword");
    let chips = [
        color(scheme.theme.background),
        color(scheme.theme.foreground),
        color(scheme.theme.accent),
        [
            keyword.r as f32 / 255.0,
            keyword.g as f32 / 255.0,
            keyword.b as f32 / 255.0,
            1.0,
        ],
    ];
    let size = 10.0 * scale.max(1.0);
    let gap = 3.0 * scale.max(1.0);
    let total = chips.len() as f32 * size + (chips.len() as f32 - 1.0) * gap;
    let mut x = rect.x + rect.w - total - 8.0 * scale.max(1.0);
    let y = rect.y + (rect.h - size) * 0.5;
    for chip in chips {
        quads.push(QuadInstance::solid([x, y, size, size], chip));
        x += size + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monokai_menu_chips_use_the_official_background_and_keyword() {
        let schemes = crate::color_scheme::bundled_schemes();
        let scheme = scheme_named(schemes, "Monokai").expect("bundled Monokai");
        assert_eq!(
            (
                scheme.theme.background.r,
                scheme.theme.background.g,
                scheme.theme.background.b
            ),
            (0x27, 0x28, 0x22)
        );
        let keyword = scheme.syntax.color_for_scope("keyword");
        assert_eq!((keyword.r, keyword.g, keyword.b), (0xF9, 0x26, 0x72));
        let mut quads = Vec::new();
        paint(
            schemes,
            "Monokai",
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 24.0,
            },
            1.0,
            &mut quads,
        );
        assert_eq!(quads.len(), 4);
        paint(
            schemes,
            "not a theme",
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 24.0,
            },
            1.0,
            &mut quads,
        );
        assert_eq!(quads.len(), 4);
    }
}
