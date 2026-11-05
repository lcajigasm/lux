//! Load a `.sublime-color-scheme` into a UI theme and a syntax theme.
//!
//! The bundled Monokai file is the runtime source of the default theme.
//! Further schemes are read from `<settings dir>/themes/`. Names and scope
//! tables are leaked for the process lifetime so the existing `'static`
//! theme APIs can hold them.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use lux_syntax::theme::SyntaxTheme;
use lux_ui::{ChromeColors, Color, Theme};
use serde_json::Value;

/// One scheme the theme picker and `set_theme` can resolve by name.
#[derive(Clone, Copy)]
pub(crate) struct LoadedScheme {
    pub(crate) name: &'static str,
    pub(crate) theme: Theme,
    pub(crate) syntax: &'static SyntaxTheme,
}

/// The vendored Monokai scheme, parsed once from the file in the repo.
pub(crate) fn bundled_monokai() -> LoadedScheme {
    static SCHEME: OnceLock<LoadedScheme> = OnceLock::new();
    *SCHEME.get_or_init(|| {
        parse_scheme(include_str!("../../../themes/Monokai.sublime-color-scheme"))
            .expect("bundled Monokai.sublime-color-scheme parses")
    })
}

/// Reads every `*.sublime-color-scheme` in `dir`. A file that does not
/// parse is skipped; it is not turned into a blank theme.
pub(crate) fn load_user_schemes(dir: &Path) -> Vec<LoadedScheme> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("sublime-color-scheme") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        match parse_scheme(&text) {
            Ok(scheme) => out.push(scheme),
            Err(error) => {
                eprintln!("Lux: skipping theme {}: {error}", path.display());
            }
        }
    }
    out
}

pub(crate) fn scheme_named(schemes: &[LoadedScheme], name: &str) -> Option<LoadedScheme> {
    let needle = normalize_name(name);
    schemes
        .iter()
        .rev()
        .copied()
        .find(|scheme| normalize_name(scheme.name) == needle)
}

pub(crate) fn syntax_theme(
    schemes: &[LoadedScheme],
    name: &str,
) -> &'static lux_syntax::theme::SyntaxTheme {
    scheme_named(schemes, name)
        .map(|scheme| scheme.syntax)
        .or_else(|| lux_syntax::theme::by_name(name))
        .unwrap_or(&lux_syntax::theme::LUX_DARK)
}

pub(crate) fn theme_names(schemes: &[LoadedScheme]) -> Vec<&'static str> {
    let mut names = crate::theme_picker::all_theme_names();
    for scheme in schemes {
        if !names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(scheme.name))
        {
            names.push(scheme.name);
        }
    }
    names
}

pub(crate) fn normalize_name(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "")
}

