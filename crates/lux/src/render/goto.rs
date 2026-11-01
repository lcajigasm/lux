//! Rendering logic for the floating Goto Anything overlay (G09 / ST07).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::goto::{GotoItemKind, GotoState};
use crate::render::paint::{color, fuzzy_spans, push_glyph, span_covers};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

const MAX_VISIBLE_RESULTS: usize = 8;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_goto(
    goto: &mut GotoState,
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
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let width = (viewport_width * 0.58).clamp(380.0 * scale_factor, 660.0 * scale_factor);
    let left = (viewport_width - width) * 0.5;
    let top = tab_bar_height
        + 12.0 * scale_factor
        + goto.motion.open_offset(now, reduced, -8.0 * scale_factor);
    let input_height = 34.0 * scale_factor;
    let row_height = 26.0 * scale_factor;

    let visible_count = goto.items.len().min(MAX_VISIBLE_RESULTS);
    let total_height = input_height + (visible_count as f32 * row_height) + 4.0 * stroke;

    let modal_font_size = font_size * 0.82;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: modal_font_size,
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
    let input_baseline = (input_height - modal_font_size) * 0.5;
    let text_x = left + 12.0 * scale_factor;
    let text_y = top + stroke + input_baseline;

    let (display_text, text_color) = if goto.query.is_empty() {
        let placeholder = if goto.mode == crate::goto::GotoMode::ProjectSymbols {
            "Goto Symbol in Project..."
        } else {
            "Goto Anything (type filename, :line, @symbol, #term)..."
        };
        (placeholder, muted)
    } else {
        (goto.query.as_str(), foreground)
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
            modal_font_size,
            &cluster,
            text_x,
            text_y + input_line.baseline(),
            text_color,
            glyph_instances,
        );
    }

    if !goto.query.is_empty() {
        let caret_x = text_x + input_line.width() + 2.0 * stroke;
        quad_instances.push(QuadInstance::solid(
            [
                caret_x,
                top + stroke + (input_height - modal_font_size) * 0.5,
                2.0 * stroke,
                modal_font_size,
            ],
            color(theme.accent),
        ));
    }

    // 5. Result rows
    let rows_top = top + input_height + 2.0 * stroke;
    let row_baseline = (row_height - modal_font_size) * 0.5;

    let start_idx = if goto.selected_index >= visible_count {
        goto.selected_index + 1 - visible_count
    } else {
        0
    };
    let visual = goto.motion.row(goto.selected_index, now, reduced);
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

    for (i, item) in goto
        .items
        .iter()
        .skip(start_idx)
        .take(visible_count)
        .enumerate()
    {
        let row_y = rows_top + i as f32 * row_height;
        let actual_idx = start_idx + i;
        let is_selected = actual_idx == goto.selected_index;
        let icon = match &item.kind {
            GotoItemKind::File { .. } => super::icons::Icon::File,
            GotoItemKind::Symbol { .. } => super::icons::Icon::Symbol,
            GotoItemKind::Term { .. } => super::icons::Icon::Find,
        };
        let icon_size = 14.0 * scale_factor;
        let icon_x = left + 12.0 * scale_factor;
        let hole = if is_selected {
            color(theme.chrome.panel_row)
        } else {
            color(theme.chrome.panel)
        };
        super::icons::push_icon(
            icon,
            quad_instances,
            icon_x,
            row_y + (row_height - icon_size) * 0.5,
            icon_size,
            color(theme.accent),
            hole,
        );
        let mut offset_x = icon_x + icon_size + 8.0 * scale_factor;

        // Render badge for symbols
        if let GotoItemKind::Symbol { kind_badge, .. } = &item.kind {
            let badge_bg = match *kind_badge {
                "fn" | "method" => [0.20, 0.35, 0.55, 0.85],
                "struct" | "class" => [0.45, 0.30, 0.55, 0.85],
                "trait" | "interface" => [0.25, 0.45, 0.35, 0.85],
                _ => [0.30, 0.30, 0.35, 0.85],
            };
            let badge_layout = editor_view
                .text_layout_mut()
                .layout_line(&style, kind_badge, &[]);
            let badge_w = badge_layout.width() + 8.0 * scale_factor;
            let badge_h = 16.0 * scale_factor;
            let badge_y = row_y + (row_height - badge_h) * 0.5;
            quad_instances.push(QuadInstance::rounded(
                [offset_x, badge_y, badge_w, badge_h],
                badge_bg,
                3.0 * scale_factor,
            ));
            for cluster in badge_layout.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    modal_font_size * 0.85,
                    &cluster,
                    offset_x + 4.0 * scale_factor,
                    badge_y + (badge_h - modal_font_size * 0.85) * 0.5 + badge_layout.baseline(),
                    [1.0, 1.0, 1.0, 1.0],
                    glyph_instances,
                );
            }
            offset_x += badge_w + 6.0 * scale_factor;
        }

        // Render main label
        let label_line = editor_view
            .text_layout_mut()
            .layout_line(&style, &item.label, &[]);
        let label_color = if is_selected { foreground } else { muted };
        let hits = fuzzy_spans(&item.label, &goto.query);

        for cluster in label_line.clusters() {
            let glyph_color = if span_covers(&hits, &cluster.byte_range) {
                color(theme.accent)
            } else {
                label_color
            };
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                modal_font_size,
                &cluster,
                offset_x,
                row_y + row_baseline + label_line.baseline(),
                glyph_color,
                glyph_instances,
            );
        }

        // Detail badge on right edge (e.g. ":line 42")
        if !item.detail.is_empty() {
            let detail_line = editor_view
                .text_layout_mut()
                .layout_line(&style, &item.detail, &[]);
            let detail_x = left + width - detail_line.width() - 14.0 * scale_factor;
            for cluster in detail_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    modal_font_size * 0.88,
                    &cluster,
                    detail_x,
                    row_y + row_baseline + detail_line.baseline(),
                    color(theme.accent),
                    glyph_instances,
                );
            }
        }
    }
    crate::overlay_motion::note_if_animating(&goto.motion, now);
}
