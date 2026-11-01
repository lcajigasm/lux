//! Rendering logic for the floating Command Palette overlay (G08).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::palette::CommandPaletteState;
use crate::render::paint::{color, fuzzy_spans, push_glyph, span_covers};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::command_palette::Context;
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 8;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_command_palette(
    palette: &mut CommandPaletteState,
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
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let width = (viewport_width * 0.55).clamp(360.0 * scale_factor, 620.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height
        + 12.0 * scale_factor
        + palette
            .motion
            .open_offset(now, reduced, -8.0 * scale_factor);
    let input_height = 34.0 * scale_factor;
    let row_height = 26.0 * scale_factor;

    let mut context = Context::new();
    context.set("editorFocus", true);
    context.set("hasSelection", has_selection);
    let visual = palette.motion.row(palette.selected_index, now, reduced);
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

    let radius = 8.0 * scale_factor;
    super::panel::push_rounded_surface(
        quad_instances,
        [left, top, width, total_height],
        radius,
        color(theme.chrome.panel),
        color(theme.chrome.border),
        6.0 * scale_factor,
        stroke,
    );
    let well_inset = radius * 0.75;
    quad_instances.push(QuadInstance::rounded(
        [
            left + well_inset,
            top + well_inset * 0.55,
            width - 2.0 * well_inset,
            input_height - well_inset * 0.45,
        ],
        color(theme.chrome.panel_input),
        5.0 * scale_factor,
    ));

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
        quad_instances.push(QuadInstance::solid(
            [
                caret_x,
                top + stroke + (input_height - palette_font_size) * 0.5,
                2.0 * stroke,
                palette_font_size,
            ],
            color(theme.accent),
        ));
    }

    // 5. Result rows
    let rows_top = top + input_height + 2.0 * stroke;
    let row_baseline = (row_height - palette_font_size) * 0.5;
    let start_idx = if palette.selected_index >= visible_count {
        palette.selected_index + 1 - visible_count
    } else {
        0
    };
    super::panel::push_moving_row(
        quad_instances,
        left,
        rows_top,
        width,
        row_height,
        visual,
        start_idx,
        visible_count,
        color(theme.chrome.panel_row),
        color(theme.accent),
        scale_factor,
    );

    for (i, entry) in results
        .iter()
        .skip(start_idx)
        .take(visible_count)
        .enumerate()
    {
        let row_y = rows_top + i as f32 * row_height;
        let actual_idx = start_idx + i;
        let is_selected = actual_idx == palette.selected_index;

        let icon_size = 14.0 * scale_factor;
        let icon_x = left + 12.0 * scale_factor;
        let icon_y = row_y + (row_height - icon_size) * 0.5;
        let hole = if is_selected {
            color(theme.chrome.panel_row)
        } else {
            color(theme.chrome.panel)
        };
        super::icons::push_icon(
            super::icons::icon_for_category(&entry.command.category),
            quad_instances,
            icon_x,
            icon_y,
            icon_size,
            color(theme.accent),
            hole,
        );

        // Title
        let title_line =
            editor_view
                .text_layout_mut()
                .layout_line(&style, &entry.command.title, &[]);
        let title_x = icon_x + icon_size + 8.0 * scale_factor;
        let title_color = if is_selected { foreground } else { muted };
        let hits = fuzzy_spans(&entry.command.title, &palette.query);

        for cluster in title_line.clusters() {
            let glyph_color = if span_covers(&hits, &cluster.byte_range) {
                color(theme.accent)
            } else {
                title_color
            };
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                palette_font_size,
                &cluster,
                title_x,
                row_y + row_baseline + title_line.baseline(),
                glyph_color,
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
    crate::overlay_motion::note_if_animating(&palette.motion, now);
}