pub(crate) fn parse_scheme(text: &str) -> Result<LoadedScheme, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Custom")
        .to_owned();
    let globals = value.get("globals").cloned().unwrap_or(Value::Null);
    let background = required_color(&globals, "background")?;
    let foreground = required_color(&globals, "foreground")?;
    let accent = optional_color(&globals, "accent").unwrap_or(foreground);
    let selection = optional_color(&globals, "selection")
        .unwrap_or(Color::rgba(accent.r, accent.g, accent.b, 0x55));
    let muted = optional_color(&globals, "muted").unwrap_or(foreground);
    let chrome = ChromeColors {
        sidebar: optional_color(&globals, "sidebar").unwrap_or(lux_ui::shift(background, -14)),
        tab_bar: optional_color(&globals, "tab_bar").unwrap_or(lux_ui::shift(background, -16)),
        tab_inactive: optional_color(&globals, "tab_inactive")
            .unwrap_or(lux_ui::shift(background, -8)),
        status_bar: optional_color(&globals, "status_bar")
            .unwrap_or(lux_ui::shift(background, -16)),
        panel: optional_color(&globals, "panel").unwrap_or(lux_ui::shift(background, -12)),
        panel_input: optional_color(&globals, "panel_input")
            .unwrap_or(lux_ui::shift(background, 18)),
        panel_row: optional_color(&globals, "panel_row").unwrap_or(lux_ui::shift(background, 32)),
        border: optional_color(&globals, "border").unwrap_or(lux_ui::shift(background, 24)),
        gutter: optional_color(&globals, "gutter").unwrap_or(background),
        line_highlight: optional_color(&globals, "line_highlight")
            .unwrap_or(lux_ui::shift(background, 12)),
    };
    let leaked_name = leak_str(name);
    let theme = Theme {
        name: leaked_name,
        background,
        foreground,
        accent,
        selection,
        muted,
        chrome,
    };

    let mut scopes: Vec<(&'static str, lux_syntax::theme::Color)> = Vec::new();
    if let Some(rules) = value.get("rules").and_then(Value::as_array) {
        for rule in rules {
            let Some(raw_scope) = rule.get("scope").and_then(Value::as_str) else {
                continue;
            };
            let Some(hex) = rule.get("foreground").and_then(Value::as_str).or_else(|| {
                rule.get("settings")
                    .and_then(|s| s.get("foreground"))
                    .and_then(Value::as_str)
            }) else {
                continue;
            };
            let Some(color) = syntax_color(hex) else {
                continue;
            };
            for piece in raw_scope.split(',') {
                if let Some(scope) = lux_scope(piece.trim()) {
                    if let Some(existing) = scopes.iter_mut().find(|(name, _)| *name == scope) {
                        existing.1 = color;
                    } else {
                        scopes.push((scope, color));
                    }
                }
            }
        }
    }
    let scope_color = |scope: &str, fallback: lux_syntax::theme::Color| {
        scopes
            .iter()
            .find(|(name, _)| *name == scope)
            .map(|(_, color)| *color)
            .unwrap_or(fallback)
    };
    let fg = syntax_color_of(foreground);
    let brackets = [
        scope_color("keyword", fg),
        scope_color("function", fg),
        scope_color("type", fg),
        scope_color("parameter", fg),
        scope_color("number", fg),
        scope_color("string", fg),
    ];
    let syntax = Box::leak(Box::new(SyntaxTheme {
        name: leaked_name,
        background: syntax_color_of(background),
        default: fg,
        scopes: Box::leak(scopes.into_boxed_slice()),
        bracket_colors: Box::leak(Box::new(brackets)),
    }));
    Ok(LoadedScheme {
        name: leaked_name,
        theme,
        syntax,
    })
}

fn required_color(globals: &Value, key: &str) -> Result<Color, String> {
    optional_color(globals, key).ok_or_else(|| format!("missing globals.{key}"))
}

fn optional_color(globals: &Value, key: &str) -> Option<Color> {
    globals
        .get(key)
        .and_then(Value::as_str)
        .and_then(Color::from_hex)
}

fn syntax_color(hex: &str) -> Option<lux_syntax::theme::Color> {
    let ui = Color::from_hex(hex)?;
    Some(lux_syntax::theme::Color::rgb(ui.r, ui.g, ui.b))
}

fn syntax_color_of(color: Color) -> lux_syntax::theme::Color {
    lux_syntax::theme::Color::rgb(color.r, color.g, color.b)
}

fn leak_str(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn lux_scope(scope: &str) -> Option<&'static str> {
    const SHORT: &[&str] = &[
        "comment",
        "keyword",
        "conditional",
        "repeat",
        "import",
        "include",
        "string",
        "character",
        "charset",
        "escape",
        "number",
        "float",
        "boolean",
        "constant",
        "type",
        "function",
        "constructor",
        "variable",
        "parameter",
        "property",
        "attribute",
        "namespace",
        "media",
        "supports",
        "keyframes",
        "tag",
        "label",
        "exception",
        "operator",
        "punctuation",
        "delimiter",
        "embedded",
        "spell",
        "text",
        "text.title",
        "text.strong",
        "text.emphasis",
        "text.literal",
        "text.uri",
        "text.reference",
    ];
    if let Some(found) = SHORT.iter().find(|name| **name == scope) {
        return Some(found);
    }
    Some(match scope {
        s if s.starts_with("comment") => "comment",
        s if s.starts_with("string") => "string",
        s if s.starts_with("constant.numeric") => "number",
        s if s.starts_with("constant") => "constant",
        s if s.starts_with("keyword") => "keyword",
        s if s.starts_with("storage.type") => "type",
        s if s.starts_with("storage") => "keyword",
        s if s.starts_with("entity.name.function") || s.starts_with("support.function") => {
            "function"
        }
        s if s.starts_with("entity.name.type")
            || s.starts_with("entity.name.class")
            || s.starts_with("support.type")
            || s.starts_with("support.class") =>
        {
            "type"
        }
        s if s.starts_with("variable.parameter") => "parameter",
        s if s.starts_with("entity.name.tag") => "tag",
        s if s.starts_with("entity.other.attribute") => "attribute",
        s if s.starts_with("punctuation") => "punctuation",
        s if s.starts_with("variable") => "variable",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
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
}
