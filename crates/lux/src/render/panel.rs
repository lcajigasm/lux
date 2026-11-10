//! Rounded selector surfaces. Radius 0 is not used here: these panels are
//! the command palette, goto, theme picker, and completion list.

use lux_render::QuadInstance;

/// Rows kept in sync with the hit tests in `modal`.
pub(crate) const SELECTOR_VISIBLE: usize = 8;
pub(crate) const THEME_VISIBLE: usize = 7;

/// Resting geometry. The open animation offsets `top` while drawing and
/// does not move the hit target.
pub(crate) struct SelectorBox {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub input_height: f32,
    pub row_height: f32,
    pub radius: f32,
}

/// About half the window, never a narrow strip and never wider than half.
pub(crate) fn selector_box(viewport_width: f32, tab_bar_height: f32, scale: f32) -> SelectorBox {
    let width = (viewport_width * 0.46).clamp(viewport_width * 0.40, viewport_width * 0.50);
    SelectorBox {
        left: (viewport_width - width) * 0.5,
        // Flush under the tab bar. A 52px gap put the card over the
        // middle of the code; Sublime and Zed sit on the tab strip.
        top: tab_bar_height + 8.0 * scale,
        width,
        input_height: 46.0 * scale,
        row_height: 36.0 * scale,
        radius: 12.0 * scale,
    }
}

pub(crate) fn panel_height(input: f32, row: f32, visible: usize, scale: f32) -> f32 {
    input + visible as f32 * row + 8.0 * scale
}

pub(crate) fn rows_top(top: f32, input: f32, scale: f32) -> f32 {
    top + input + 2.0 * scale
}

/// Card lifted off the editor (`#414339` on `#272822`) with a light rim.
pub(crate) fn card_fill(theme: &lux_ui::Theme) -> [f32; 4] {
    color(theme.chrome.panel_input)
}

pub(crate) fn field_fill(theme: &lux_ui::Theme) -> [f32; 4] {
    color(theme.background)
}

pub(crate) fn selected_fill(theme: &lux_ui::Theme) -> [f32; 4] {
    color(theme.chrome.panel_row)
}

