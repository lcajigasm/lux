//! Token/scope-based syntax themes (Phase 3 deliverable 2) — the fix for
//! v0's concrete failure of styling syntax off a flat, global UI palette
//! (a handful of chrome colors with no per-token granularity). This is a
//! self-contained theme format, the same shape as TextMate/VS Code themes:
//! an editor background/foreground pair plus a table from *scope* (the
//! dotted, TextMate-style names our own `queries/*/highlights.scm` files
//! already emit as capture names — see [`crate::Token::kind`]) to a color.
//!
//! Scope resolution falls back through the dotted hierarchy: a theme that
//! defines `"keyword"` but not `"keyword.conditional.ternary"` still colors
//! the latter, by trimming trailing `.segments` until a match is found or
//! the theme's flat `default` color is used. This is the actual mechanism
//! that makes it scope-*based* rather than a flat lookup table — the same
//! best-match-by-specificity algorithm TextMate/VS Code themes use.
//!
//! This crate has no UI dependency (golden rule #2): `lux-ui`'s own
//! `Theme` (Phase 2) covers UI chrome (selection highlight, caret) and is
//! deliberately not reused here or vice versa. Wiring a [`SyntaxTheme`]'s
//! colors into `lux-ui`'s rendering is separate, later work.

/// A plain sRGB color, no alpha — syntax token colors are always opaque
/// fills; translucency (e.g. a selection highlight) is a UI-chrome concern,
/// not a token-color one.
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

    /// Parses a `#RRGGBB` hex string. Returns `None` for anything else
    /// rather than guessing.
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

    /// WCAG relative luminance (sRGB channels linearized per the spec),
    /// used by [`contrast_ratio`].
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
/// AA body text wants >= 4.5, AAA wants >= 7.0 — a high-contrast theme
/// should clear the AAA bar, not just eyeball "looks dark and light".
#[must_use]
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (l1, l2) = (a.relative_luminance(), b.relative_luminance());
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// A complete token/scope theme: an editor background/foreground pair plus
/// a scope-name-to-color table, resolved with TextMate-style prefix
/// fallback (see the module docs).
pub struct SyntaxTheme {
    pub name: &'static str,
    pub background: Color,
    pub default: Color,
    scopes: &'static [(&'static str, Color)],
    /// Cycling palette for bracket pair colorization (deliverable 3): the
    /// color for nesting depth `d` is `bracket_colors[d % len]`, the same
    /// modulo-cycle scheme editors converged on (VS Code ships 6 colors
    /// for the same reason) — an unbounded distinct-color-per-depth
    /// palette isn't meaningfully more readable past a handful of steps.
    bracket_colors: &'static [Color],
}

impl SyntaxTheme {
    /// Resolves a highlight scope (a [`crate::Token::kind`] value, e.g.
    /// `"keyword.conditional.ternary"`) to a color: exact match first, then
    /// each shorter dotted prefix, then [`Self::default`]. The special
    /// scope `"none"` (the convention grammars use for "force plain text")
    /// always resolves to `default` without walking the table.
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

    /// The bracket-pair-colorization color for nesting depth `depth`
    /// (0 = outermost pair), cycling through [`Self::bracket_colors`].
    #[must_use]
    pub fn color_for_bracket_depth(&self, depth: usize) -> Color {
        self.bracket_colors[depth % self.bracket_colors.len()]
    }
}

/// Base scope categories every theme in this module defines explicitly,
/// covering every distinct capture name used across `queries/*/highlights.scm`
/// (either directly, for bare names like `"tag"`/`"conditional"`, or as the
/// dotted prefix that names like `"keyword.conditional.ternary"` or
/// `"function.method"` fall back to).
macro_rules! scope_table {
    ($($scope:literal => $color:expr),+ $(,)?) => {
        &[$(($scope, $color)),+]
    };
}

