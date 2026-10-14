//! Token/scope-based syntax themes (Phase 3 deliverable 2).
//!
//! A self-contained theme format, the same shape as TextMate/VS Code themes:
//! an editor background/foreground pair plus a table from scope to a color.
//! Scope resolution falls back through the dotted hierarchy.

mod presets;
#[cfg(test)]
mod tests;

pub use presets::{
    ALL, BREAKERS, CELESTE, LUX_DARK, LUX_HIGH_CONTRAST, LUX_LIGHT, MARIANA, MONOKAI,
};

/// A plain sRGB color, no alpha — syntax token colors are always opaque fills.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses a `#RRGGBB` hex string. Returns `None` for anything else.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            return None;
        }
        let byte = |s: &str| u8::from_str_radix(s, 16).ok();
        Some(Self::rgb(
            byte(&hex[0..2])?,
            byte(&hex[2..4])?,
            byte(&hex[4..6])?,
        ))
    }

    /// WCAG relative luminance (sRGB channels linearized per the spec).
    #[must_use]
    pub fn relative_luminance(self) -> f64 {
        let channel = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

/// WCAG contrast ratio between two colors (1.0..=21.0, order-independent).
#[must_use]
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (l1, l2) = (a.relative_luminance(), b.relative_luminance());
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// A complete token/scope theme: an editor background/foreground pair plus
/// a scope-name-to-color table, resolved with TextMate-style prefix fallback.
pub struct SyntaxTheme {
    pub name: &'static str,
    pub background: Color,
    pub default: Color,
    pub scopes: &'static [(&'static str, Color)],
    /// Cycling palette for bracket pair colorization
    pub bracket_colors: &'static [Color],
}

impl SyntaxTheme {
    /// Resolves a highlight scope to a color: exact match first, then
    /// each shorter dotted prefix, then [`Self::default`].
    #[must_use]
    pub fn color_for_scope(&self, scope: &str) -> Color {
        if scope == "none" {
            return self.default;
        }
        let mut candidate = scope;
        loop {
            if let Some(&(_, color)) = self.scopes.iter().find(|(s, _)| *s == candidate) {
                return color;
            }
            match candidate.rfind('.') {
                Some(idx) => candidate = &candidate[..idx],
                None => return self.default,
            }
        }
    }

    /// Convenience over [`Self::color_for_scope`] for a [`crate::Token`].
    #[must_use]
    pub fn color_for_token(&self, token: &crate::Token) -> Color {
        self.color_for_scope(&token.kind)
    }

    /// The bracket-pair-colorization color for nesting depth `depth`.
    #[must_use]
    pub fn color_for_bracket_depth(&self, depth: usize) -> Color {
        self.bracket_colors[depth % self.bracket_colors.len()]
    }
}

/// Resolves a syntax theme by name (case-insensitive, ignoring hyphens and spaces).
#[must_use]
pub fn by_name(name: &str) -> Option<&'static SyntaxTheme> {
    let n = name.trim().to_ascii_lowercase();
    let normalized = n.replace(['-', '_', ' '], "");
    match normalized.as_str() {
        "monokai" => Some(&MONOKAI),
        "mariana" => Some(&MARIANA),
        "breakers" => Some(&BREAKERS),
        "celeste" => Some(&CELESTE),
        "luxdark" | "dark" => Some(&LUX_DARK),
        "luxlight" | "light" => Some(&LUX_LIGHT),
        "luxhighcontrast" | "highcontrast" => Some(&LUX_HIGH_CONTRAST),
        _ => None,
    }
}
