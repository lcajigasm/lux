//! One row of the shared card: title, optional path line, badge, and the
//! right-aligned hint or swatches. Text that does not fit is elided with
//! "…": a title keeps its start, a path keeps its end.

use swash::FontRef;
use swash::scale::ScaleContext;

use super::OverlayItem;
use super::colors::OverlayColors;
use crate::overlay_list::{DETAIL_RIGHT_PT, ListFrame, RowSlots, SUBTITLE_PT, TITLE_PT};
use crate::render::paint::{push_bold_glyph, push_glyph, span_covers};
use lux_render::{ClusterGeometry, GlyphAtlas, GlyphInstance, LineStyle, QuadInstance};
use lux_ui::editor_view::EditorView;

/// Everything a line of text needs to reach the glyph batch.
pub(super) struct Pen<'a, 'f> {
    pub font: FontRef<'f>,
    pub family: &'a str,
    pub editor_view: &'a mut EditorView,
    pub atlas: &'a mut GlyphAtlas,
    pub atlas_dirty: &'a mut bool,
    pub scale_cx: &'a mut ScaleContext,
    pub glyphs: &'a mut Vec<GlyphInstance>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Elide {
    /// Keep the start: a name or a title.
    End,
    /// Keep the end: a path, whose file and parent matter most.
    Start,
}

/// Colors for one line.
#[derive(Clone, Copy)]
pub(super) struct Ink {
    pub text: [f32; 4],
    pub hit: [f32; 4],
    pub hit_bold: bool,
}

/// Leading clusters that fit before a trailing ellipsis, or `None` when
/// the whole line fits. Extents are each cluster's `(x, advance)`.
pub(super) fn keep_head(
    extents: &[(f32, f32)],
    width: f32,
    room: f32,
    ellipsis: f32,
) -> Option<usize> {
    if width <= room {
        return None;
    }
    let room = room - ellipsis;
    Some(
        extents
            .iter()
            .take_while(|(x, advance)| x + advance <= room)
            .count(),
    )
}

/// First cluster kept after a leading ellipsis, or `None` when the whole
/// line fits.
pub(super) fn keep_tail(
    extents: &[(f32, f32)],
    width: f32,
    room: f32,
    ellipsis: f32,
) -> Option<usize> {
    if width <= room {
        return None;
    }
    let room = room - ellipsis;
    Some(
        extents
            .iter()
            .position(|(x, _)| width - x <= room)
            .unwrap_or(extents.len()),
    )
}

impl Pen<'_, '_> {
    fn layout(&mut self, text: &str, size: f32) -> lux_render::LineLayout {
        let style = LineStyle {
            family: self.family,
            font_size: size,
            ligatures: false,
        };
        self.editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[])
    }

    pub(super) fn width(&mut self, text: &str, size: f32) -> f32 {
        self.layout(text, size).width()
    }

    fn glyph(
        &mut self,
        size: f32,
        cluster: &ClusterGeometry,
        x: f32,
        y: f32,
        ink: [f32; 4],
        bold: bool,
    ) {
        let draw = if bold { push_bold_glyph } else { push_glyph };
        draw(
            self.atlas,
            self.atlas_dirty,
            self.scale_cx,
            self.font,
            size,
            cluster,
            x,
            y,
            ink,
            self.glyphs,
        );
    }

    /// Draws `text` with its top at `top`, inside `room` from `x`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn line(
        &mut self,
        text: &str,
        hits: &[std::ops::Range<usize>],
        size: f32,
        x: f32,
        top: f32,
        room: f32,
        elide: Elide,
        ink: Ink,
    ) {
        let line = self.layout(text, size);
        let clusters = line.clusters();
        let baseline = top + line.baseline();
        let extents: Vec<(f32, f32)> = clusters.iter().map(|c| (c.x, c.advance)).collect();
        // The fallback face may lack U+2026; the glyph is drawn from
        // `font`, so only use it when that face has it.
        let mark = if self.font.charmap().map('\u{2026}') != 0 {
            "\u{2026}"
        } else {
            "..."
        };
        let dots = self.layout(mark, size);
        let dots_w = dots.width();
        let (range, shift, dots_x) = match elide {
            Elide::End => match keep_head(&extents, line.width(), room, dots_w) {
                None => (0..clusters.len(), 0.0, None),
                Some(n) => {
                    let end = n
                        .checked_sub(1)
                        .map_or(0.0, |i| extents[i].0 + extents[i].1);
                    (0..n, 0.0, Some(x + end))
                }
            },
            Elide::Start => match keep_tail(&extents, line.width(), room, dots_w) {
                None => (0..clusters.len(), 0.0, None),
                Some(i) => {
                    let from = extents.get(i).map_or(line.width(), |e| e.0);
                    (i..clusters.len(), dots_w - from, Some(x))
                }
            },
        };
        for cluster in &clusters[range] {
            let matched = span_covers(hits, &cluster.byte_range);
            let (color, bold) = if matched {
                (ink.hit, ink.hit_bold)
            } else {
                (ink.text, false)
            };
            self.glyph(size, cluster, x + shift, baseline, color, bold);
        }
        if let Some(dots_x) = dots_x {
            for cluster in dots.clusters() {
                self.glyph(
                    size,
                    &cluster,
                    dots_x,
                    top + dots.baseline(),
                    ink.text,
                    false,
                );
            }
        }
    }
}

