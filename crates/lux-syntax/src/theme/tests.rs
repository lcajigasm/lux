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

fn monokai_token(lang: crate::LanguageId, source: &str, needle: &str, scope: &str) {
    let mut doc = crate::Document::new(lang);
    doc.set_text(source);
    let tokens = doc.highlight(source);
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("{}: fixture has no {needle:?}", lang.name()));
    let end = start + needle.len();
    let token = tokens
        .iter()
        .find(|token| {
            token.start_byte <= start
                && token.end_byte >= end
                && (token.kind == scope || token.kind.starts_with(&format!("{scope}.")))
        })
        .unwrap_or_else(|| {
            panic!(
                "{}: {needle:?} is not scope {scope}. tokens: {:?}",
                lang.name(),
                tokens
                    .iter()
                    .map(|token| {
                        format!("{}..{} {}", token.start_byte, token.end_byte, token.kind)
                    })
                    .collect::<Vec<_>>()
            )
        });
    assert_eq!(
        MONOKAI.color_for_token(token),
        MONOKAI.color_for_scope(scope),
        "{}: Monokai should color {scope}",
        lang.name()
    );
}

#[test]
fn f1_monokai_colors_current_language_tokens() {
    use crate::LanguageId::*;
    let cases = [
        (Rust, "fn main() {}\n", "fn", "keyword"),
        (TypeScript, "function main() {}\n", "function", "keyword"),
        (Tsx, "const A = <div />;\n", "const", "keyword"),
        (JavaScript, "function main() {}\n", "function", "keyword"),
        (Python, "def main():\n    return \"hi\"\n", "def", "keyword"),
        (Go, "package main\n", "package", "keyword"),
        (C, "int main() { return 0; }\n", "return", "keyword"),
        (Cpp, "int main() { return 0; }\n", "return", "keyword"),
        (Java, "class Main {}\n", "class", "keyword"),
        (Kotlin, "fun main() {}\n", "fun", "keyword"),
        (Swift, "func main() {}\n", "func", "keyword"),
        (Html, "<div>hi</div>\n", "div", "tag"),
        (Css, "/* note */\n", "note", "comment"),
        (Json, "{ \"name\": 1 }\n", "name", "string"),
        (Yaml, "name: \"hi\"\n", "hi", "string"),
        (Markdown, "# Title\n", "Title", "text.title"),
        (Bash, "# note\n", "note", "comment"),
        (Toml, "name = \"hi\"\n", "hi", "string"),
        (Php, "<?php echo \"hi\";\n", "echo", "keyword"),
        (Ruby, "def main\nend\n", "def", "keyword"),
        (Sql, "SELECT 1;\n", "SELECT", "keyword"),
        (Xml, "<note>hi</note>\n", "note", "tag"),
        (Dockerfile, "FROM alpine\n", "FROM", "keyword"),
        (Make, "export CC=gcc\n", "export", "keyword"),
        (Lua, "local function main() end\n", "function", "keyword"),
        (CSharp, "class Main {}\n", "class", "keyword"),
        (Diff, "index 1111111..2222222 100644\n", "index", "keyword"),
        (Zig, "pub fn main() void {}\n", "fn", "keyword"),
        (Graphql, "# note\n", "note", "comment"),
        (Ini, "; note\n", "note", "comment"),
        (Proto, "syntax = \"proto3\";\n", "syntax", "keyword"),
        (Hcl, "# note\n", "note", "comment"),
        (Nix, "# note\n", "note", "comment"),
        (Cmake, "# note\n", "note", "comment"),
        (Gitignore, "# note\n", "note", "comment"),
        (Scss, "// note\n", "note", "comment"),
        (Prisma, "// note\n", "note", "comment"),
        (Fish, "# note\n", "note", "comment"),
        (Scheme, "; note\n", "note", "comment"),
        (Clojure, "; note\n", "note", "comment"),
        (Regex, "z+\n", "z", "string"),
        (Erlang, "% note\n", "note", "comment"),
        (Gleam, "// note\n", "note", "comment"),
        (R, "# note\n", "note", "comment"),
        (Elixir, "# note\n", "note", "comment"),
        (Scala, "// note\n", "note", "comment"),
        (Dart, "void main() {}\n", "void", "keyword"),
        (Julia, "# note\n", "note", "comment"),
        (Haskell, "-- note\n", "note", "comment"),
        (Ocaml, "(* note *)\n", "note", "comment"),
        (CommonLisp, "; note\n", "note", "comment"),
    ];
    let mut seen = Vec::new();
    for (lang, source, needle, scope) in cases {
        monokai_token(lang, source, needle, scope);
        seen.push(lang);
    }
    for lang in crate::LanguageId::ALL {
        assert!(
            seen.contains(lang),
            "{} has no Monokai token fixture",
            lang.name()
        );
    }
}
