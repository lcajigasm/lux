//! Contrast math for painted theme colors: compositing, WCAG ratios, and
//! the selection floor.

use super::Color;

/// Straight-alpha composite of `over` onto `under`, in sRGB. An opaque
/// source is returned with alpha 255. This is the color a selection quad
/// paints on the editor background.
pub fn composite(over: Color, under: Color) -> Color {
    if over.a == 255 {
        return Color::rgb(over.r, over.g, over.b);
    }
    let alpha = f64::from(over.a) / 255.0;
    let mix = |src: u8, dst: u8| -> u8 {
        let mixed = (f64::from(src) * alpha + f64::from(dst) * (1.0 - alpha)).round();
        mixed.clamp(0.0, 255.0) as u8
    };
    Color::rgb(
        mix(over.r, under.r),
        mix(over.g, under.g),
        mix(over.b, under.b),
    )
}

/// Painted selection against the editor background. Sublime Monokai
/// `#49483E` on `#272822` is 1.61. The same RGB at alpha `0x88` paints
/// 1.28. VS Code Monokai `#878b9180` paints about 2.13. 1.30 is above
/// that invisible band and below the other bundled themes that were
/// already distinct.
pub const SELECTION_CONTRAST_MIN: f64 = 1.30;

/// The current-line highlight, pulled toward the page in 5% steps until
/// `selection` painted over it clears [`SELECTION_CONTRAST_MIN`]. The
/// selection paints above the highlight, so on the caret's line it has to
/// stand off the highlight, not just the page. A highlight that already
/// clears the floor is returned unchanged. When even the page does not
/// clear it, the selection itself is under the floor and fading would only
/// lose the current line, so the authored highlight is kept.
pub fn fit_line_highlight(selection: Color, line_highlight: Color, background: Color) -> Color {
    let line = composite(line_highlight, background);
    let mix = |from: u8, to: u8, t: f64| -> u8 {
        (f64::from(from) + (f64::from(to) - f64::from(from)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    for step in 0..=20 {
        let t = f64::from(step) / 20.0;
        let candidate = Color::rgb(
            mix(line.r, background.r, t),
            mix(line.g, background.g, t),
            mix(line.b, background.b, t),
        );
        let painted = composite(selection, candidate);
        if contrast_ratio(painted, candidate) >= SELECTION_CONTRAST_MIN {
            return if step == 0 { line_highlight } else { candidate };
        }
    }
    line_highlight
}

/// Relative-luminance contrast ratio (WCAG 2).
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

fn relative_luminance(c: Color) -> f64 {
    fn chan(u: u8) -> f64 {
        let s = f64::from(u) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * chan(c.r) + 0.7152 * chan(c.g) + 0.0722 * chan(c.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_line_highlight_keeps_a_clear_highlight_and_fades_a_crowded_one() {
        // Monokai already clears the floor: the authored color stays.
        let lh = Color::rgb(0x2C, 0x2D, 0x27);
        let bg = Color::rgb(0x27, 0x28, 0x22);
        assert_eq!(fit_line_highlight(Color::rgb(0x49, 0x48, 0x3E), lh, bg), lh);
        // Nord's selection is 1.17:1 over its own highlight.
        let (sel, lh, bg) = (
            Color::from_hex("#434c5e").unwrap(),
            Color::from_hex("#3b4252").unwrap(),
            Color::from_hex("#2e3440").unwrap(),
        );
        let fitted = fit_line_highlight(sel, lh, bg);
        assert_ne!(fitted, lh);
        assert!(contrast_ratio(sel, fitted) >= SELECTION_CONTRAST_MIN);
        assert!(contrast_ratio(fitted, bg) > 1.0, "still a visible line");
    }
}
