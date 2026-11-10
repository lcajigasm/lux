//! Rendering logic for the floating Goto Anything overlay (G09 / ST07).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::goto::{GotoItemKind, GotoState};
use crate::render::paint::{
    color, faded, fuzzy_spans, overlay_fade, overlay_text, push_bold_glyph, push_glyph, span_covers,
};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

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
    let _overlay = overlay_text();
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let _fade = overlay_fade(goto.motion.open_fade(now, reduced));
    let visible_count = goto.items.len().min(super::panel::SELECTOR_VISIBLE);
    let open_offset = goto.motion.open_offset(now, reduced, -8.0 * scale_factor);
    let chrome = super::panel::paint_selector_chrome(
        quad_instances,
        viewport_width,
        tab_bar_height,
        scale_factor,
        stroke,
        visible_count,
        open_offset,
        theme,
    );
    let modal_font_size = font_size * 0.92;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: modal_font_size,
        ligatures: false,
    };
    let foreground = color(theme.foreground);

    let input_baseline = (chrome.input_height - modal_font_size) * 0.5;
    let text_x = chrome.left + 22.0 * scale_factor;
    let text_y = chrome.top + stroke + input_baseline;
    let (display_text, text_color) = if goto.query.is_empty() {
        let placeholder = if goto.mode == crate::goto::GotoMode::ProjectSymbols {
            "Go to symbol…"
        } else {
            "Go to file…"
        };
        (placeholder, super::panel::placeholder_ink(theme))
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
                chrome.top + stroke + (chrome.input_height - modal_font_size) * 0.5,
                2.0 * stroke,
                modal_font_size,
            ],
            faded(color(theme.accent)),
        ));
    }

    let rows_top = chrome.rows_top;
    let row_height = chrome.row_height;
    let row_baseline = (row_height - modal_font_size) * 0.5;
    let start_idx = if goto.selected_index >= visible_count {
        goto.selected_index + 1 - visible_count
    } else {
        0
    };
    let visual = goto.motion.row(goto.selected_index, now, reduced);
    super::panel::push_moving_row(
        quad_instances,
        chrome.left,
        rows_top,
        chrome.width,
        row_height,
        visual,
        start_idx,
        visible_count,
        super::panel::selected_fill(theme),
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
        let icon_size = 16.0 * scale_factor;
        let icon_x = chrome.left + 16.0 * scale_factor;
        let hole = if is_selected {
            super::panel::selected_fill(theme)
        } else {
            super::panel::card_fill(theme)
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
                faded(badge_bg),
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
        let label_color = foreground;
        let hits = fuzzy_spans(&item.label, &goto.query);

        for cluster in label_line.clusters() {
            let hit = span_covers(&hits, &cluster.byte_range);
            let glyph_color = if hit {
                super::panel::hit_ink(is_selected, theme)
            } else {
                label_color
            };
            let draw = if hit && is_selected {
                push_bold_glyph
            } else {
                push_glyph
            };
            draw(
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
            let detail_x = chrome.left + chrome.width - detail_line.width() - 18.0 * scale_factor;
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
                    super::panel::shortcut_ink(theme),
                    glyph_instances,
                );
            }
        }
    }
    crate::overlay_motion::note_if_animating(&goto.motion, now);
}
