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

/// Chrome surfaces (sidebar, tabs, status bar, selectors). One nested
/// struct so [`Theme`] stays a small palette instead of growing a field
/// per rectangle.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChromeColors {
    pub sidebar: Color,
    pub tab_bar: Color,
    pub tab_inactive: Color,
    pub status_bar: Color,
    pub panel: Color,
    pub panel_input: Color,
    pub panel_row: Color,
    pub border: Color,
    pub gutter: Color,
    pub line_highlight: Color,
}

const fn shift_channel(v: u8, delta: i16) -> u8 {
    let n = v as i16 + delta;
    if n < 0 {
        0
    } else if n > 255 {
        255
    } else {
        n as u8
    }
}

/// Moves every channel of `c` by `delta`, clamped to `0..=255`.
pub const fn shift(c: Color, delta: i16) -> Color {
    Color {
        r: shift_channel(c.r, delta),
        g: shift_channel(c.g, delta),
        b: shift_channel(c.b, delta),
        a: c.a,
    }
}

/// Chrome derived from an editor background, for themes that do not name
/// each surface. Dark backgrounds get a darker well and a lighter row.
pub const fn derived_chrome(bg: Color) -> ChromeColors {
    ChromeColors {
        sidebar: shift(bg, -14),
        tab_bar: shift(bg, -16),
        tab_inactive: shift(bg, -8),
        status_bar: shift(bg, -16),
        panel: shift(bg, -12),
        panel_input: shift(bg, 18),
        panel_row: shift(bg, 32),
        border: shift(bg, 24),
        gutter: shift(bg, -6),
        line_highlight: shift(bg, 12),
    }
}

/// Classic Monokai wells. Hex values are the ones the MIT VS Code
/// `theme-monokai` file documents as the original Monokai surfaces
/// (`#1E1F1C` tab well, `#272822` editor, `#414339` status/input,
/// `#3E3D32` border, `#49483E` selection). The current line is `#2C2D27`,
/// a step above the editor, so a comment on that line stays readable.
pub const MONOKAI_CHROME: ChromeColors = ChromeColors {
    sidebar: Color::rgb(0x1E, 0x1F, 0x1C),
    tab_bar: Color::rgb(0x1E, 0x1F, 0x1C),
    tab_inactive: Color::rgb(0x34, 0x35, 0x2F),
    status_bar: Color::rgb(0x41, 0x43, 0x39),
    panel: Color::rgb(0x1E, 0x1F, 0x1C),
    panel_input: Color::rgb(0x41, 0x43, 0x39),
    panel_row: Color::rgb(0x49, 0x48, 0x3E),
    border: Color::rgb(0x3E, 0x3D, 0x32),
    gutter: Color::rgb(0x27, 0x28, 0x22),
    line_highlight: Color::rgb(0x2C, 0x2D, 0x27),
};

/// Editor palette plus the chrome surfaces painted around it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Human-readable display name of the theme.
    pub name: &'static str,
    /// Editor fill.
    pub background: Color,
    /// Body text color, legible against `background`.
    pub foreground: Color,
    /// Caret, tab underline, and the selected-row stripe.
    pub accent: Color,
    /// Selection highlight.
    pub selection: Color,
    /// De-emphasized text (line numbers, inactive labels).
    pub muted: Color,
    /// Sidebar, tabs, status bar, and selector surfaces.
    pub chrome: ChromeColors,
}

impl Theme {
    /// The v0 identity, carried forward per `LUX_PROMPT_MAESTRO.md`'s
    /// Phase 2 deliverable 7.
    pub const LUX_DARK: Theme = Theme {
        name: "Lux Dark",
        background: Color::rgb(0x12, 0x12, 0x1B),
        foreground: Color::rgb(0xE6, 0xE6, 0xEB),
        accent: Color::rgb(0xF0, 0xB4, 0x42),
        selection: Color::rgba(0xF0, 0xB4, 0x42, 0x33),
        muted: Color::rgb(0x6E, 0x6E, 0x82),
        chrome: derived_chrome(Color::rgb(0x12, 0x12, 0x1B)),
    };