/// The v0 identity's dark theme, extended with real per-scope token colors.
/// `background`/`default` match `lux-ui::Theme::LUX_DARK` verbatim (same
/// `#12121B`/`#E6E6EB`) for brand consistency; numeric/boolean literals use
/// the brand's golden-amber accent (`#F0B442`, `LUX_PROMPT_MAESTRO.md`
/// §0.6's "the warm accent is the brand's mark"), since literal values are
/// the traditional home for a theme's signature accent color.
pub const LUX_DARK: SyntaxTheme = SyntaxTheme {
    name: "Lux Dark",
    background: Color::rgb(0x12, 0x12, 0x1B),
    default: Color::rgb(0xE6, 0xE6, 0xEB),
    scopes: scope_table! {
        "comment" => Color::rgb(0x6E, 0x6E, 0x82),
        "keyword" => Color::rgb(0x9D, 0x7C, 0xD8),
        "conditional" => Color::rgb(0x9D, 0x7C, 0xD8),
        "repeat" => Color::rgb(0x9D, 0x7C, 0xD8),
        "import" => Color::rgb(0x9D, 0x7C, 0xD8),
        "include" => Color::rgb(0x9D, 0x7C, 0xD8),
        "string" => Color::rgb(0x9E, 0xCE, 0x6A),
        "character" => Color::rgb(0x9E, 0xCE, 0x6A),
        "charset" => Color::rgb(0x9E, 0xCE, 0x6A),
        "escape" => Color::rgb(0x89, 0xDD, 0xFF),
        "number" => Color::rgb(0xF0, 0xB4, 0x42),
        "float" => Color::rgb(0xF0, 0xB4, 0x42),
        "boolean" => Color::rgb(0xF0, 0xB4, 0x42),
        "constant" => Color::rgb(0xF0, 0xB4, 0x42),
        "type" => Color::rgb(0x4F, 0xD6, 0xBE),
        "function" => Color::rgb(0x7A, 0xA2, 0xF7),
        "constructor" => Color::rgb(0x7A, 0xA2, 0xF7),
        "variable" => Color::rgb(0xE6, 0xE6, 0xEB),
        "parameter" => Color::rgb(0xC0, 0xCA, 0xF5),
        "property" => Color::rgb(0x7D, 0xCF, 0xFF),
        "attribute" => Color::rgb(0xFF, 0x9E, 0x64),
        "namespace" => Color::rgb(0xBB, 0x9A, 0xF7),
        "media" => Color::rgb(0xBB, 0x9A, 0xF7),
        "supports" => Color::rgb(0xBB, 0x9A, 0xF7),
        "keyframes" => Color::rgb(0xBB, 0x9A, 0xF7),
        "tag" => Color::rgb(0xF7, 0x76, 0x8E),
        "label" => Color::rgb(0xF7, 0x76, 0x8E),
        "exception" => Color::rgb(0xF7, 0x76, 0x8E),
        "operator" => Color::rgb(0x89, 0xDD, 0xFF),
        "punctuation" => Color::rgb(0x95, 0x99, 0xA8),
        "delimiter" => Color::rgb(0x95, 0x99, 0xA8),
        "embedded" => Color::rgb(0xE6, 0xE6, 0xEB),
        "spell" => Color::rgb(0xE6, 0xE6, 0xEB),
        "text" => Color::rgb(0xE6, 0xE6, 0xEB),
    },
    bracket_colors: &[
        Color::rgb(0xF0, 0xB4, 0x42),
        Color::rgb(0x9D, 0x7C, 0xD8),
        Color::rgb(0x4F, 0xD6, 0xBE),
        Color::rgb(0xF7, 0x76, 0x8E),
        Color::rgb(0x7A, 0xA2, 0xF7),
        Color::rgb(0x9E, 0xCE, 0x6A),
    ],
};

