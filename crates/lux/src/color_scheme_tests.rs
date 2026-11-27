use super::*;

#[test]
fn bundled_monokai_matches_the_builtin_constants() {
    let loaded = bundled_monokai();
    assert_eq!(loaded.theme, Theme::MONOKAI);
    for (scope, color) in lux_syntax::theme::MONOKAI.scopes {
        assert_eq!(
            loaded.syntax.color_for_scope(scope),
            *color,
            "{scope} must come from the scheme file"
        );
    }
    assert_eq!(
        loaded.syntax.bracket_colors,
        lux_syntax::theme::MONOKAI.bracket_colors
    );
}

#[test]
fn textmate_scopes_map_onto_lux_short_names() {
    let parsed = parse_scheme(
        r##"{
            "name": "Sample",
            "globals": { "background": "#111111", "foreground": "#EEEEEE" },
            "rules": [
                { "scope": "entity.name.function", "foreground": "#A6E22E" },
                { "scope": "comment.line", "foreground": "#75715E" }
            ]
        }"##,
    )
    .unwrap();
    assert_eq!(
        parsed.syntax.color_for_scope("function"),
        lux_syntax::theme::Color::rgb(0xA6, 0xE2, 0x2E)
    );
    assert_eq!(
        parsed.syntax.color_for_scope("comment"),
        lux_syntax::theme::Color::rgb(0x75, 0x71, 0x5E)
    );
    assert_eq!(
        parsed.theme.chrome.sidebar,
        lux_ui::shift(parsed.theme.background, -14)
    );
}

#[test]
fn a_scheme_without_a_background_is_refused() {
    assert!(parse_scheme(r#"{ "name": "Broken", "globals": {} }"#).is_err());
}

#[test]
fn bundled_schemes_keep_the_published_backgrounds() {
    let schemes = bundled_schemes();
    let mut seen = std::collections::HashSet::new();
    for scheme in schemes {
        let bg = scheme.theme.background;
        assert!(
            seen.insert((bg.r, bg.g, bg.b, bg.a)),
            "{} repeats a background",
            scheme.name
        );
    }
    let bg = |name| scheme_named(schemes, name).unwrap().theme.background;
    assert_eq!(bg("One Dark"), Color::rgb(0x28, 0x2C, 0x34));
    assert_eq!(bg("Dracula"), Color::rgb(0x28, 0x2A, 0x36));
    assert_eq!(bg("Solarized Light"), Color::rgb(0xFD, 0xF6, 0xE3));
    assert_eq!(bg("Nord"), Color::rgb(0x2E, 0x34, 0x40));
    assert_eq!(bg("Catppuccin"), Color::rgb(0x1E, 0x1E, 0x2E));
    assert_eq!(bg("GitHub Light"), Color::rgb(0xFF, 0xFF, 0xFF));
    assert_eq!(bg("Ayu"), Color::rgb(0x0B, 0x0E, 0x14));
    assert_eq!(bg("Rose Pine"), Color::rgb(0x19, 0x17, 0x24));
    assert!(theme_setting_options().contains(&"One Dark"));
    assert!(theme_setting_options().contains(&"monokai"));
    assert_eq!(theme_for(schemes, "Rose Pine").background, bg("Rose Pine"));
    assert_eq!(theme_for(schemes, "Nord").background, bg("Nord"));
    assert_eq!(theme_for(&[], "Mariana").name, "Mariana");
}

#[test]
fn bundled_monokai_uses_the_official_palette() {
    let loaded = bundled_monokai();
    assert_eq!(
        (
            loaded.theme.background.r,
            loaded.theme.background.g,
            loaded.theme.background.b,
            loaded.theme.background.a
        ),
        (0x27, 0x28, 0x22, 0xFF)
    );
    assert_eq!(
        (
            loaded.theme.foreground.r,
            loaded.theme.foreground.g,
            loaded.theme.foreground.b,
            loaded.theme.foreground.a
        ),
        (0xF8, 0xF8, 0xF2, 0xFF)
    );
    let scope = |name: &str| {
        let color = loaded.syntax.color_for_scope(name);
        (color.r, color.g, color.b)
    };
    assert_eq!(scope("keyword"), (0xF9, 0x26, 0x72));
    assert_eq!(scope("function"), (0xA6, 0xE2, 0x2E));
    assert_eq!(scope("type"), (0x66, 0xD9, 0xEF));
    assert_eq!(scope("string"), (0xE6, 0xDB, 0x74));
    assert_eq!(scope("constant"), (0xAE, 0x81, 0xFF));
    assert_eq!(scope("parameter"), (0xFD, 0x97, 0x1F));
    assert_eq!(scope("comment"), (0x75, 0x71, 0x5E));
}

/// Selection is the color painted over the editor, after straight-alpha
/// compositing. The floor is [`lux_ui::SELECTION_CONTRAST_MIN`] (1.30).
/// Sublime Monokai `#49483E` on `#272822` is 1.61. `#49483E88` paints
/// 1.28 and fails. VS Code Monokai `#878b9180` paints about 2.13.
#[test]
fn bundled_selections_clear_the_editor() {
    let schemes = bundled_schemes();
    assert_eq!(
        schemes.len(),
        14,
        "the contrast gate covers every bundled theme"
    );
    let floor = lux_ui::SELECTION_CONTRAST_MIN;
    for scheme in schemes {
        let painted = lux_ui::composite(scheme.theme.selection, scheme.theme.background);
        let ratio = lux_ui::contrast_ratio(painted, scheme.theme.background);
        assert!(
            ratio >= floor,
            "{} selection paints {ratio:.3}:1, below {floor}",
            scheme.name
        );
    }
    let monokai = scheme_named(schemes, "Monokai").unwrap().theme;
    assert_eq!(monokai.selection, Color::from_hex("#49483E").unwrap());
    let bg = Color::rgb(0x27, 0x28, 0x22);
    let faded = lux_ui::composite(Color::rgba(0x49, 0x48, 0x3E, 0x88), bg);
    assert!(
        lux_ui::contrast_ratio(faded, bg) < floor,
        "the old Monokai alpha must stay under the floor"
    );
}

/// The current line paints its highlight first and the selection on
/// top, so a selection on the caret's line has to clear the highlight,
/// not just the page (UI review item 13).
#[test]
fn bundled_selections_clear_the_current_line_highlight() {
    let floor = lux_ui::SELECTION_CONTRAST_MIN;
    for scheme in bundled_schemes() {
        let theme = scheme.theme;
        let line = lux_ui::composite(theme.chrome.line_highlight, theme.background);
        let painted = lux_ui::composite(theme.selection, line);
        let ratio = lux_ui::contrast_ratio(painted, line);
        assert!(
            ratio >= floor,
            "{} selection over the current line paints {ratio:.3}:1, below {floor}",
            scheme.name
        );
    }
}

#[test]
fn a_commented_scheme_keeps_an_array_of_scopes() {
    let text = r##"{
        // Sublime schemes are JSONC and list scopes as an array.
        "name": "Commented",
        "globals": {
            "background": "#111111",
            "foreground": "#eeeeee",
        },
        "rules": [
            { "scope": ["keyword", "storage.type"], "foreground": "#ff0000", }
        ]
    }"##;
    let scheme = parse_scheme(text).expect("scheme");
    let rgb = |name: &str| {
        let color = scheme.syntax.color_for_scope(name);
        (color.r, color.g, color.b)
    };
    assert_eq!(rgb("keyword"), (0xFF, 0x00, 0x00));
    assert_eq!(rgb("type"), (0xFF, 0x00, 0x00));
}

const USER_SCHEME: &str = r##"{
    "name": "Crowded",
    "globals": {
        "background": "#303841",
        "foreground": "#D8DEE9",
        "selection": "#4F5B6688",
        "line_highlight": "#3C444D"
    }
}"##;

