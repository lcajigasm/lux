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
fn sublime_themes_clear_the_wcag_aa_bar() {
    for theme in [&MONOKAI, &MARIANA, &BREAKERS, &CELESTE] {
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
    let mut doc = crate::Document::new(crate::LanguageId::Rust);
    let source = include_str!("../../tests/fixtures/rust/sample.rs");
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

#[test]
fn by_name_resolves_all_syntax_themes() {
    assert_eq!(by_name("monokai").map(|t| t.name), Some("Monokai"));
    assert_eq!(by_name("Monokai").map(|t| t.name), Some("Monokai"));
    assert_eq!(by_name("mariana").map(|t| t.name), Some("Mariana"));
    assert_eq!(by_name("Mariana").map(|t| t.name), Some("Mariana"));
    assert_eq!(by_name("breakers").map(|t| t.name), Some("Breakers"));
    assert_eq!(by_name("Breakers").map(|t| t.name), Some("Breakers"));
    assert_eq!(by_name("celeste").map(|t| t.name), Some("Celeste"));
    assert_eq!(by_name("Celeste").map(|t| t.name), Some("Celeste"));
    assert_eq!(by_name("lux-dark").map(|t| t.name), Some("Lux Dark"));
    assert_eq!(by_name("Lux Dark").map(|t| t.name), Some("Lux Dark"));
    assert_eq!(by_name("lux-light").map(|t| t.name), Some("Lux Light"));
    assert_eq!(by_name("Lux Light").map(|t| t.name), Some("Lux Light"));
    assert_eq!(
        by_name("high-contrast").map(|t| t.name),
        Some("Lux High Contrast")
    );
    assert!(by_name("unknown-theme").is_none());
}