/// A light complement to Lux Dark: same hue families (each scope keeps its
/// role — keywords stay violet, strings stay green, etc.) darkened and
/// re-saturated for legibility on a light background, rather than a naive
/// per-channel inversion (which reliably produces muddy, low-contrast
/// colors — inverting a light pastel doesn't produce a readable dark one).
pub const LUX_LIGHT: SyntaxTheme = SyntaxTheme {
    name: "Lux Light",
    background: Color::rgb(0xFA, 0xFA, 0xF7),
    default: Color::rgb(0x1B, 0x1B, 0x23),
    scopes: scope_table! {
        "comment" => Color::rgb(0x8A, 0x8A, 0x9E),
        "keyword" => Color::rgb(0x7C, 0x3F, 0xC4),
        "conditional" => Color::rgb(0x7C, 0x3F, 0xC4),
        "repeat" => Color::rgb(0x7C, 0x3F, 0xC4),
        "import" => Color::rgb(0x7C, 0x3F, 0xC4),
        "include" => Color::rgb(0x7C, 0x3F, 0xC4),
        "string" => Color::rgb(0x2E, 0x7D, 0x32),
        "character" => Color::rgb(0x2E, 0x7D, 0x32),
        "charset" => Color::rgb(0x2E, 0x7D, 0x32),
        "escape" => Color::rgb(0x0E, 0x74, 0x90),
        "number" => Color::rgb(0xB4, 0x65, 0x0A),
        "float" => Color::rgb(0xB4, 0x65, 0x0A),
        "boolean" => Color::rgb(0xB4, 0x65, 0x0A),
        "constant" => Color::rgb(0xB4, 0x65, 0x0A),
        "type" => Color::rgb(0x0F, 0x76, 0x6E),
        "function" => Color::rgb(0x1E, 0x56, 0xC8),
        "constructor" => Color::rgb(0x1E, 0x56, 0xC8),
        "variable" => Color::rgb(0x1B, 0x1B, 0x23),
        "parameter" => Color::rgb(0x3B, 0x3B, 0x55),
        "property" => Color::rgb(0x0E, 0x74, 0x90),
        "attribute" => Color::rgb(0xC2, 0x54, 0x0A),
        "namespace" => Color::rgb(0x7C, 0x3F, 0xC4),
        "media" => Color::rgb(0x7C, 0x3F, 0xC4),
        "supports" => Color::rgb(0x7C, 0x3F, 0xC4),
        "keyframes" => Color::rgb(0x7C, 0x3F, 0xC4),
        "tag" => Color::rgb(0xB3, 0x26, 0x4D),
        "label" => Color::rgb(0xB3, 0x26, 0x4D),
        "exception" => Color::rgb(0xB3, 0x26, 0x4D),
        "operator" => Color::rgb(0x0E, 0x74, 0x90),
        "punctuation" => Color::rgb(0x6B, 0x6B, 0x80),
        "delimiter" => Color::rgb(0x6B, 0x6B, 0x80),
        "embedded" => Color::rgb(0x1B, 0x1B, 0x23),
        "spell" => Color::rgb(0x1B, 0x1B, 0x23),
        "text" => Color::rgb(0x1B, 0x1B, 0x23),
    },
    bracket_colors: &[
        Color::rgb(0xB4, 0x65, 0x0A),
        Color::rgb(0x7C, 0x3F, 0xC4),
        Color::rgb(0x0F, 0x76, 0x6E),
        Color::rgb(0xB3, 0x26, 0x4D),
        Color::rgb(0x1E, 0x56, 0xC8),
        Color::rgb(0x2E, 0x7D, 0x32),
    ],
};