/// WCAG contrast ratio. 4.5 is AA for normal text.
pub(crate) fn contrast_ratio(a: [f32; 4], b: [f32; 4]) -> f32 {
    fn channel(v: f32) -> f32 {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    fn luminance(c: [f32; 4]) -> f32 {
        0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
    }
    let left = luminance(a);
    let right = luminance(b);
    let (hi, lo) = if left > right {
        (left, right)
    } else {
        (right, left)
    };
    (hi + 0.05) / (lo + 0.05)
}

/// Moves `ink` toward black or white until it clears WCAG AA on
/// `background`. The theme color is kept when it already clears 4.8,
/// which leaves a little room for antialiasing.
/// A control fill that still reads as a control after the snapshot
/// checker quantizes each channel with `& ~7` and allows 10 levels of
/// slack. A theme whose `panel_input` sits on `panel` (Ayu, Solarized,
/// GitHub Light) otherwise paints a search field with no edge.
pub(crate) fn distinct_fill(fill: [f32; 4], page: [f32; 4]) -> [f32; 4] {
    if quant_gap(fill, page) > 18 {
        return fill;
    }
    let sum = page[0] + page[1] + page[2];
    let target = if sum < 1.5 {
        [1.0, 1.0, 1.0, 1.0]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    };
    let mut best = fill;
    for step in 1..=20 {
        let t = step as f32 / 20.0;
        let mixed = [
            fill[0] + (target[0] - fill[0]) * t,
            fill[1] + (target[1] - fill[1]) * t,
            fill[2] + (target[2] - fill[2]) * t,
            1.0,
        ];
        best = mixed;
        if quant_gap(mixed, page) > 18 {
            return mixed;
        }
    }
    best
}

fn quant_gap(a: [f32; 4], b: [f32; 4]) -> i32 {
    (0..3)
        .map(|channel| (quant_channel(a[channel]) - quant_channel(b[channel])).abs())
        .max()
        .unwrap_or(0)
}

fn quant_channel(value: f32) -> i32 {
    let byte = (value.clamp(0.0, 1.0) * 255.0).round() as i32;
    byte & !7
}

pub(crate) fn readable_ink(ink: [f32; 4], background: [f32; 4]) -> [f32; 4] {
    if contrast_ratio(ink, background) >= 4.8 {
        return ink;
    }
    let mut best = ink;
    let mut best_ratio = contrast_ratio(ink, background);
    for target in [[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]] {
        for step in 1..=16 {
            let t = step as f32 / 16.0;
            let mixed = [
                ink[0] + (target[0] - ink[0]) * t,
                ink[1] + (target[1] - ink[1]) * t,
                ink[2] + (target[2] - ink[2]) * t,
                1.0,
            ];
            let ratio = contrast_ratio(mixed, background);
            if ratio >= 4.8 {
                return mixed;
            }
            if ratio > best_ratio {
                best = mixed;
                best_ratio = ratio;
            }
        }
    }
    best
}

pub(crate) fn rim_color() -> [f32; 4] {
    color(lux_ui::Color::rgb(0xC2, 0xC0, 0xA8))
}

pub(crate) fn placeholder_ink(theme: &lux_ui::Theme) -> [f32; 4] {
    let ink = color(theme.foreground);
    [ink[0] * 0.78, ink[1] * 0.78, ink[2] * 0.72, 1.0]
}

/// Match ink has to stay readable on the row it sits on. The accent is
/// used only when it clears WCAG AA against that row.
pub(crate) fn hit_ink(on_selection: bool, theme: &lux_ui::Theme) -> [f32; 4] {
    let background = if on_selection {
        selected_fill(theme)
    } else {
        card_fill(theme)
    };
    let accent = color(theme.accent);
    let foreground = color(theme.foreground);
    let chosen = if contrast_ratio(accent, background) >= 4.5 {
        accent
    } else {
        foreground
    };
    readable_ink(chosen, background)
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    #[test]
    fn palette_ink_meets_wcag_aa() {
        for theme in [
            lux_ui::Theme::MONOKAI,
            lux_ui::Theme::CELESTE,
            lux_ui::Theme::LUX_LIGHT,
            lux_ui::Theme::LUX_DARK,
        ] {
            let card = card_fill(&theme);
            let row = selected_fill(&theme);
            let text = color(theme.foreground);
            assert!(
                contrast_ratio(text, card) >= 4.5,
                "{} text/card {}",
                theme.name,
                contrast_ratio(text, card)
            );
            assert!(
                contrast_ratio(text, row) >= 4.5,
                "{} text/row {}",
                theme.name,
                contrast_ratio(text, row)
            );
            assert!(contrast_ratio(hit_ink(false, &theme), card) >= 4.5);
            assert!(contrast_ratio(hit_ink(true, &theme), row) >= 4.5);
            assert!(
                contrast_ratio(readable_ink(text, row), row) >= 4.5,
                "{}",
                theme.name
            );
        }
        let solarized = color(lux_ui::Color::from_hex("#657b83").unwrap());
        let paper = color(lux_ui::Color::from_hex("#EEE8D5").unwrap());
        assert!(contrast_ratio(readable_ink(solarized, paper), paper) >= 4.5);
    }

    #[test]
    fn a_search_well_clears_the_page_after_quantization() {
        let rgb = |hex: &str| color(lux_ui::Color::from_hex(hex).unwrap());
        for (page, well) in [
            ("#0d1017", "#11151c"),
            ("#073642", "#073642"),
            ("#eee8d5", "#eee8d5"),
            ("#f6f8fa", "#f6f8fa"),
            ("#1f1d2e", "#26233a"),
            ("#1e1f1c", "#414339"),
        ] {
            let page = rgb(page);
            let out = distinct_fill(rgb(well), page);
            assert!(quant_gap(out, page) > 18, "{page:?} -> {out:?}");
        }
        let monokai_page = rgb("#1e1f1c");
        let monokai_well = rgb("#414339");
        let kept = distinct_fill(monokai_well, monokai_page);
        assert_eq!(kept, monokai_well);
    }

    #[test]
    fn glyphs_outside_the_window_are_dropped() {
        let glyph = |x| {
            lux_render::GlyphInstance::new(
                [x, 10.0, 8.0, 12.0],
                [0.0, 0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
                lux_render::GlyphKind::Monochrome,
            )
        };
        let kept = crate::render::paint::clip_glyphs(&[glyph(1200.0), glyph(12.0)], 1000.0, 700.0);
        assert_eq!(kept.len(), 1);
        assert!((kept[0].rect[0] - 12.0).abs() < 0.1);
    }
}

pub(crate) fn shortcut_ink(theme: &lux_ui::Theme) -> [f32; 4] {
    let ink = color(theme.foreground);
    [ink[0] * 0.94, ink[1] * 0.93, ink[2] * 0.84, 1.0]
}

fn color(c: lux_ui::Color) -> [f32; 4] {
    super::paint::color(c)
}

pub(crate) struct PaintedSelector {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub input_height: f32,
    pub row_height: f32,
    pub rows_top: f32,
}

/// Shadow, rim, lifted fill, and a recessed input well.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_selector_chrome(
    quads: &mut Vec<QuadInstance>,
    viewport_width: f32,
    tab_bar_height: f32,
    scale: f32,
    stroke: f32,
    visible: usize,
    open_offset: f32,
    theme: &lux_ui::Theme,
) -> PaintedSelector {
    let box_ = selector_box(viewport_width, tab_bar_height, scale);
    let top = box_.top + open_offset;
    let height = panel_height(box_.input_height, box_.row_height, visible, scale);
    push_rounded_surface(
        quads,
        [box_.left, top, box_.width, height],
        box_.radius,
        card_fill(theme),
        rim_color(),
        20.0 * scale,
        stroke.max(1.5 * scale),
    );
    let pad = 10.0 * scale;
    let field_h = box_.input_height - 14.0 * scale;
    push_rounded_surface(
        quads,
        [
            box_.left + pad,
            top + 8.0 * scale,
            box_.width - 2.0 * pad,
            field_h,
        ],
        8.0 * scale,
        field_fill(theme),
        rim_color(),
        0.0,
        stroke.max(1.0),
    );
    PaintedSelector {
        left: box_.left,
        top,
        width: box_.width,
        input_height: box_.input_height,
        row_height: box_.row_height,
        rows_top: rows_top(top, box_.input_height, scale),
    }
}

pub(crate) fn push_rounded_surface(
    quads: &mut Vec<QuadInstance>,
    rect: [f32; 4],
    radius: f32,
    fill: [f32; 4],
    border: [f32; 4],
    shadow: f32,
    stroke: f32,
) {
    if shadow > 0.0 {
        let grow = shadow * 0.45;
        quads.push(QuadInstance::rounded(
            [
                rect[0] - grow * 0.15,
                rect[1] + shadow * 0.42,
                rect[2] + grow * 0.35,
                rect[3] + shadow * 0.28,
            ],
            super::paint::faded([0.0, 0.0, 0.0, 0.78]),
            radius + grow * 0.2,
        ));
    }
    quads.push(QuadInstance::rounded(
        rect,
        super::paint::faded(border),
        radius,
    ));
    let inset = stroke.max(1.0);
    quads.push(QuadInstance::rounded(
        [
            rect[0] + inset,
            rect[1] + inset,
            (rect[2] - 2.0 * inset).max(1.0),
            (rect[3] - 2.0 * inset).max(1.0),
        ],
        super::paint::faded(fill),
        (radius - inset).max(0.0),
    ));
}

/// One moving selection row. `visual_index` is fractional while the
/// highlight is easing between rows.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_moving_row(
    quads: &mut Vec<QuadInstance>,
    left: f32,
    rows_top: f32,
    width: f32,
    row_height: f32,
    visual_index: f32,
    start_idx: usize,
    visible: usize,
    fill: [f32; 4],
    accent: [f32; 4],
    scale: f32,
) {
    if visible == 0 {
        return;
    }
    let y = rows_top + (visual_index - start_idx as f32) * row_height;
    let bottom = rows_top + visible as f32 * row_height;
    if y + row_height <= rows_top || y >= bottom {
        return;
    }
    let inset = 4.0 * scale;
    let radius = 6.0 * scale;
    quads.push(QuadInstance::rounded(
        [
            left + inset,
            y + scale,
            (width - 2.0 * inset).max(1.0),
            (row_height - 2.0 * scale).max(1.0),
        ],
        super::paint::faded(fill),
        radius,
    ));
    quads.push(QuadInstance::rounded(
        [
            left + inset,
            y + 5.0 * scale,
            4.0 * scale,
            (row_height - 10.0 * scale).max(1.0),
        ],
        super::paint::faded(accent),
        2.0 * scale,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selector_is_about_half_the_window_and_clears_the_tab_bar() {
        let box_ = selector_box(1000.0, 32.0, 1.0);
        assert!((box_.width - 460.0).abs() < 0.1);
        assert!(box_.width / 1000.0 >= 0.40 && box_.width / 1000.0 <= 0.50);
        assert!((box_.top - 40.0).abs() < 0.1);
        assert!((box_.input_height - 46.0).abs() < 0.1);
        assert!((box_.row_height - 36.0).abs() < 0.1);
        let wide = selector_box(2000.0, 64.0, 2.0);
        assert!((wide.width / 2000.0 - 0.46).abs() < 0.001);
        assert!((wide.top - 80.0).abs() < 0.1);
    }
}