    /// High-contrast dark: near-black / near-white, WCAG AA for body text.
    pub const LUX_DARK_HIGH_CONTRAST: Theme = Theme {
        name: "Lux High Contrast",
        background: Color::rgb(0x00, 0x00, 0x00),
        foreground: Color::rgb(0xFF, 0xFF, 0xFF),
        accent: Color::rgb(0xFF, 0xD0, 0x00),
        selection: Color::rgba(0xFF, 0xD0, 0x00, 0x66),
        muted: Color::rgb(0xC0, 0xC0, 0xC0),
        chrome: derived_chrome(Color::rgb(0x00, 0x00, 0x00)),
    };

    /// Lux Light theme variant.
    pub const LUX_LIGHT: Theme = Theme {
        name: "Lux Light",
        background: Color::rgb(0xFA, 0xFA, 0xFD),
        foreground: Color::rgb(0x1E, 0x1E, 0x24),
        accent: Color::rgb(0xD9, 0x77, 0x06),
        selection: Color::rgba(0xD9, 0x77, 0x06, 0x33),
        muted: Color::rgb(0x71, 0x71, 0x7A),
        chrome: derived_chrome(Color::rgb(0xFA, 0xFA, 0xFD)),
    };

    /// Sublime Text 3 classic dark theme (ST15).
    pub const MONOKAI: Theme = Theme {
        name: "Monokai",
        background: Color::rgb(0x27, 0x28, 0x22),
        foreground: Color::rgb(0xF8, 0xF8, 0xF2),
        accent: Color::rgb(0xF9, 0x26, 0x72),
        selection: Color::rgba(0x49, 0x48, 0x3E, 0x88),
        muted: Color::rgb(0x75, 0x71, 0x5E),
        chrome: MONOKAI_CHROME,
    };

    /// Sublime Text 3.1+ default slate blue theme (ST15).
    pub const MARIANA: Theme = Theme {
        name: "Mariana",
        background: Color::rgb(0x30, 0x38, 0x41),
        foreground: Color::rgb(0xD8, 0xDE, 0xE9),
        accent: Color::rgb(0x5F, 0xB3, 0xB3),
        selection: Color::rgba(0x4F, 0x5B, 0x66, 0x88),
        muted: Color::rgb(0xA7, 0xAD, 0xBA),
        chrome: derived_chrome(Color::rgb(0x30, 0x38, 0x41)),
    };

    /// Sublime Text high-contrast ocean theme (ST15).
    pub const BREAKERS: Theme = Theme {
        name: "Breakers",
        background: Color::rgb(0x0C, 0x10, 0x14),
        foreground: Color::rgb(0xFF, 0xFF, 0xFF),
        accent: Color::rgb(0x00, 0xD0, 0xD0),
        selection: Color::rgba(0x1B, 0x25, 0x2E, 0xAA),
        muted: Color::rgb(0x61, 0x73, 0x82),
        chrome: derived_chrome(Color::rgb(0x0C, 0x10, 0x14)),
    };

    /// Sublime Text light theme (ST15).
    pub const CELESTE: Theme = Theme {
        name: "Celeste",
        background: Color::rgb(0xF8, 0xF9, 0xFA),
        foreground: Color::rgb(0x21, 0x25, 0x29),
        accent: Color::rgb(0x00, 0x7A, 0xCC),
        selection: Color::rgba(0xD0, 0xE1, 0xF9, 0x88),
        muted: Color::rgb(0x6C, 0x75, 0x7D),
        chrome: derived_chrome(Color::rgb(0xF8, 0xF9, 0xFA)),
    };

    pub const fn lux_dark() -> Self {
        Self::LUX_DARK
    }

    pub const fn lux_light() -> Self {
        Self::LUX_LIGHT
    }

    pub const fn high_contrast() -> Self {
        Self::LUX_DARK_HIGH_CONTRAST
    }

    pub const fn monokai() -> Self {
        Self::MONOKAI
    }

    pub const fn mariana() -> Self {
        Self::MARIANA
    }

    pub const fn breakers() -> Self {
        Self::BREAKERS
    }

    pub const fn celeste() -> Self {
        Self::CELESTE
    }

