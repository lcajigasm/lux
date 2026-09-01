//! Lux Dark: the only theme for now (Phase 2 deliverable 7). Carries
//! forward v0's visual identity verbatim — indigo-black base `#12121B`,
//! golden-amber accent `#F0B442` (`LUX_PROMPT_MAESTRO.md` §0.6: "Lux es
//! luz, y el acento cálido es el sello de la marca. Consérvala.") — rather
//! than reinventing it. A full token/scope-based theme system with Light
//! and high-contrast variants is Phase 3's job (deliverable 2); this is
//! deliberately just a small, fixed palette, not a configurable one.

/// An sRGB color with straight (non-premultiplied) alpha, 8 bits per
/// channel — the layout a `wgpu` texture upload will want once the
/// quad/text render pipeline exists (it doesn't yet; see this
/// deliverable's receipt for what "golden capture" means without one).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xFF }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses a `#RRGGBB` or `#RRGGBBAA` hex string — the format the v0
    /// identity's own colors are documented in. Returns `None` for
    /// anything else rather than guessing.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        let byte = |s: &str| u8::from_str_radix(s, 16).ok();
        match hex.len() {
            6 => Some(Self::rgb(
                byte(&hex[0..2])?,
                byte(&hex[2..4])?,
                byte(&hex[4..6])?,
            )),
            8 => Some(Self::rgba(
                byte(&hex[0..2])?,
                byte(&hex[2..4])?,
                byte(&hex[4..6])?,
                byte(&hex[6..8])?,
            )),
            _ => None,
        }
    }
}

/// The smallest palette that makes "Lux Dark" an actual usable theme
/// rather than two floating constants: every editor needs a background
/// *and* a foreground, and `editor_view` already models cursors
/// ([`crate::CursorGlyph`]) and selections ([`crate::SelectionRect`]) that
/// will need a color once something paints them. Only `background` and
/// `accent` are part of the documented v0 identity; `foreground`,
/// `selection`, and `muted` are this deliverable's own reasonable
/// completions, not values pulled from a spec — a full token/scope theme
/// (comments, strings, brackets, diagnostics...) is Phase 3's job.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// The editor's base fill — v0's indigo-black, `#12121B`.
    pub background: Color,
    /// Body text color, legible against `background`.
    pub foreground: Color,
    /// v0's golden amber, `#F0B442` — the brand's warm accent. Used for
    /// the caret and other highlights that should read as "Lux," not just
    /// "a dark theme."
    pub accent: Color,
    /// Selection highlight: `accent` at low alpha, so selected text stays
    /// readable instead of being fully painted over.
    pub selection: Color,
    /// De-emphasized text (e.g. line numbers, comments once Phase 3's
    /// token theme exists) — `foreground` dimmed toward `background`.
    pub muted: Color,
}

impl Theme {
    /// The v0 identity, carried forward per `LUX_PROMPT_MAESTRO.md`'s
    /// Phase 2 deliverable 7.
    pub const LUX_DARK: Theme = Theme {
        background: Color::rgb(0x12, 0x12, 0x1B),
        foreground: Color::rgb(0xE6, 0xE6, 0xEB),
        accent: Color::rgb(0xF0, 0xB4, 0x42),
        selection: Color::rgba(0xF0, 0xB4, 0x42, 0x33),
        muted: Color::rgb(0x6E, 0x6E, 0x82),
    };

