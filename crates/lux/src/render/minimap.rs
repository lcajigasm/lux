//! Render pass for the minimap overview widget.

use crate::minimap::{MINIMAP_ROW_HEIGHT, MinimapState};
use crate::render::paint::{color, faint_rule, syntax_color};
use lux_core::buffer::Buffer;
use lux_render::QuadInstance;
use lux_syntax::theme::SyntaxTheme;
use lux_ui::minimap::{MinimapInk, build_frame};
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
    syntax_theme: &SyntaxTheme,
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

    // Left border divider stroke
    quad_instances.push(QuadInstance::solid(
        [minimap_x, minimap_y, stroke, minimap_h],
        faint_rule(theme),
    ));

    // Column sketch. One slab per word, string, or comment, in the syntax
    // theme's own colors, so the strip reads as the file instead of a
    // single gray bar per line.
    let pad_left = 4.0 * scale_factor;
    let col_w = ((minimap_w - pad_left - 2.0 * scale_factor) / 96.0).max(scale_factor);
    let run_h = (row_h - 0.5 * stroke).max(1.0);
    let map_right = minimap_x + minimap_w;

    for (row_idx, row) in frame.rows.iter().enumerate() {
        let y = minimap_y + row_idx as f32 * row_h;
        for run in &row.runs {
            let x = minimap_x + pad_left + run.column as f32 * col_w;
            if x >= map_right {
                break;
            }
            let w = (run.width as f32 * col_w).max(scale_factor);
            let w = w.min(map_right - x);
            quad_instances.push(QuadInstance::solid(
                [x, y, w, run_h],
                minimap_ink_color(syntax_theme, run.ink),
            ));
        }
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

/// Opaque enough to read on the editor well. The old map used the
/// foreground at 0.28 alpha, which is the muted strip the phase-1 row
/// called unreadable.
pub(crate) fn minimap_ink_color(theme: &SyntaxTheme, ink: MinimapInk) -> [f32; 4] {
    let scope = match ink {
        MinimapInk::Text => "text",
        MinimapInk::Keyword => "keyword",
        MinimapInk::Punct => "punctuation",
        MinimapInk::String => "string",
        MinimapInk::Comment => "comment",
    };
    let mut color = syntax_color(theme.color_for_scope(scope));
    color[3] = 1.0;
    color
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_syntax::theme::MONOKAI;

    #[test]
    fn minimap_inks_use_the_syntax_theme_and_stay_opaque() {
        let text = minimap_ink_color(&MONOKAI, MinimapInk::Text);
        let comment = minimap_ink_color(&MONOKAI, MinimapInk::Comment);
        let string = minimap_ink_color(&MONOKAI, MinimapInk::String);
        let keyword = minimap_ink_color(&MONOKAI, MinimapInk::Keyword);
        assert!(text[3] > 0.9 && comment[3] > 0.9);
        assert_ne!(text, comment);
        assert_ne!(text, string);
        assert_ne!(text, keyword);
        assert!((keyword[0] - 249.0 / 255.0).abs() < 0.01);
        assert!((keyword[1] - 38.0 / 255.0).abs() < 0.01);
    }
}