    /// Looks up a built-in theme by name (case-insensitive, ignoring hyphens and spaces).
    pub fn by_name(name: &str) -> Option<Self> {
        let n = name.trim().to_ascii_lowercase();
        let normalized = n.replace(['-', '_', ' '], "");
        match normalized.as_str() {
            "monokai" => Some(Self::MONOKAI),
            "mariana" => Some(Self::MARIANA),
            "breakers" => Some(Self::BREAKERS),
            "celeste" => Some(Self::CELESTE),
            "luxdark" | "dark" => Some(Self::LUX_DARK),
            "luxlight" | "light" => Some(Self::LUX_LIGHT),
            "luxhighcontrast" | "highcontrast" => Some(Self::LUX_DARK_HIGH_CONTRAST),
            _ => None,
        }
    }
}

/// All themes available in the editor.
pub const ALL_THEMES: &[Theme] = &[
    Theme::MONOKAI,
    Theme::MARIANA,
    Theme::BREAKERS,
    Theme::CELESTE,
    Theme::LUX_DARK,
    Theme::LUX_LIGHT,
    Theme::LUX_DARK_HIGH_CONTRAST,
];

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

    #[test]
    fn sublime_themes_meet_wcag_aa() {
        for t in [
            &Theme::MONOKAI,
            &Theme::MARIANA,
            &Theme::BREAKERS,
            &Theme::CELESTE,
        ] {
            let ratio = contrast_ratio(t.foreground, t.background);
            assert!(
                ratio >= 4.5,
                "{}: foreground/background contrast {ratio} must meet WCAG AA (>= 4.5)",
                t.name
            );
        }
    }

    #[test]
    fn by_name_resolves_all_themes_with_various_casings() {
        assert_eq!(Theme::by_name("monokai"), Some(Theme::MONOKAI));
        assert_eq!(Theme::by_name("Monokai"), Some(Theme::MONOKAI));
        assert_eq!(Theme::by_name("mariana"), Some(Theme::MARIANA));
        assert_eq!(Theme::by_name("Mariana"), Some(Theme::MARIANA));
        assert_eq!(Theme::by_name("breakers"), Some(Theme::BREAKERS));
        assert_eq!(Theme::by_name("Breakers"), Some(Theme::BREAKERS));
        assert_eq!(Theme::by_name("celeste"), Some(Theme::CELESTE));
        assert_eq!(Theme::by_name("Celeste"), Some(Theme::CELESTE));
        assert_eq!(Theme::by_name("lux-dark"), Some(Theme::LUX_DARK));
        assert_eq!(Theme::by_name("Lux Dark"), Some(Theme::LUX_DARK));
        assert_eq!(Theme::by_name("lux-light"), Some(Theme::LUX_LIGHT));
        assert_eq!(Theme::by_name("Lux Light"), Some(Theme::LUX_LIGHT));
        assert_eq!(
            Theme::by_name("high-contrast"),
            Some(Theme::LUX_DARK_HIGH_CONTRAST)
        );
        assert_eq!(Theme::by_name("non-existent"), None);
    }

    #[test]
    fn monokai_chrome_uses_the_classic_wells() {
        let t = Theme::MONOKAI;
        assert_eq!(t.background, Color::from_hex("#272822").unwrap());
        assert_eq!(t.foreground, Color::from_hex("#F8F8F2").unwrap());
        assert_eq!(t.chrome.sidebar, Color::from_hex("#1E1F1C").unwrap());
        assert_eq!(t.chrome.tab_bar, Color::from_hex("#1E1F1C").unwrap());
        assert_eq!(t.chrome.status_bar, Color::from_hex("#414339").unwrap());
        assert_eq!(t.chrome.panel, Color::from_hex("#1E1F1C").unwrap());
        assert_eq!(t.chrome.panel_input, Color::from_hex("#414339").unwrap());
        assert_eq!(t.chrome.panel_row, Color::from_hex("#49483E").unwrap());
        assert_eq!(t.chrome.line_highlight, Color::from_hex("#2C2D27").unwrap());
        assert_eq!(t.chrome.gutter, t.background);
    }
}
