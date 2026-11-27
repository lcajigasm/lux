//! Inks for the shared picker card.
//!
//! The selected row is the theme's own selection color, nudged off the
//! card only as far as it takes to be seen. A 3:1 floor here painted a
//! flat `#8A8A80` slab with dark text in Monokai. Matches use the theme
//! accent wherever it clears 3:1 against the row; where it cannot, they
//! fall back to the row's text color in bold.
//!
//! A translucent theme selection (Lux Dark's is amber at 20%) is flattened
//! onto the card first. Every ink below is chosen against the color the
//! GPU actually paints, not against the selection's opaque RGB: judged
//! against solid amber, Lux Dark's row text went near-black on what is
//! really a dark brown row.

use crate::render::paint::color;
use crate::render::panel::{
    card_fill, contrast_ratio, label_ink, readable_ink, shortcut_ink, visible_fill,
};

/// The accent has to clear this against a row to mark a match.
pub(crate) const MATCH_FLOOR: f32 = 3.0;
/// The selection fill is moved off the card until it clears this.
pub(crate) const SELECTION_FLOOR: f32 = 1.4;
/// Matches on the selected row are bold text: the accent is kept there
/// only when it reads as body text (WCAG AA) on the painted row.
pub(crate) const SELECTED_MATCH_FLOOR: f32 = 4.5;

pub(crate) struct OverlayColors {
    pub card: [f32; 4],
    pub selection: [f32; 4],
    pub thumb: [f32; 4],
    pub text_card: [f32; 4],
    pub text_selection: [f32; 4],
    pub match_card: [f32; 4],
    pub match_selection: [f32; 4],
    /// The accent failed [`MATCH_FLOOR`] on the card, so matches there
    /// are the text color in bold. Matches on the selection are always bold.
    pub match_card_bold: bool,
    pub detail_card: [f32; 4],
    pub detail_selection: [f32; 4],
    /// 1 px rim around a color swatch: 20% foreground over the card.
    pub chip_border: [f32; 4],
}

pub(crate) fn overlay_colors(theme: &lux_ui::Theme) -> OverlayColors {
    let card = card_fill(theme);
    let painted = lux_ui::composite(theme.selection, theme.chrome.panel_input);
    let selection = visible_fill(color(painted), card, SELECTION_FLOOR);
    let accent = color(theme.accent);
    let text_card = label_ink(theme, card);
    let text_selection = label_ink(theme, selection);
    let accent_on = |row: [f32; 4]| contrast_ratio(accent, row) >= MATCH_FLOOR;
    let (match_card, match_card_bold) = if accent_on(card) {
        (accent, false)
    } else {
        (text_card, true)
    };
    let match_selection = if contrast_ratio(accent, selection) >= SELECTED_MATCH_FLOOR {
        accent
    } else {
        text_selection
    };
    let foreground = color(theme.foreground);
    let mix = |t: f32| {
        [
            card[0] + (foreground[0] - card[0]) * t,
            card[1] + (foreground[1] - card[1]) * t,
            card[2] + (foreground[2] - card[2]) * t,
            1.0,
        ]
    };
    OverlayColors {
        card,
        selection,
        thumb: visible_fill(color(theme.chrome.border), card, 3.05),
        text_card,
        text_selection,
        match_card,
        match_selection,
        match_card_bold,
        detail_card: readable_ink(shortcut_ink(theme), card),
        detail_selection: readable_ink(shortcut_ink(theme), selection),
        chip_border: mix(0.2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every theme the picker and settings can show: the seven built-ins
    /// and each bundled scheme, resolved the way the app resolves them.
    fn every_shown_theme() -> Vec<lux_ui::Theme> {
        let schemes = crate::color_scheme::bundled_schemes();
        assert!(schemes.len() >= 14, "{} schemes", schemes.len());
        let names = crate::color_scheme::theme_names(schemes);
        assert!(names.contains(&"Lux Dark"), "{names:?}");
        names
            .iter()
            .map(|name| crate::color_scheme::theme_for(schemes, name))
            .collect()
    }

    /// The row as the GPU blends it: the pushed fill over the card. Row
    /// text and matches are judged against this, never the fill's RGB.
    fn painted(fill: [f32; 4], card: [f32; 4]) -> [f32; 4] {
        let a = fill[3];
        [
            fill[0] * a + card[0] * (1.0 - a),
            fill[1] * a + card[1] * (1.0 - a),
            fill[2] * a + card[2] * (1.0 - a),
            1.0,
        ]
    }

    #[test]
    fn every_shown_theme_keeps_matches_and_selection_readable() {
        for theme in every_shown_theme() {
            let c = overlay_colors(&theme);
            let name = theme.name;
            assert_eq!(c.card[3], 1.0, "{name} card is opaque");
            let row = painted(c.selection, c.card);
            let at_least = |ratio: f32, floor: f32, what: &str| {
                println!("{name} {what} {ratio:.2}");
                assert!(ratio >= floor - 0.01, "{name} {what} {ratio:.2} < {floor}");
            };
            at_least(contrast_ratio(c.text_card, c.card), 4.5, "text/card");
            at_least(contrast_ratio(c.text_selection, row), 4.5, "text/selection");
            at_least(contrast_ratio(c.detail_card, c.card), 4.5, "hint/card");
            at_least(
                contrast_ratio(c.detail_selection, row),
                4.5,
                "hint/selection",
            );
            at_least(
                contrast_ratio(row, c.card),
                SELECTION_FLOOR,
                "selection/card",
            );
            at_least(contrast_ratio(c.thumb, c.card), 3.0, "thumb/card");
            at_least(
                contrast_ratio(c.match_card, c.card),
                MATCH_FLOOR,
                "match/card",
            );
            // Matches on the selected row are bold and must read as text.
            at_least(
                contrast_ratio(c.match_selection, row),
                SELECTED_MATCH_FLOOR,
                "match/selection",
            );
            // The accent is used exactly when it clears the floor.
            let accent = color(theme.accent);
            assert_eq!(
                c.match_card_bold,
                contrast_ratio(accent, c.card) < MATCH_FLOOR,
                "{name}"
            );
            if c.match_card_bold {
                assert_eq!(c.match_card, c.text_card, "{name} falls back to text");
            } else {
                assert_eq!(c.match_card, accent, "{name} uses the accent");
            }
            assert!(c.chip_border != c.card, "{name} chip rim is visible");
        }
    }

    #[test]
    fn lux_dark_selected_row_has_light_text_on_its_painted_fill() {
        let c = overlay_colors(&lux_ui::Theme::LUX_DARK);
        // The amber selection is 20% alpha; flattened onto the card it is
        // a dark row, so its text stays the light theme foreground.
        assert_eq!(c.selection[3], 1.0);
        let luminance = |v: [f32; 4]| v[0] + v[1] + v[2];
        assert!(luminance(c.selection) < 1.0, "{:?}", c.selection);
        assert_eq!(c.text_selection, color(lux_ui::Theme::LUX_DARK.foreground));
    }

    #[test]
    fn monokai_selection_is_the_theme_selection_with_light_text() {
        let schemes = crate::color_scheme::bundled_schemes();
        let monokai = schemes
            .iter()
            .find(|scheme| scheme.name == "Monokai")
            .expect("Monokai is bundled");
        let c = overlay_colors(&monokai.theme);
        // No grey slab: the fill stays near the card and the text is the
        // theme foreground, not dark ink.
        assert!(contrast_ratio(c.selection, c.card) < 2.0);
        assert_eq!(c.text_selection, color(monokai.theme.foreground));
        assert_eq!(c.text_card, color(monokai.theme.foreground));
    }
}
