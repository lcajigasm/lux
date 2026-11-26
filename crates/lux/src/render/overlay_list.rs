//! The one card every picker paints: input, virtualized rows, scrollbar.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::overlay_list::{ListFrame, TEXT_INSET_PT, TITLE_PT};
use crate::render::icons::Icon;
use crate::render::paint::{overlay_text, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

mod colors;
mod row;

pub(crate) use colors::{OverlayColors, overlay_colors};

#[derive(Default)]
pub(crate) struct OverlayItem<'a> {
    pub title: &'a str,
    /// Byte ranges of `title` to draw as the match. They come from the
    /// matcher that ranked the row, never from re-scanning the title.
    pub hits: crate::overlay_list::Hits,
    /// A second, smaller line under the title (goto's file path). Only
    /// painted when the frame has two-line rows.
    pub subtitle: Option<&'a str>,
    /// Byte ranges of `subtitle` the query matched.
    pub subtitle_hits: crate::overlay_list::Hits,
    pub badge: Option<&'a str>,
    pub detail: Option<&'a str>,
    /// Only for icons that carry meaning; painted small and muted.
    pub icon: Option<Icon>,
    pub swatches: Option<[[f32; 4]; 4]>,
    /// The current choice, marked with a check.
    pub checked: bool,
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
    let font_size = TITLE_PT * scale;
    let (font, family) = crate::font::ui_font(font, editor_view.family());
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
    let mut pen = row::Pen {
        font,
        family: &family,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        glyphs,
    };
    let (display, ink) = if query.is_empty() {
        (
            placeholder,
            super::panel::readable_ink(super::panel::placeholder_ink(theme), colors.card),
        )
    } else {
        (query, colors.text_card)
    };
    // The query starts where every row's text starts.
    let text_x = frame.left + TEXT_INSET_PT * scale;
    let input_y = top + 8.0 * scale + (field_h - font_size) * 0.5;
    let plain = row::Ink {
        text: ink,
        hit: ink,
        hit_bold: false,
    };
    let room = frame.width - 2.0 * TEXT_INSET_PT * scale;
    pen.line(
        display,
        &[],
        font_size,
        text_x,
        input_y,
        room,
        row::Elide::Start,
        plain,
    );
    if !query.is_empty() {
        let caret_x = text_x + pen.width(query, font_size).min(room);
        quads.push(QuadInstance::solid(
            [caret_x + 2.0 * stroke, input_y, 2.0 * stroke, font_size],
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
    for (offset, item) in items
        .iter()
        .skip(frame.origin)
        .take(frame.visible)
        .enumerate()
    {
        let slots = frame.row_slots(rows_top, offset, scale);
        let selected_row = frame.origin + offset == selected;
        row::paint_row(
            &mut pen,
            item,
            frame,
            slots,
            selected_row,
            &colors,
            scale,
            quads,
        );
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

/// A short status at the right end of the input field, such as
/// "Indexing…" while the project symbol index is still being built.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_input_notice(
    frame: &ListFrame,
    notice: &str,
    open_offset: f32,
    scale: f32,
    font: FontRef<'_>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let colors = overlay_colors(theme);
    let top = frame.top + open_offset;
    let font_size = 13.0 * scale;
    let (font, family) = crate::font::ui_font(font, editor_view.family());
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };
    let field_h = (frame.input_height - 14.0 * scale).max(font_size + 8.0 * scale);
    let line = editor_view
        .text_layout_mut()
        .layout_line(&style, notice, &[]);
    let x = frame.left + frame.width - 22.0 * scale - line.width();
    let y = top + 8.0 * scale + (field_h - font_size) * 0.5;
    let ink = super::panel::readable_ink(super::panel::placeholder_ink(theme), colors.card);
    for cluster in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            x,
            y + line.baseline(),
            ink,
            glyphs,
        );
    }
}
