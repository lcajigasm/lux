//! The one card every picker paints: input, virtualized rows, scrollbar.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::overlay_list::ListFrame;
use crate::render::icons::Icon;
use crate::render::paint::{fuzzy_spans, overlay_text, push_bold_glyph, push_glyph, span_covers};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

pub(crate) struct OverlayItem<'a> {
    pub title: &'a str,
    pub badge: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub icon: Option<Icon>,
    pub swatches: Option<[[f32; 4]; 4]>,
}

pub(crate) struct OverlayColors {
    pub card: [f32; 4],
    pub selection: [f32; 4],
    pub thumb: [f32; 4],
    pub text_card: [f32; 4],
    pub text_selection: [f32; 4],
    pub match_card: [f32; 4],
    pub match_selection: [f32; 4],
    pub detail_card: [f32; 4],
    pub detail_selection: [f32; 4],
}

pub(crate) fn overlay_colors(theme: &lux_ui::Theme) -> OverlayColors {
    use super::paint::color;
    use super::panel::{
        card_fill, contrasting_fill, label_ink, readable_ink, selected_fill, shortcut_ink,
    };
    let card = card_fill(theme);
    let selection = contrasting_fill(selected_fill(theme), card);
    let accent = color(theme.accent);
    let match_on = |background| {
        if super::panel::contrast_ratio(accent, background) >= 4.8 {
            accent
        } else {
            label_ink(theme, background)
        }
    };
    OverlayColors {
        card,
        selection,
        thumb: contrasting_fill(color(theme.chrome.border), card),
        text_card: label_ink(theme, card),
        text_selection: label_ink(theme, selection),
        match_card: match_on(card),
        match_selection: match_on(selection),
        detail_card: readable_ink(shortcut_ink(theme), card),
        detail_selection: readable_ink(shortcut_ink(theme), selection),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_overlay(
    frame: &ListFrame,
    placeholder: &str,
    query: &str,
    items: &[OverlayItem<'_>],
    selected: usize,
    visual: f32,
    open_offset: f32,
    scale: f32,
    stroke: f32,
    font: FontRef<'_>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let _overlay = overlay_text();
    let colors = overlay_colors(theme);
    let top = frame.top + open_offset;
    super::panel::push_rounded_surface(
        quads,
        [frame.left, top, frame.width, frame.height],
        frame.radius,
        colors.card,
        super::paint::color(theme.chrome.border),
        20.0 * scale,
        stroke.max(1.5 * scale),
    );
    let font_size = 13.0 * scale;
    let (font, family) = crate::font::ui_font(font, editor_view.family());
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };
    let pad = 10.0 * scale;
    let field_h = (frame.input_height - 14.0 * scale).max(font_size + 8.0 * scale);
    super::panel::push_rounded_surface(
        quads,
        [
            frame.left + pad,
            top + 8.0 * scale,
            frame.width - 2.0 * pad,
            field_h,
        ],
        8.0 * scale,
        super::panel::field_fill(theme),
        super::paint::color(theme.chrome.border),
        0.0,
        stroke.max(1.0),
    );
    let (display, ink) = if query.is_empty() {
        (
            placeholder,
            super::panel::readable_ink(super::panel::placeholder_ink(theme), colors.card),
        )
    } else {
        (query, colors.text_card)
    };
    let input = editor_view
        .text_layout_mut()
        .layout_line(&style, display, &[]);
    let text_x = frame.left + 22.0 * scale;
    let input_y = top + 8.0 * scale + (field_h - font_size) * 0.5;
    for cluster in input.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            text_x,
            input_y + input.baseline(),
            ink,
            glyphs,
        );
    }
    if !query.is_empty() {
        quads.push(QuadInstance::solid(
            [
                text_x + input.width() + 2.0 * stroke,
                input_y,
                2.0 * stroke,
                font_size,
            ],
            colors.text_card,
        ));
    }

    let rows_top = top + frame.input_height + 2.0 * scale;
    if frame.visible > 0 && !items.is_empty() {
        super::panel::push_moving_row(
            quads,
            frame.left,
            rows_top,
            frame.width - 14.0 * scale,
            frame.row_height,
            visual,
            frame.origin,
            frame.visible.min(items.len().saturating_sub(frame.origin)),
            colors.selection,
            colors.selection,
            scale,
        );
    }
    paint_scrollbar(frame, items.len(), top, &colors, scale, quads);
    let row_baseline = (frame.row_height - font_size) * 0.5;
    for (offset, item) in items
        .iter()
        .skip(frame.origin)
        .take(frame.visible)
        .enumerate()
    {
        let index = frame.origin + offset;
        let selected_row = index == selected;
        let row_y = rows_top + offset as f32 * frame.row_height;
        let row_fill = if selected_row {
            colors.selection
        } else {
            colors.card
        };
        let text = if selected_row {
            colors.text_selection
        } else {
            colors.text_card
        };
        let hit = if selected_row {
            colors.match_selection
        } else {
            colors.match_card
        };
        let detail_ink = if selected_row {
            colors.detail_selection
        } else {
            colors.detail_card
        };
        let mut text_x = frame.left + 16.0 * scale;
        if let Some(icon) = item.icon {
            let icon_size = 16.0 * scale;
            super::icons::push_icon(
                icon,
                quads,
                text_x,
                row_y + (frame.row_height - icon_size) * 0.5,
                icon_size,
                text,
                row_fill,
            );
            text_x += icon_size + 10.0 * scale;
        }
        if let Some(badge) = item.badge {
            text_x = paint_badge(
                badge,
                text_x,
                row_y,
                frame.row_height,
                font_size,
                scale,
                &style,
                font,
                text,
                row_fill,
                editor_view,
                atlas,
                atlas_dirty,
                scale_cx,
                glyphs,
                quads,
            );
        }
        let detail_w = item.detail.map(|detail| {
            editor_view
                .text_layout_mut()
                .layout_line(&style, detail, &[])
                .width()
        });
        let swatch_w = item
            .swatches
            .map(|_| 4.0 * 12.0 * scale + 3.0 * 3.0 * scale);
        let right_w = detail_w.or(swatch_w).unwrap_or(0.0);
        let limit = frame.left + frame.width - right_w - 28.0 * scale;
        let title = editor_view
            .text_layout_mut()
            .layout_line(&style, item.title, &[]);
        let hits = fuzzy_spans(item.title, query);
        for cluster in title.clusters() {
            if text_x + cluster.x + cluster.advance > limit {
                continue;
            }
            let matched = span_covers(&hits, &cluster.byte_range);
            let draw = if matched && selected_row {
                push_bold_glyph
            } else {
                push_glyph
            };
            draw(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &cluster,
                text_x,
                row_y + row_baseline + title.baseline(),
                if matched { hit } else { text },
                glyphs,
            );
        }
        if let Some(colors4) = item.swatches {
            paint_swatches(colors4, frame, row_y, scale, quads);
        } else if let Some(detail) = item.detail {
            let line = editor_view
                .text_layout_mut()
                .layout_line(&style, detail, &[]);
            let x = frame.left + frame.width - line.width() - 18.0 * scale;
            for cluster in line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    font_size,
                    &cluster,
                    x,
                    row_y + row_baseline + line.baseline(),
                    detail_ink,
                    glyphs,
                );
            }
        }
    }
}