/// Maximum-legibility theme: pure black/white base (background/foreground
/// contrast ratio > 20:1, comfortably clearing WCAG AAA's 7:1 bar for
/// normal text — verified in this module's tests, not just chosen by eye),
/// every scope a fully-saturated, mutually distinct hue.
pub const LUX_HIGH_CONTRAST: SyntaxTheme = SyntaxTheme {
    name: "Lux High Contrast",
    background: Color::rgb(0x00, 0x00, 0x00),
    default: Color::rgb(0xFF, 0xFF, 0xFF),
    scopes: scope_table! {
        "comment" => Color::rgb(0xA6, 0xA6, 0xA6),
        "keyword" => Color::rgb(0xFF, 0x6E, 0xC7),
        "conditional" => Color::rgb(0xFF, 0x6E, 0xC7),
        "repeat" => Color::rgb(0xFF, 0x6E, 0xC7),
        "import" => Color::rgb(0xFF, 0x6E, 0xC7),
        "include" => Color::rgb(0xFF, 0x6E, 0xC7),
        "string" => Color::rgb(0x4F, 0xFF, 0xB0),
        "character" => Color::rgb(0x4F, 0xFF, 0xB0),
        "charset" => Color::rgb(0x4F, 0xFF, 0xB0),
        "escape" => Color::rgb(0x4F, 0xFF, 0xB0),
        "number" => Color::rgb(0xFF, 0xD4, 0x00),
        "float" => Color::rgb(0xFF, 0xD4, 0x00),
        "boolean" => Color::rgb(0xFF, 0xD4, 0x00),
        "constant" => Color::rgb(0xFF, 0xD4, 0x00),
        "type" => Color::rgb(0x00, 0xE5, 0xFF),
        "function" => Color::rgb(0x5E, 0xA1, 0xFF),
        "constructor" => Color::rgb(0x5E, 0xA1, 0xFF),
        "variable" => Color::rgb(0xFF, 0xFF, 0xFF),
        "parameter" => Color::rgb(0xDD, 0xEE, 0xFF),
        "property" => Color::rgb(0x7C, 0xF5, 0xFF),
        "attribute" => Color::rgb(0xFF, 0xA6, 0x30),
        "namespace" => Color::rgb(0xC7, 0x92, 0xFF),
        "media" => Color::rgb(0xC7, 0x92, 0xFF),
        "supports" => Color::rgb(0xC7, 0x92, 0xFF),
        "keyframes" => Color::rgb(0xC7, 0x92, 0xFF),
        "tag" => Color::rgb(0xFF, 0x5C, 0x7A),
        "label" => Color::rgb(0xFF, 0x5C, 0x7A),
        "exception" => Color::rgb(0xFF, 0x5C, 0x7A),
        "operator" => Color::rgb(0xFF, 0xFF, 0xFF),
        "punctuation" => Color::rgb(0xCF, 0xCF, 0xCF),
        "delimiter" => Color::rgb(0xCF, 0xCF, 0xCF),
        "embedded" => Color::rgb(0xFF, 0xFF, 0xFF),
        "spell" => Color::rgb(0xFF, 0xFF, 0xFF),
        "text" => Color::rgb(0xFF, 0xFF, 0xFF),
    },
    bracket_colors: &[
        Color::rgb(0xFF, 0xD4, 0x00),
        Color::rgb(0xFF, 0x6E, 0xC7),
        Color::rgb(0x00, 0xE5, 0xFF),
        Color::rgb(0xFF, 0x5C, 0x7A),
        Color::rgb(0x5E, 0xA1, 0xFF),
        Color::rgb(0x4F, 0xFF, 0xB0),
    ],
};

