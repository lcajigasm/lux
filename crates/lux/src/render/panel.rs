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

pub(crate) fn selected_fill() -> [f32; 4] {
    color(lux_ui::Color::rgb(0x75, 0x71, 0x5E))
}

pub(crate) fn rim_color() -> [f32; 4] {
    color(lux_ui::Color::rgb(0xC2, 0xC0, 0xA8))
}

pub(crate) fn placeholder_ink(theme: &lux_ui::Theme) -> [f32; 4] {
    let ink = color(theme.foreground);
    [ink[0] * 0.78, ink[1] * 0.78, ink[2] * 0.72, 1.0]
}

/// Accent pink fails on the beige selection. Hits there use the
/// foreground; unselected hits stay on the accent.
pub(crate) fn hit_ink(on_selection: bool, theme: &lux_ui::Theme) -> [f32; 4] {
    if on_selection {
        color(theme.foreground)
    } else {
        color(theme.accent)
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
            [0.0, 0.0, 0.0, 0.78],
            radius + grow * 0.2,
        ));
    }
    quads.push(QuadInstance::rounded(rect, border, radius));
    let inset = stroke.max(1.0);
    quads.push(QuadInstance::rounded(
        [
            rect[0] + inset,
            rect[1] + inset,
            (rect[2] - 2.0 * inset).max(1.0),
            (rect[3] - 2.0 * inset).max(1.0),
        ],
        fill,
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
        fill,
        radius,
    ));
    quads.push(QuadInstance::rounded(
        [
            left + inset,
            y + 5.0 * scale,
            4.0 * scale,
            (row_height - 10.0 * scale).max(1.0),
        ],
        accent,
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
