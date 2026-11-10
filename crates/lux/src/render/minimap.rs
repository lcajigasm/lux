//! Render pass for the minimap overview widget.

use crate::minimap::{MINIMAP_ROW_HEIGHT, MinimapState};
use crate::render::paint::color;
use lux_core::buffer::Buffer;
use lux_render::QuadInstance;
use lux_ui::minimap::build_frame;
use lux_ui::{Theme, Viewport};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_minimap(
    state: &MinimapState,
    buffer: &Buffer,
    viewport: &Viewport,
    window_width: f32,
    window_height: f32,
    tab_bar_height: f32,
    status_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    line_height: f32,
    theme: &Theme,
    folds: Option<&lux_core::folding::FoldState>,
    search_info: Option<(&[lux_core::selection::Selection], Option<usize>)>,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let minimap_w = state.effective_width(scale_factor);
    if minimap_w <= 0.0 {
        return;
    }

    let minimap_x = window_width - minimap_w;
    let minimap_y = tab_bar_height;
    let minimap_h = (window_height - tab_bar_height - status_bar_height).max(0.0);
    if minimap_h <= 0.0 {
        return;
    }

    let row_h = MINIMAP_ROW_HEIGHT * scale_factor;
    let available_rows = (minimap_h / row_h).floor() as usize;
    if available_rows == 0 {
        return;
    }

    let visible_start = (viewport.scroll_y / line_height).floor() as usize;
    let visible_end = ((viewport.scroll_y + viewport.height) / line_height).ceil() as usize;
    let frame = build_frame(
        &state.inner,
        buffer,
        visible_start..visible_end,
        available_rows,
        folds,
    );

    let Some(frame) = frame else {
        return;
    };

    // Minimap background quad (slightly darker/contrast)
    let bg = color(theme.background);
    let bg_color = [bg[0] * 0.95, bg[1] * 0.95, bg[2] * 0.95, bg[3]];
    quad_instances.push(QuadInstance::solid(
        [minimap_x, minimap_y, minimap_w, minimap_h],
        bg_color,
    ));

    // Density rows
    let char_w = 1.2 * scale_factor;
    let pad_left = 6.0 * scale_factor;
    let max_density_w = minimap_w - pad_left - 4.0 * scale_factor;
    let bar_color = density_bar(color(theme.foreground), color(theme.background));

    for (row_idx, row) in frame.rows.iter().enumerate() {
        if row.content_columns == 0 {
            continue;
        }
        let y = minimap_y + row_idx as f32 * row_h;
        let indent_x = (row.indent_columns as f32 * char_w).min(minimap_w * 0.4);
        let x = minimap_x + pad_left + indent_x;
        let w = (row.content_columns as f32 * char_w).clamp(2.0 * scale_factor, max_density_w);
        let h = (row_h - 1.0 * stroke).max(1.0);

        quad_instances.push(QuadInstance::solid([x, y, w, h], bar_color));
    }

    // Viewport indicator overlay
    if !frame.viewport.rows.is_empty() {
        let vy = minimap_y + frame.viewport.rows.start as f32 * row_h;
        let vh = (frame.viewport.rows.len() as f32 * row_h).max(row_h * 2.0);
        let clamped_vh = vh.min(minimap_h - (vy - minimap_y).max(0.0));

        let sel = color(theme.selection);
        let indicator_bg = [sel[0], sel[1], sel[2], 0.18];
        quad_instances.push(QuadInstance::solid(
            [minimap_x, vy, minimap_w, clamped_vh],
            indicator_bg,
        ));

        // Indicator top and bottom borders
        quad_instances.push(QuadInstance::solid(
            [minimap_x, vy, minimap_w, stroke],
            color(theme.selection),
        ));
        quad_instances.push(QuadInstance::solid(
            [
                minimap_x,
                (vy + clamped_vh - stroke).max(vy),
                minimap_w,
                stroke,
            ],
            color(theme.selection),
        ));
    }

    // Search match highlights on minimap (Sublime gold ticks)
    if let Some((matches, current_idx)) = search_info {
        let total_lines = buffer.len_lines();
        for (i, m) in matches.iter().enumerate() {
            let line = m.head.line;
            if let Some(row_idx) =
                lux_ui::minimap::row_for_buffer_line(total_lines, available_rows, line, folds)
                && row_idx < available_rows
            {
                let y = minimap_y + row_idx as f32 * row_h;
                let is_curr = current_idx == Some(i);
                let tick_color = if is_curr {
                    [1.0, 0.85, 0.20, 0.95]
                } else {
                    [0.85, 0.65, 0.15, 0.70]
                };
                let tick_h = (row_h).max(2.0 * stroke);
                quad_instances.push(QuadInstance::solid(
                    [minimap_x, y, minimap_w, tick_h],
                    tick_color,
                ));
            }
        }
    }
}

/// Density bars are a fraction of the foreground, drawn on a slightly
/// darker copy of the page. Solarized Dark's body color at 28% alpha
/// lands 37 levels from `#002b36`, under the capture check, so the
/// minimap reads as empty. Raise the alpha until the composited bar
/// clears the page, and leave a strong foreground at the soft alpha.
fn density_bar(foreground: [f32; 4], background: [f32; 4]) -> [f32; 4] {
    let mut ink = foreground;
    for _ in 0..8 {
        for step in 0..=12 {
            let alpha = 0.28 + step as f32 * 0.05;
            let bar = [ink[0], ink[1], ink[2], alpha];
            if composited_gap(bar, background) >= 56 {
                return bar;
            }
        }
        let sum = background[0] + background[1] + background[2];
        let target = if sum < 1.5 { 1.0 } else { 0.0 };
        ink = [
            ink[0] + (target - ink[0]) * 0.2,
            ink[1] + (target - ink[1]) * 0.2,
            ink[2] + (target - ink[2]) * 0.2,
            1.0,
        ];
    }
    [ink[0], ink[1], ink[2], 0.88]
}

/// Chebyshev gap, in 8-bit levels, of a bar composited over the minimap
/// well (`background * 0.95`) against the editor page. Straight alpha,
/// the same space as the `Rgba8Unorm` snapshot.
fn composited_gap(bar: [f32; 4], background: [f32; 4]) -> i32 {
    let alpha = bar[3].clamp(0.0, 1.0);
    let mut gap = 0;
    for channel in 0..3 {
        let under = background[channel] * 0.95;
        let mixed = bar[channel] * alpha + under * (1.0 - alpha);
        let left = (mixed.clamp(0.0, 1.0) * 255.0).round() as i32;
        let right = (background[channel].clamp(0.0, 1.0) * 255.0).round() as i32;
        gap = gap.max((left - right).abs());
    }
    gap
}

#[cfg(test)]
mod tests {
    use super::{composited_gap, density_bar};
    use crate::render::paint::color;

    fn rgba(hex: &str) -> [f32; 4] {
        color(lux_ui::Color::from_hex(hex).expect("hex"))
    }

    #[test]
    fn solarized_dark_bars_clear_the_editor() {
        let background = rgba("002b36");
        let bar = density_bar(rgba("839496"), background);
        assert!(composited_gap(bar, background) >= 56, "gap too small");
    }

    #[test]
    fn solarized_light_bars_clear_the_editor() {
        let background = rgba("fdf6e3");
        let bar = density_bar(rgba("657b83"), background);
        assert!(composited_gap(bar, background) >= 56);
    }

    #[test]
    fn monokai_keeps_the_soft_bar() {
        let bar = density_bar(rgba("f8f8f2"), rgba("272822"));
        assert!((bar[3] - 0.28).abs() < 0.001);
    }
}