fn paint_scrollbar(
    frame: &ListFrame,
    total: usize,
    top: f32,
    colors: &OverlayColors,
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) {
    if total <= frame.visible || frame.visible == 0 {
        return;
    }
    let rows_top = top + frame.input_height + 2.0 * scale;
    let track_h = (frame.visible as f32 * frame.row_height - 8.0 * scale).max(1.0);
    let thumb_h = (track_h * frame.visible as f32 / total as f32).max(18.0 * scale);
    let travel = (track_h - thumb_h).max(0.0);
    let span = total.saturating_sub(frame.visible).max(1) as f32;
    let y = rows_top + 4.0 * scale + travel * (frame.origin as f32 / span);
    quads.push(QuadInstance::rounded(
        [
            frame.left + frame.width - 10.0 * scale,
            y,
            4.0 * scale,
            thumb_h,
        ],
        colors.thumb,
        2.0 * scale,
    ));
}

#[allow(clippy::too_many_arguments)]
fn paint_badge(
    badge: &str,
    x: f32,
    row_y: f32,
    row_height: f32,
    font_size: f32,
    scale: f32,
    style: &lux_render::LineStyle<'_>,
    font: FontRef<'_>,
    ink: [f32; 4],
    fill: [f32; 4],
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    glyphs: &mut Vec<GlyphInstance>,
    quads: &mut Vec<QuadInstance>,
) -> f32 {
    let line = editor_view.text_layout_mut().layout_line(style, badge, &[]);
    let width = line.width() + 10.0 * scale;
    let height = font_size + 6.0 * scale;
    let y = row_y + (row_height - height) * 0.5;
    quads.push(QuadInstance::rounded(
        [x, y, width, height],
        super::panel::contrasting_fill(fill, ink),
        3.0 * scale,
    ));
    for cluster in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            x + 5.0 * scale,
            y + (height - font_size) * 0.5 + line.baseline(),
            ink,
            glyphs,
        );
    }
    x + width + 8.0 * scale
}

fn paint_swatches(
    colors: [[f32; 4]; 4],
    frame: &ListFrame,
    row_y: f32,
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) {
    let size = 12.0 * scale;
    let gap = 3.0 * scale;
    let total = 4.0 * size + 3.0 * gap;
    let mut x = frame.left + frame.width - total - 18.0 * scale;
    let y = row_y + (frame.row_height - size) * 0.5;
    for color in colors {
        quads.push(QuadInstance::rounded(
            [x, y, size, size],
            color,
            2.0 * scale,
        ));
        x += size + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_bundled_themes_paint_overlay_at_wcag_aa() {
        use super::super::panel::contrast_ratio;
        let schemes = crate::color_scheme::bundled_schemes();
        assert!(schemes.len() >= 10);
        for scheme in schemes.iter().take(10) {
            let colors = overlay_colors(&scheme.theme);
            let pairs = [
                (
                    contrast_ratio(colors.selection, colors.card),
                    3.0,
                    "selection/card",
                ),
                (contrast_ratio(colors.thumb, colors.card), 3.0, "thumb/card"),
                (
                    contrast_ratio(colors.text_card, colors.card),
                    4.5,
                    "text/card",
                ),
                (
                    contrast_ratio(colors.text_selection, colors.selection),
                    4.5,
                    "text/selection",
                ),
                (
                    contrast_ratio(colors.match_card, colors.card),
                    4.5,
                    "match/card",
                ),
                (
                    contrast_ratio(colors.match_selection, colors.selection),
                    4.5,
                    "match/selection",
                ),
                (
                    contrast_ratio(colors.detail_card, colors.card),
                    4.5,
                    "detail/card",
                ),
                (
                    contrast_ratio(colors.detail_selection, colors.selection),
                    4.5,
                    "detail/selection",
                ),
            ];
            println!(
                "{} {}",
                scheme.name,
                pairs
                    .iter()
                    .map(|(ratio, _, name)| format!("{name} {ratio:.2}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            for (ratio, floor, name) in pairs {
                assert!(
                    ratio >= floor,
                    "{} {name} {ratio:.2} < {floor}",
                    scheme.name
                );
            }
        }
    }
}
