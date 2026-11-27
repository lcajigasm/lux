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
        fit_bundled(
            parse_scheme(include_str!("../../../themes/Monokai.sublime-color-scheme"))
                .expect("bundled Monokai.sublime-color-scheme parses"),
        )
    })
}

/// Every scheme compiled into the binary. Monokai stays first.
pub(crate) fn bundled_schemes() -> &'static [LoadedScheme] {
    static SCHEMES: OnceLock<Vec<LoadedScheme>> = OnceLock::new();
    SCHEMES.get_or_init(|| {
        let mut out = vec![bundled_monokai()];
        for text in [
            include_str!("../../../themes/One Dark.sublime-color-scheme"),
            include_str!("../../../themes/Dracula.sublime-color-scheme"),
            include_str!("../../../themes/Solarized Dark.sublime-color-scheme"),
            include_str!("../../../themes/Solarized Light.sublime-color-scheme"),
            include_str!("../../../themes/Nord.sublime-color-scheme"),
            include_str!("../../../themes/Gruvbox.sublime-color-scheme"),
            include_str!("../../../themes/Tokyo Night.sublime-color-scheme"),
            include_str!("../../../themes/Catppuccin.sublime-color-scheme"),
            include_str!("../../../themes/Catppuccin Latte.sublime-color-scheme"),
            include_str!("../../../themes/GitHub Dark.sublime-color-scheme"),
            include_str!("../../../themes/GitHub Light.sublime-color-scheme"),
            include_str!("../../../themes/Ayu.sublime-color-scheme"),
            include_str!("../../../themes/Rose Pine.sublime-color-scheme"),
        ] {
            out.push(fit_bundled(
                parse_scheme(text).expect("bundled color scheme parses"),
            ));
        }
        out
    });
    SCHEMES.get().expect("bundled schemes").as_slice()
}

/// A bundled scheme's current-line highlight, faded toward the page as far
/// as its selection needs to clear `SELECTION_CONTRAST_MIN` over it (the
/// selection paints above the highlight). Done once, when the scheme loads;
/// painting reads the stored color. User schemes keep the highlight they
/// set: see [`parse_scheme`].
fn fit_bundled(mut scheme: LoadedScheme) -> LoadedScheme {
    let theme = &mut scheme.theme;
    theme.chrome.line_highlight = lux_ui::fit_line_highlight(
        theme.selection,
        theme.chrome.line_highlight,
        theme.background,
    );
    scheme
}

/// Settings menu values: the historical ids, then each bundled display name.
pub(crate) fn theme_setting_options() -> &'static [&'static str] {
    static OPTIONS: OnceLock<Vec<&'static str>> = OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut names = vec![
            "system",
            "monokai",
            "mariana",
            "breakers",
            "celeste",
            "lux-dark",
            "lux-light",
            "high-contrast",
        ];
        for scheme in bundled_schemes() {
            if names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(scheme.name))
            {
                continue;
            }
            names.push(scheme.name);
        }
        names
    });
    OPTIONS.get().expect("theme options").as_slice()
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

/// The theme a picker row should preview. Loaded schemes win, then the
/// seven built-in themes (Mariana, Celeste, and the rest have no scheme file).
pub(crate) fn theme_for(schemes: &[LoadedScheme], name: &str) -> Theme {
    scheme_named(schemes, name)
        .map(|scheme| scheme.theme)
        .or_else(|| Theme::by_name(name))
        .unwrap_or(Theme::MONOKAI)
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
    let value: Value =
        lux_config::layering::jsonc::parse(text).map_err(|error| error.to_string())?;
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
        // An explicit `line_highlight` is kept as written. Without one, the
        // derived highlight is fitted under the selection like a bundled one.
        line_highlight: optional_color(&globals, "line_highlight").unwrap_or_else(|| {
            lux_ui::fit_line_highlight(selection, lux_ui::shift(background, 12), background)
        }),
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
            let listed = scope_list(rule.get("scope"));
            if listed.is_empty() {
                continue;
            }
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
            for piece in listed {
                if let Some(scope) = lux_scope(&piece) {
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

fn scope_list(value: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    match value {
        Some(Value::String(text)) => push_scopes(&mut out, text),
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(text) = item.as_str() {
                    push_scopes(&mut out, text);
                }
            }
        }
        _ => {}
    }
    out
}

fn push_scopes(out: &mut Vec<String>, text: &str) {
    for piece in text.split(',') {
        let piece = piece.trim();
        if !piece.is_empty() {
            out.push(piece.to_string());
        }
    }
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
#[path = "color_scheme_tests.rs"]
mod tests;
