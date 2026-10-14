//! Rendering logic for the floating Command Palette overlay (G08).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::palette::CommandPaletteState;
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::command_palette::Context;
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 8;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_command_palette(
    palette: &CommandPaletteState,
    viewport_width: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    has_selection: bool,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let width = (viewport_width * 0.55).clamp(360.0 * scale_factor, 620.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height + 12.0 * scale_factor;
    let input_height = 34.0 * scale_factor;
    let row_height = 26.0 * scale_factor;

    let mut context = Context::new();
    context.set("editorFocus", true);
    context.set("hasSelection", has_selection);
    let results = palette.results(&context);
    let visible_count = results.len().min(MAX_VISIBLE_RESULTS);
    let total_height = input_height + (visible_count as f32 * row_height) + 4.0 * stroke;

    let palette_font_size = font_size * 0.82;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: palette_font_size,
        ligatures: false,
    };

    let foreground = color(theme.foreground);
    let muted = color(theme.muted);

    // 1. Overlay main background
    quad_instances.push(QuadInstance {
        rect: [left, top, width, total_height],
        color: [0.09, 0.09, 0.12, 0.98],
    });

    // 2. Outer border
    let border_color = [0.25, 0.25, 0.32, 1.0];
    quad_instances.push(QuadInstance {
        rect: [left, top, width, stroke],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left, top + total_height - stroke, width, stroke],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left, top, stroke, total_height],
        color: border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [left + width - stroke, top, stroke, total_height],
        color: border_color,
    });

    // 3. Input background & separator
    quad_instances.push(QuadInstance {
        rect: [
            left + stroke,
            top + stroke,
            width - 2.0 * stroke,
            input_height,
        ],
        color: [0.065, 0.065, 0.085, 1.0],
    });
    quad_instances.push(QuadInstance {
        rect: [left, top + input_height + stroke, width, stroke],
        color: [0.18, 0.18, 0.22, 1.0],
    });

    // 4. Input text & caret
    let input_baseline = (input_height - palette_font_size) * 0.5;
    let text_x = left + 12.0 * scale_factor;
    let text_y = top + stroke + input_baseline;

    let (display_text, text_color) = if palette.query.is_empty() {
        ("> Type a command...", muted)
    } else {
        (palette.query.as_str(), foreground)
    };

    let input_line = editor_view
        .text_layout_mut()
        .layout_line(&style, display_text, &[]);
    for cluster in input_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            palette_font_size,
            &cluster,
            text_x,
            text_y + input_line.baseline(),
            text_color,
            glyph_instances,
        );
    }

    if !palette.query.is_empty() {
        let caret_x = text_x + input_line.width() + 2.0 * stroke;
        quad_instances.push(QuadInstance {
            rect: [
                caret_x,
                top + stroke + (input_height - palette_font_size) * 0.5,
                2.0 * stroke,
                palette_font_size,
            ],
            color: color(theme.accent),
        });
    }

    // 5. Result rows
    let rows_top = top + input_height + 2.0 * stroke;
    let row_baseline = (row_height - palette_font_size) * 0.5;

    for (i, entry) in results.iter().take(visible_count).enumerate() {
        let row_y = rows_top + i as f32 * row_height;
        let is_selected = i == palette.selected_index;

        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [left + stroke, row_y, width - 2.0 * stroke, row_height],
                color: [0.18, 0.22, 0.32, 1.0],
            });
            quad_instances.push(QuadInstance {
                rect: [left + stroke, row_y, 3.0 * stroke, row_height],
                color: color(theme.accent),
            });
        }

        // Title
        let title_line =
            editor_view
                .text_layout_mut()
                .layout_line(&style, &entry.command.title, &[]);
        let title_x = left + 14.0 * scale_factor;
        let title_color = if is_selected { foreground } else { muted };

        for cluster in title_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                palette_font_size,
                &cluster,
                title_x,
                row_y + row_baseline + title_line.baseline(),
                title_color,
                glyph_instances,
            );
        }

        // Category
        let cat_line =
            editor_view
                .text_layout_mut()
                .layout_line(&style, &entry.command.category, &[]);
        let cat_x = left + width - cat_line.width() - 14.0 * scale_factor;
        for cluster in cat_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                palette_font_size,
                &cluster,
                cat_x,
                row_y + row_baseline + cat_line.baseline(),
                muted,
                glyph_instances,
            );
        }
    }
}
