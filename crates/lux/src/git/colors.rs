//! Git state colors, derived from the active theme.
//!
//! Each state has a fixed hue (Sublime Text's conventions: green for added,
//! orange for modified, red for removed and conflicted). The hue is then
//! lightened on a dark surface or darkened on a light one until it reaches
//! a WCAG contrast ratio against the surface it is painted on: 4.5:1 for a
//! file name (text), 3:1 for a gutter bar (a non-text indicator). Ignored
//! rows take the theme's own `muted` color, the same dimming line numbers use.

use lux_ui::Color;

use super::marks::GitMark;

const ADDED: Color = Color::rgb(0x4E, 0xC9, 0x9B);
const MODIFIED: Color = Color::rgb(0xE8, 0x9F, 0x4C);
const REMOVED: Color = Color::rgb(0xE0, 0x55, 0x61);

/// WCAG 2 contrast for normal-size text.
pub(crate) const TEXT_CONTRAST: f32 = 4.5;
/// WCAG 2.1 non-text contrast (1.4.11).
pub(crate) const MARK_CONTRAST: f32 = 3.0;

/// What the gutter paints for each kind of hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GutterKind {
    Added,
    Modified,
    Removed,
}

pub(crate) fn gutter_color(kind: GutterKind, surface: Color) -> Color {
    let hue = match kind {
        GutterKind::Added => ADDED,
        GutterKind::Modified => MODIFIED,
        GutterKind::Removed => REMOVED,
    };
    legible(hue, surface, MARK_CONTRAST)
}

/// The color of a tree row's name and letter for `mark`.
pub(crate) fn mark_color(mark: GitMark, theme: &lux_ui::Theme) -> Color {
    let hue = match mark {
        GitMark::Ignored => return theme.muted,
        GitMark::Untracked | GitMark::Added => ADDED,
        GitMark::Modified => MODIFIED,
        GitMark::Deleted | GitMark::Conflicted => REMOVED,
    };
    legible(hue, theme.chrome.sidebar, TEXT_CONTRAST)
}

fn channel(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color) -> f32 {
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

pub(crate) fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::rgb(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b))
}

/// `hue`, moved toward white on a dark `surface` or black on a light one,
/// in the smallest step that reaches `target`.
fn legible(hue: Color, surface: Color, target: f32) -> Color {
    let toward = if luminance(surface) < 0.18 {
        Color::rgb(0xFF, 0xFF, 0xFF)
    } else {
        Color::rgb(0x00, 0x00, 0x00)
    };
    (0..=20)
        .map(|step| mix(hue, toward, step as f32 / 20.0))
        .find(|candidate| contrast(*candidate, surface) >= target)
        .unwrap_or(toward)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_ui::Theme;

    const THEMES: [Theme; 7] = [
        Theme::LUX_DARK,
        Theme::LUX_DARK_HIGH_CONTRAST,
        Theme::LUX_LIGHT,
        Theme::MONOKAI,
        Theme::MARIANA,
        Theme::BREAKERS,
        Theme::CELESTE,
    ];

    #[test]
    fn every_tree_state_is_legible_on_every_builtin_sidebar() {
        for theme in THEMES {
            for mark in [
                GitMark::Untracked,
                GitMark::Added,
                GitMark::Modified,
                GitMark::Deleted,
                GitMark::Conflicted,
            ] {
                let fg = mark_color(mark, &theme);
                let ratio = contrast(fg, theme.chrome.sidebar);
                assert!(ratio >= TEXT_CONTRAST, "{} {mark:?}: {ratio}", theme.name);
            }
        }
    }

    #[test]
    fn every_gutter_mark_is_visible_on_every_builtin_gutter() {
        for theme in THEMES {
            for kind in [GutterKind::Added, GutterKind::Modified, GutterKind::Removed] {
                let ratio = contrast(gutter_color(kind, theme.chrome.gutter), theme.chrome.gutter);
                assert!(ratio >= MARK_CONTRAST, "{} {kind:?}: {ratio}", theme.name);
            }
        }
    }

    #[test]
    fn modified_reads_orange_and_states_stay_distinct() {
        let theme = Theme::LUX_DARK;
        let modified = mark_color(GitMark::Modified, &theme);
        assert!(
            modified.r > modified.b && modified.r > modified.g,
            "{modified:?}"
        );
        assert_ne!(modified, mark_color(GitMark::Added, &theme));
        assert_ne!(modified, mark_color(GitMark::Conflicted, &theme));
        assert_eq!(mark_color(GitMark::Ignored, &theme), theme.muted);
    }
}
