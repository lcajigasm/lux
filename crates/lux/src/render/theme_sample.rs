//! Color chips and the open list on the Settings theme menu.
//!
//! A bundled scheme uses its own background, foreground, accent, and keyword.
//! A historical id uses the built-in UI theme and syntax preset of that name.
//! `system` uses the theme that setting resolves to for the current OS scheme.

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::SystemScheme;
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::color_scheme::{LoadedScheme, scheme_named};
use crate::render::paint::{color, syntax_color};
use crate::render::panel::settings_selection;
use crate::settings::layout::{MenuGeom, Rect, SettingsLayout};
use crate::settings::{SettingKind, SettingsPanelState};

pub(crate) fn paint_bundled(
    name: &str,
    system: SystemScheme,
    rect: Rect,
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) {
    paint(
        crate::color_scheme::bundled_schemes(),
        name,
        system,
        rect,
        scale,
        quads,
    );
}

pub(crate) fn paint(
    schemes: &[LoadedScheme],
    name: &str,
    system: SystemScheme,
    rect: Rect,
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) {
    let resolved = if lux_ui::Chrome::follows_system(name) {
        lux_ui::Chrome::theme_name_for_scheme(system)
    } else {
        name
    };
    let Some(chips) = chip_colors(schemes, resolved) else {
        return;
    };
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

fn chip_colors(schemes: &[LoadedScheme], name: &str) -> Option<[[f32; 4]; 4]> {
    if let Some(scheme) = scheme_named(schemes, name) {
        let keyword = scheme.syntax.color_for_scope("keyword");
        return Some([
            color(scheme.theme.background),
            color(scheme.theme.foreground),
            color(scheme.theme.accent),
            syntax_color(keyword),
        ]);
    }
    let theme = lux_ui::Theme::by_name(name)?;
    let keyword = lux_syntax::theme::by_name(name)
        .map(|syntax| syntax.color_for_scope("keyword"))
        .map(syntax_color)
        .unwrap_or_else(|| color(theme.accent));
    Some([
        color(theme.background),
        color(theme.foreground),
        color(theme.accent),
        keyword,
    ])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_menu(
    menu: &MenuGeom,
    panel: &SettingsPanelState,
    layout: &SettingsLayout,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    family: &str,
    foreground: [f32; 4],
    well: [f32; 4],
    border: [f32; 4],
    theme: &lux_ui::Theme,
    system: SystemScheme,
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let Some(SettingKind::Enum { options, .. }) = panel.items.get(menu.item).map(|item| &item.kind)
    else {
        return;
    };
    let scale = layout.scale;
    super::settings::paint_button(menu.panel, well, border, scale, quads);
    let highlight = panel.menu.map(|state| state.highlight);
    for (offset, rect) in menu.options.iter().enumerate() {
        let index = menu.first + offset;
        let Some(option) = options.get(index) else {
            continue;
        };
        if highlight == Some(index) {
            quads.push(QuadInstance::rounded(
                [
                    rect.x,
                    rect.y + scale,
                    rect.w,
                    (rect.h - 2.0 * scale).max(1.0),
                ],
                settings_selection(theme),
                5.0 * scale,
            ));
        }
        paint_bundled(option, system, *rect, scale, quads);
        super::settings::draw_line(
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            family,
            13.0 * scale,
            option,
            rect.x + 8.0 * scale,
            rect.y + (rect.h - 13.0 * scale) * 0.5,
            if highlight == Some(index) {
                super::panel::label_ink(theme, settings_selection(theme))
            } else {
                foreground
            },
            glyphs,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 220.0,
            h: 30.0,
        }
    }

    fn first_chip(name: &str, system: SystemScheme) -> [f32; 4] {
        let mut quads = Vec::new();
        paint(
            crate::color_scheme::bundled_schemes(),
            name,
            system,
            rect(),
            1.0,
            &mut quads,
        );
        assert_eq!(quads.len(), 4, "{name}");
        quads[0].color
    }

    fn channel(value: u8) -> f32 {
        value as f32 / 255.0
    }

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
            SystemScheme::Dark,
            rect(),
            1.0,
            &mut quads,
        );
        assert_eq!(quads.len(), 4);
        assert_eq!(quads[3].color[0], channel(0xF9));
        paint(
            schemes,
            "not a theme",
            SystemScheme::Dark,
            rect(),
            1.0,
            &mut quads,
        );
        assert_eq!(quads.len(), 4);
    }

    #[test]
    fn historical_rows_and_system_use_the_palette_they_resolve_to() {
        let cases = [
            ("mariana", (0x30, 0x38, 0x41), (0xC5, 0x94, 0xC5)),
            ("breakers", (0x0C, 0x10, 0x14), (0xFF, 0x6B, 0x8B)),
            ("celeste", (0xF8, 0xF9, 0xFA), (0xA6, 0x26, 0xA4)),
            ("lux-dark", (0x12, 0x12, 0x1B), (0x9D, 0x7C, 0xD8)),
            ("lux-light", (0xFA, 0xFA, 0xFD), (0x7C, 0x3F, 0xC4)),
            ("high-contrast", (0x00, 0x00, 0x00), (0xFF, 0x6E, 0xC7)),
        ];
        for (name, background, keyword) in cases {
            let mut quads = Vec::new();
            paint(
                crate::color_scheme::bundled_schemes(),
                name,
                SystemScheme::Dark,
                rect(),
                1.0,
                &mut quads,
            );
            assert_eq!(quads.len(), 4, "{name}");
            assert_eq!(quads[0].color[0], channel(background.0), "{name}");
            assert_eq!(quads[0].color[1], channel(background.1), "{name}");
            assert_eq!(quads[0].color[2], channel(background.2), "{name}");
            assert_eq!(quads[3].color[0], channel(keyword.0), "{name}");
            assert_eq!(quads[3].color[1], channel(keyword.1), "{name}");
            assert_eq!(quads[3].color[2], channel(keyword.2), "{name}");
        }
        assert_eq!(first_chip("system", SystemScheme::Dark)[0], channel(0x12));
        assert_eq!(first_chip("system", SystemScheme::Light)[0], channel(0xFA));
    }
}