/// Paints one row's text and right-hand slot.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_row(
    pen: &mut Pen<'_, '_>,
    item: &OverlayItem<'_>,
    frame: &ListFrame,
    slots: RowSlots,
    selected: bool,
    colors: &OverlayColors,
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) {
    let (fill, text, hit, detail) = if selected {
        (
            colors.selection,
            colors.text_selection,
            colors.match_selection,
            colors.detail_selection,
        )
    } else {
        (
            colors.card,
            colors.text_card,
            colors.match_card,
            colors.detail_card,
        )
    };
    let title_ink = Ink {
        text,
        hit,
        hit_bold: selected || colors.match_card_bold,
    };
    let size = TITLE_PT * scale;
    let right = frame.left + frame.width - DETAIL_RIGHT_PT * scale;
    let center_top = slots.top + (frame.row_height - size) * 0.5;

    // Right-hand slot first: it sets how far the title may run.
    let mut limit = right;
    if let Some(swatches) = item.swatches {
        limit = paint_swatches(
            swatches,
            right,
            slots.top,
            frame.row_height,
            colors,
            scale,
            quads,
        );
    } else if let Some(hint) = item.detail {
        let width = pen.width(hint, size);
        let hint_ink = Ink {
            text: detail,
            hit: detail,
            hit_bold: false,
        };
        pen.line(
            hint,
            &[],
            size,
            right - width,
            center_top,
            width + 1.0,
            Elide::End,
            hint_ink,
        );
        limit = right - width;
    }
    if item.checked {
        let mark = 12.0 * scale;
        limit -= mark + 8.0 * scale;
        super::super::icons::push_icon(
            super::super::icons::Icon::Check,
            quads,
            limit + 4.0 * scale,
            slots.top + (frame.row_height - mark) * 0.5,
            mark,
            hit,
            fill,
        );
    }
    limit -= 12.0 * scale;

    let mut x = slots.text_left;
    if let Some(icon) = item.icon {
        // Only where the icon carries meaning (a completion's kind):
        // small and muted.
        let icon_size = 12.0 * scale;
        super::super::icons::push_icon(
            icon,
            quads,
            x,
            slots.top + (frame.row_height - icon_size) * 0.5,
            icon_size,
            detail,
            fill,
        );
        x += icon_size + 8.0 * scale;
    }
    if let Some(badge) = item.badge {
        x = paint_badge(
            pen,
            badge,
            x,
            slots.title_top,
            size,
            scale,
            text,
            fill,
            quads,
        );
    }
    pen.line(
        item.title,
        &item.hits,
        size,
        x,
        slots.title_top,
        (limit - x).max(0.0),
        Elide::End,
        title_ink,
    );
    if let (Some(path), Some(top)) = (item.subtitle, slots.subtitle_top) {
        let path_ink = Ink {
            text: detail,
            hit,
            hit_bold: false,
        };
        let left = slots.text_left;
        pen.line(
            path,
            &item.subtitle_hits,
            SUBTITLE_PT * scale,
            left,
            top,
            (limit - left).max(0.0),
            Elide::Start,
            path_ink,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_badge(
    pen: &mut Pen<'_, '_>,
    badge: &str,
    x: f32,
    text_top: f32,
    size: f32,
    scale: f32,
    ink: [f32; 4],
    fill: [f32; 4],
    quads: &mut Vec<QuadInstance>,
) -> f32 {
    let width = pen.width(badge, size) + 10.0 * scale;
    let height = size + 6.0 * scale;
    let y = text_top - 3.0 * scale;
    quads.push(QuadInstance::rounded(
        [x, y, width, height],
        crate::render::panel::contrasting_fill(fill, ink),
        3.0 * scale,
    ));
    let plain = Ink {
        text: ink,
        hit: ink,
        hit_bold: false,
    };
    pen.line(
        badge,
        &[],
        size,
        x + 5.0 * scale,
        text_top,
        width,
        Elide::End,
        plain,
    );
    x + width + 8.0 * scale
}

/// Four theme chips ending at `right`, each with a 1 px rim so a dark
/// chip does not vanish into a dark card. Returns the slot's left edge.
fn paint_swatches(
    swatches: [[f32; 4]; 4],
    right: f32,
    row_top: f32,
    row_height: f32,
    colors: &OverlayColors,
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) -> f32 {
    let size = 12.0 * scale;
    let gap = 4.0 * scale;
    let rim = 1.0;
    let left = right - (4.0 * size + 3.0 * gap);
    let y = row_top + (row_height - size) * 0.5;
    let mut x = left;
    for fill in swatches {
        quads.push(QuadInstance::rounded(
            [x - rim, y - rim, size + 2.0 * rim, size + 2.0 * rim],
            colors.chip_border,
            2.0 * scale + rim,
        ));
        quads.push(QuadInstance::rounded([x, y, size, size], fill, 2.0 * scale));
        x += size + gap;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    fn even(count: usize, advance: f32) -> (Vec<(f32, f32)>, f32) {
        let extents = (0..count).map(|i| (i as f32 * advance, advance)).collect();
        (extents, count as f32 * advance)
    }

    #[test]
    fn a_line_that_fits_is_not_elided() {
        let (extents, width) = even(10, 7.0);
        assert_eq!(keep_head(&extents, width, width, 8.0), None);
        assert_eq!(keep_tail(&extents, width, width + 5.0, 8.0), None);
    }

    #[test]
    fn a_long_title_keeps_its_start_and_ends_in_an_ellipsis() {
        let (extents, width) = even(20, 10.0);
        // 100 of room, 10 of it for "…": nine clusters fit.
        let kept = keep_head(&extents, width, 100.0, 10.0).unwrap();
        assert_eq!(kept, 9);
        assert!(extents[kept - 1].0 + extents[kept - 1].1 + 10.0 <= 100.0);
    }

    #[test]
    fn a_long_path_keeps_its_end_after_a_leading_ellipsis() {
        let path = "/private/tmp/claude-501/worktrees/lux/fixture/src/status_bar.rs";
        let (extents, width) = even(path.len(), 6.0);
        let from = keep_tail(&extents, width, 120.0, 6.0).unwrap();
        let tail = &path[from..];
        assert!(tail.ends_with("status_bar.rs"), "{tail}");
        assert!((width - extents[from].0) + 6.0 <= 120.0);
        // One cluster more would not fit.
        assert!((width - extents[from - 1].0) + 6.0 > 120.0);
    }

    #[test]
    fn no_room_elides_everything_without_panicking() {
        let (extents, width) = even(5, 10.0);
        assert_eq!(keep_head(&extents, width, 0.0, 10.0), Some(0));
        assert_eq!(keep_tail(&extents, width, 0.0, 10.0), Some(5));
        assert_eq!(keep_head(&[], 1.0, 0.0, 10.0), Some(0));
    }
}