/// A user scheme keeps the current-line color it sets, even one that
/// crowds its selection: the user chose it.
#[test]
fn a_user_scheme_keeps_its_explicit_line_highlight() {
    let theme = parse_scheme(USER_SCHEME).expect("parses").theme;
    let set = Color::from_hex("#3C444D").expect("hex");
    assert_eq!(theme.chrome.line_highlight, set);
    let fitted = lux_ui::fit_line_highlight(theme.selection, set, theme.background);
    assert_ne!(fitted, set, "the bundled rule would have faded it");
}

/// Without a `line_highlight`, nobody chose the derived one, so it is
/// fitted under the selection as a bundled scheme's is.
#[test]
fn a_user_scheme_without_a_line_highlight_gets_a_fitted_one() {
    let text = USER_SCHEME.replace(",\n        \"line_highlight\": \"#3C444D\"", "");
    assert!(!text.contains("line_highlight"));
    let theme = parse_scheme(&text).expect("parses").theme;
    let derived = lux_ui::shift(theme.background, 12);
    let fitted = lux_ui::fit_line_highlight(theme.selection, derived, theme.background);
    assert_ne!(fitted, derived);
    assert_eq!(theme.chrome.line_highlight, fitted);
}

/// Bundled schemes are fitted once at load: the stored color is what is
/// painted, and fitting it again changes nothing.
#[test]
fn bundled_line_highlights_are_fitted_when_they_load() {
    let mut faded = 0;
    for scheme in bundled_schemes() {
        let theme = scheme.theme;
        let line = theme.chrome.line_highlight;
        assert_eq!(
            lux_ui::fit_line_highlight(theme.selection, line, theme.background),
            line,
            "{}",
            scheme.name
        );
        let raw = parse_scheme_raw_line(scheme.name);
        faded += usize::from(raw != line);
    }
    assert!(faded > 0, "at least one bundled highlight needed the fade");
}

fn parse_scheme_raw_line(name: &str) -> Color {
    let file = format!(
        "{}/../../themes/{name}.sublime-color-scheme",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(file).expect("bundled scheme file");
    parse_scheme(&text)
        .expect("parses")
        .theme
        .chrome
        .line_highlight
}