    /// High-contrast dark: near-black / near-white, WCAG AA for body text.
    pub const LUX_DARK_HIGH_CONTRAST: Theme = Theme {
        background: Color::rgb(0x00, 0x00, 0x00),
        foreground: Color::rgb(0xFF, 0xFF, 0xFF),
        accent: Color::rgb(0xFF, 0xD0, 0x00),
        selection: Color::rgba(0xFF, 0xD0, 0x00, 0x66),
        muted: Color::rgb(0xC0, 0xC0, 0xC0),
    };
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

/// Side length, in pixels, of each solid color square in [`capture_swatch`].
pub const SWATCH_SIZE: u32 = 8;
const SWATCH_COUNT: u32 = 5;

/// Renders a tiny, fixed-layout strip of solid squares — background,
/// foreground, accent, selection, muted, left to right — as a tightly
/// packed RGBA8 buffer (`wgpu::Queue::write_texture`'s own layout). This is
/// what a "golden capture" of a theme can mean without an actual `wgpu`
/// framebuffer, which doesn't exist yet: a deterministic, real pixel
/// buffer derived only from `theme`'s actual color values, suitable for
/// exact byte comparison in a test — the same "tolerance" a real
/// framebuffer golden test would use is simply zero here, since flat color
/// fills have no antialiasing to tolerate.
pub fn capture_swatch(theme: &Theme) -> Vec<u8> {
    let colors = [
        theme.background,
        theme.foreground,
        theme.accent,
        theme.selection,
        theme.muted,
    ];
    let width = SWATCH_SIZE * SWATCH_COUNT;
    let mut buffer = vec![0u8; (width * SWATCH_SIZE * 4) as usize];
    for y in 0..SWATCH_SIZE {
        for (i, color) in colors.iter().enumerate() {
            for x in 0..SWATCH_SIZE {
                let px = i as u32 * SWATCH_SIZE + x;
                let offset = ((y * width + px) * 4) as usize;
                buffer[offset] = color.r;
                buffer[offset + 1] = color.g;
                buffer[offset + 2] = color.b;
                buffer[offset + 3] = color.a;
            }
        }
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lux_dark_matches_the_master_prompts_own_hex_codes_verbatim() {
        assert_eq!(
            Color::from_hex("#12121B").unwrap(),
            Theme::LUX_DARK.background
        );
        assert_eq!(Color::from_hex("#F0B442").unwrap(), Theme::LUX_DARK.accent);
    }

    #[test]
    fn from_hex_parses_with_and_without_a_leading_hash() {
        assert_eq!(Color::from_hex("#112233"), Color::from_hex("112233"));
        assert_eq!(
            Color::from_hex("112233").unwrap(),
            Color::rgb(0x11, 0x22, 0x33)
        );
    }

    #[test]
    fn from_hex_parses_an_explicit_alpha_channel() {
        assert_eq!(
            Color::from_hex("F0B44233").unwrap(),
            Color::rgba(0xF0, 0xB4, 0x42, 0x33)
        );
    }

    #[test]
    fn from_hex_rejects_malformed_input_instead_of_guessing() {
        assert_eq!(Color::from_hex("not-a-color"), None);
        assert_eq!(Color::from_hex("#ABC"), None);
        assert_eq!(Color::from_hex("#GGGGGG"), None);
    }

    #[test]
    fn selection_is_the_accent_color_at_reduced_alpha() {
        let selection = Theme::LUX_DARK.selection;
        let accent = Theme::LUX_DARK.accent;
        assert_eq!(
            (selection.r, selection.g, selection.b),
            (accent.r, accent.g, accent.b)
        );
        assert!(
            selection.a < accent.a,
            "selection must be translucent, not a solid fill"
        );
    }

    #[test]
    fn captures_each_named_color_at_its_expected_swatch_position() {
        let buffer = capture_swatch(&Theme::LUX_DARK);
        let width = (SWATCH_SIZE * SWATCH_COUNT) as usize;
        assert_eq!(buffer.len(), width * SWATCH_SIZE as usize * 4);

        let pixel_at = |x: u32, y: u32| -> [u8; 4] {
            let offset = ((y * SWATCH_SIZE * SWATCH_COUNT + x) * 4) as usize;
            buffer[offset..offset + 4].try_into().unwrap()
        };

        assert_eq!(pixel_at(0, 0), [0x12, 0x12, 0x1B, 0xFF], "background");
        assert_eq!(
            pixel_at(SWATCH_SIZE, 0),
            [0xE6, 0xE6, 0xEB, 0xFF],
            "foreground"
        );
        assert_eq!(
            pixel_at(SWATCH_SIZE * 2, 0),
            [0xF0, 0xB4, 0x42, 0xFF],
            "accent"
        );
        assert_eq!(
            pixel_at(SWATCH_SIZE * 3, 0),
            [0xF0, 0xB4, 0x42, 0x33],
            "selection"
        );
        assert_eq!(
            pixel_at(SWATCH_SIZE * 4, 0),
            [0x6E, 0x6E, 0x82, 0xFF],
            "muted"
        );
    }

    #[test]
    fn each_swatch_square_is_a_uniform_solid_fill_with_no_bleed_across_the_boundary() {
        let buffer = capture_swatch(&Theme::LUX_DARK);
        let width = SWATCH_SIZE * SWATCH_COUNT;
        let pixel_at = |x: u32, y: u32| -> [u8; 4] {
            let offset = ((y * width + x) * 4) as usize;
            buffer[offset..offset + 4].try_into().unwrap()
        };

        for swatch in 0..SWATCH_COUNT {
            let first = pixel_at(swatch * SWATCH_SIZE, 0);
            for y in 0..SWATCH_SIZE {
                for x in 0..SWATCH_SIZE {
                    assert_eq!(
                        pixel_at(swatch * SWATCH_SIZE + x, y),
                        first,
                        "swatch {swatch} must be a uniform fill at ({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn high_contrast_theme_meets_wcag_aa() {
        let t = Theme::LUX_DARK_HIGH_CONTRAST;
        assert!(contrast_ratio(t.foreground, t.background) >= 4.5);
        assert!(contrast_ratio(t.muted, t.background) >= 4.5);
    }
}