/// Every theme this module ships, for tests (and eventually a theme
/// picker) that want to iterate all of them instead of naming each one.
pub const ALL: &[&SyntaxTheme] = &[&LUX_DARK, &LUX_LIGHT, &LUX_HIGH_CONTRAST];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_an_exact_scope_match() {
        assert_eq!(
            LUX_DARK.color_for_scope("comment"),
            Color::rgb(0x6E, 0x6E, 0x82)
        );
    }

    #[test]
    fn falls_back_through_the_dotted_hierarchy() {
        // "keyword.conditional.ternary" isn't a table entry; it must trim
        // to "keyword.conditional" (also not an entry _by that exact
        // three-vs-two-segment distinction_ here) and then to "keyword".
        assert_eq!(
            LUX_DARK.color_for_scope("keyword.conditional.ternary"),
            LUX_DARK.color_for_scope("keyword")
        );
        assert_eq!(
            LUX_DARK.color_for_scope("function.method.builtin"),
            LUX_DARK.color_for_scope("function")
        );
    }

    #[test]
    fn unknown_scope_falls_back_to_the_theme_default() {
        assert_eq!(
            LUX_DARK.color_for_scope("this.scope.does.not.exist"),
            LUX_DARK.default
        );
    }

    #[test]
    fn the_none_scope_always_means_plain_default_text() {
        assert_eq!(LUX_DARK.color_for_scope("none"), LUX_DARK.default);
        assert_eq!(
            LUX_HIGH_CONTRAST.color_for_scope("none"),
            LUX_HIGH_CONTRAST.default
        );
    }

    #[test]
    fn from_hex_parses_and_rejects_malformed_input() {
        assert_eq!(
            Color::from_hex("#F0B442"),
            Some(Color::rgb(0xF0, 0xB4, 0x42))
        );
        assert_eq!(
            Color::from_hex("F0B442"),
            Some(Color::rgb(0xF0, 0xB4, 0x42))
        );
        assert_eq!(Color::from_hex("not-a-color"), None);
        assert_eq!(Color::from_hex("#ABC"), None);
    }

    #[test]
    fn lux_dark_matches_lux_uis_v0_identity_verbatim() {
        // Same brand background/foreground as `lux-ui::Theme::LUX_DARK`
        // (Phase 2 deliverable 7) — duplicated by value, not by dependency
        // (this crate has no UI dependency, golden rule #2), but the hex
        // codes must still agree so the two crates read as one brand.
        assert_eq!(LUX_DARK.background, Color::from_hex("#12121B").unwrap());
        assert_eq!(LUX_DARK.default, Color::from_hex("#E6E6EB").unwrap());
    }

    #[test]
    fn high_contrast_theme_clears_the_wcag_aaa_bar() {
        let ratio = contrast_ratio(LUX_HIGH_CONTRAST.background, LUX_HIGH_CONTRAST.default);
        assert!(
            ratio >= 7.0,
            "background/foreground contrast {ratio} must be >= 7.0 (WCAG AAA)"
        );
    }

    #[test]
    fn dark_and_light_themes_clear_the_wcag_aa_bar() {
        for theme in [&LUX_DARK, &LUX_LIGHT] {
            let ratio = contrast_ratio(theme.background, theme.default);
            assert!(
                ratio >= 4.5,
                "{}: background/foreground contrast {ratio} must be >= 4.5 (WCAG AA)",
                theme.name
            );
        }
    }

    #[test]
    fn every_theme_covers_every_base_scope_used_by_a_real_grammar_query() {
        // Extracted from `queries/*/highlights.scm`: every bare (dot-less)
        // capture name any vendored grammar actually emits. Checks for an
        // explicit table entry (not "resolves to a color different from
        // `default`" — plain variables and prose-only scopes like `"text"`
        // are legitimately styled the same as `default`, that's a design
        // choice, not a missing entry).
        const BASE_SCOPES: &[&str] = &[
            "attribute",
            "boolean",
            "character",
            "charset",
            "comment",
            "conditional",
            "constant",
            "constructor",
            "delimiter",
            "embedded",
            "escape",
            "exception",
            "float",
            "function",
            "import",
            "include",
            "keyframes",
            "keyword",
            "label",
            "media",
            "namespace",
            "number",
            "operator",
            "parameter",
            "property",
            "punctuation",
            "repeat",
            "spell",
            "string",
            "supports",
            "tag",
            "text",
            "type",
            "variable",
        ];
        for theme in ALL {
            for scope in BASE_SCOPES {
                assert!(
                    theme.scopes.iter().any(|(s, _)| s == scope),
                    "{}: scope {scope:?} has no explicit table entry",
                    theme.name
                );
            }
        }
    }

    #[test]
    fn a_real_highlighted_document_uses_a_meaningfully_diverse_set_of_colors() {
        // Not a flat, global palette (v0's concrete failure): highlighting
        // a real fixture through a real theme must produce more than a
        // couple of distinct colors.
        let mut doc = crate::Document::new(crate::LanguageId::Rust);
        let source = include_str!("../tests/fixtures/rust/sample.rs");
        doc.set_text(source);
        let tokens = doc.highlight(source);
        assert!(
            !tokens.is_empty(),
            "fixture must actually produce highlight tokens"
        );

        for theme in ALL {
            let distinct = tokens
                .iter()
                .map(|t| theme.color_for_token(t))
                .collect::<std::collections::HashSet<_>>()
                .len();
            assert!(
                distinct >= 8,
                "{}: only {distinct} distinct colors across a real file, expected a real per-scope theme",
                theme.name
            );
        }
    }
}
