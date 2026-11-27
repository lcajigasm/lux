//! Render pass for the minimap overview widget: syntax-colored word
//! strokes, the viewport box, and git, bookmark and search marks.

use crate::app::SyntaxState;
use crate::git::colors::{GutterKind, gutter_color};
use crate::minimap::SyntaxColors;
use crate::minimap::{MINIMAP_ROW_HEIGHT, MinimapState, drag};
use crate::render::paint::color;
use crate::search::SearchBarState;
use crate::ui::UiOverlays;
use lux_core::bookmarks::BookmarkState;
use lux_core::buffer::Buffer;
use lux_core::folding::FoldState;
use lux_git::gutter::{GutterMarker, HunkKind};
use lux_render::QuadInstance;
use lux_ui::minimap::{LineColors, MinimapFrame, row_for_buffer_line};
use lux_ui::{Theme, Viewport};

/// The document the minimap shows.
pub(crate) struct MinimapDoc<'a> {
    pub(crate) id: u64,
    pub(crate) buffer: &'a Buffer,
    pub(crate) colors: Option<&'a dyn LineColors>,
    pub(crate) bookmarks: &'a BookmarkState,
    pub(crate) folds: &'a FoldState,
}

/// Marks that belong to the focused file only: its git hunks and its
/// Cmd+F matches.
pub(crate) struct Marks<'a> {
    pub(crate) git: &'a [GutterMarker],
    pub(crate) search: Option<&'a SearchBarState>,
}

/// Where one minimap goes and which cache it reads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placement {
    /// `[x, y, width, height]` in physical pixels.
    pub(crate) rect: [f32; 4],
    /// Split group index; the single layout is group 0.
    pub(crate) group: usize,
    pub(crate) scale_factor: f32,
    pub(crate) line_height: f32,
}

/// Where the strip sits and how big one row is, in physical pixels.
struct Strip {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    row_h: f32,
    rows: usize,
    stroke: f32,
}

impl Strip {
    fn row_y(&self, row: usize) -> f32 {
        self.y + row as f32 * self.row_h
    }
}

/// Bookmark marks take the gutter's bookmark cyan.
const BOOKMARK: [f32; 4] = [0.25, 0.70, 0.95, 0.95];

impl Placement {
    /// The single layout's minimap: the editor column's right edge, from
    /// the tab row to the editor's bottom, above the find bar and the dock.
    pub(crate) fn single(
        minimap_w: f32,
        window_w: f32,
        (tab_bar_height, editor_bottom): (f32, f32),
        scale_factor: f32,
        line_height: f32,
    ) -> Self {
        let h = (editor_bottom - tab_bar_height).max(0.0);
        Self {
            rect: [window_w - minimap_w, tab_bar_height, minimap_w, h],
            group: 0,
            scale_factor,
            line_height,
        }
    }
}

pub(crate) fn render_minimap(
    minimap: &mut MinimapState,
    doc: &MinimapDoc<'_>,
    marks: Option<&Marks<'_>>,
    viewport: &Viewport,
    at: Placement,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let [x, y, minimap_w, minimap_h] = at.rect;
    let row_h = MINIMAP_ROW_HEIGHT * at.scale_factor;
    let available_rows = (minimap_h / row_h).floor() as usize;
    if minimap_w <= 0.0 || available_rows == 0 {
        return;
    }
    let strip = Strip {
        x,
        y,
        w: minimap_w,
        h: minimap_h,
        row_h,
        rows: available_rows,
        stroke: at.scale_factor,
    };
    let line_height = at.line_height;
    let visible_start = (viewport.scroll_y / line_height).floor() as usize;
    let visible_end = ((viewport.scroll_y + viewport.height) / line_height).ceil() as usize;
    let Some(frame) = minimap.group_frame(
        at.group,
        doc.id,
        doc.buffer,
        visible_start..visible_end,
        available_rows,
        Some(doc.folds),
        doc.colors,
    ) else {
        return;
    };

    let bg = color(theme.background);
    let well = [bg[0] * 0.95, bg[1] * 0.95, bg[2] * 0.95, bg[3]];
    quad_instances.push(QuadInstance::solid(
        [strip.x, strip.y, strip.w, strip.h],
        well,
    ));
    paint_strokes(&frame, &strip, theme, quad_instances);
    paint_viewport_box(doc, viewport, line_height, &strip, theme, quad_instances);
    paint_marks(marks, doc, &strip, theme, quad_instances);
}

/// The focused document's minimap, with its git and search marks.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_focused(
    ui: &mut UiOverlays,
    id: u64,
    buffer: &Buffer,
    syntax: Option<&SyntaxState>,
    (bookmarks, folds): (&BookmarkState, &FoldState),
    viewport: &Viewport,
    at: Placement,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let colors = syntax.map(SyntaxColors);
    let doc = MinimapDoc {
        id,
        buffer,
        colors: colors.as_ref().map(|c| c as &dyn LineColors),
        bookmarks,
        folds,
    };
    let marks = Marks {
        git: &ui.git.markers,
        search: ui.search_bar.as_ref(),
    };
    render_minimap(
        &mut ui.minimap,
        &doc,
        Some(&marks),
        viewport,
        at,
        theme,
        quad_instances,
    );
}

/// Each row's words, in their syntax colors, one column per 1.2 points. All
/// strokes join the frame's one instanced quad batch.
fn paint_strokes(
    frame: &MinimapFrame,
    strip: &Strip,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let char_w = 1.2 * strip.stroke;
    let left = strip.x + 6.0 * strip.stroke;
    let right = strip.x + strip.w - 4.0 * strip.stroke;
    let plain = density_bar(color(theme.foreground), color(theme.background));
    let h = (strip.row_h - strip.stroke).max(1.0);
    // A half-pixel radius makes the quad shader's coverage soften each
    // stroke's ends, so a word reads as a smudge of text, not a hard block.
    let radius = 0.5 * strip.stroke;
    for row in 0..frame.rows.len() {
        let y = strip.row_y(row);
        for span in frame.row_spans(row) {
            let x = left + span.start as f32 * char_w;
            if x >= right {
                break;
            }
            let w = (span.len as f32 * char_w).min(right - x);
            let ink = span.color.map_or(plain, |[r, g, b]| {
                [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 0.85]
            });
            quad_instances.push(QuadInstance::rounded([x, y, w, h], ink, radius));
        }
    }
}

/// The translucent "you are here" box. Its top follows the scroll offset
/// continuously, so a drag moves it smoothly rather than a row at a time.
fn paint_viewport_box(
    doc: &MinimapDoc<'_>,
    viewport: &Viewport,
    line_height: f32,
    strip: &Strip,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let total_visual = doc.folds.total_visual_lines(doc.buffer.len_lines()).max(1);
    let per_row = lux_ui::minimap::lines_per_row(total_visual, strip.rows) as f32;
    let scale = strip.row_h / (per_row * line_height.max(1.0));
    let (offset, vh) = drag::box_extent(
        viewport.scroll_y,
        viewport.height,
        scale,
        strip.row_h,
        strip.h,
    );
    let vy = strip.y + offset;
    let sel = color(theme.selection);
    quad_instances.push(QuadInstance::solid(
        [strip.x, vy, strip.w, vh],
        viewport_band(theme),
    ));
    for edge in [vy, (vy + vh - strip.stroke).max(vy)] {
        quad_instances.push(QuadInstance::solid(
            [strip.x, edge, strip.w, strip.stroke],
            sel,
        ));
    }
}

/// The visible-region band: the foreground at 8% on a dark page and 6%
/// on a light one. An 18% tint of Lux Dark's amber selection painted a
/// strong brown block over the top of the strip.
fn viewport_band(theme: &Theme) -> [f32; 4] {
    let ink = color(theme.foreground);
    let page = color(theme.background);
    let dark = page[0] + page[1] + page[2] < 1.5;
    [ink[0], ink[1], ink[2], if dark { 0.08 } else { 0.06 }]
}

/// Git changes on the left edge (green added, orange modified, red
/// removed), bookmarks on the right edge, and Cmd+F matches as gold ticks
/// across the strip.
fn paint_marks(
    marks: Option<&Marks<'_>>,
    doc: &MinimapDoc<'_>,
    strip: &Strip,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let total_lines = doc.buffer.len_lines();
    let row_of = |line: usize| {
        row_for_buffer_line(total_lines, strip.rows, line, Some(doc.folds))
            .filter(|row| *row < strip.rows)
    };
    let edge_w = 2.0 * strip.stroke;
    let git = marks.map_or(&[][..], |m| m.git);
    for marker in git {
        let Some(row) = row_of((marker.line as usize).saturating_sub(1)) else {
            continue;
        };
        let kind = match marker.kind {
            HunkKind::Added => GutterKind::Added,
            HunkKind::Modified => GutterKind::Modified,
            HunkKind::Removed => GutterKind::Removed,
        };
        quad_instances.push(QuadInstance::solid(
            [strip.x, strip.row_y(row), edge_w, strip.row_h],
            color(gutter_color(kind, theme.background)),
        ));
    }
    for line in doc.bookmarks.iter() {
        if let Some(row) = row_of(line) {
            quad_instances.push(QuadInstance::solid(
                [
                    strip.x + strip.w - edge_w,
                    strip.row_y(row),
                    edge_w,
                    strip.row_h,
                ],
                BOOKMARK,
            ));
        }
    }
    let Some(search) = marks.and_then(|m| m.search) else {
        return;
    };
    let current = search.search_bar.current_index();
    for (i, m) in search.search_bar.matches().iter().enumerate() {
        let Some(row) = row_of(m.head.line) else {
            continue;
        };
        let tick = if current == Some(i) {
            [1.0, 0.85, 0.20, 0.95]
        } else {
            [0.85, 0.65, 0.15, 0.70]
        };
        quad_instances.push(QuadInstance::solid(
            [
                strip.x,
                strip.row_y(row),
                strip.w,
                strip.row_h.max(2.0 * strip.stroke),
            ],
            tick,
        ));
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
    use super::{composited_gap, density_bar, viewport_band};
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

    #[test]
    fn the_viewport_band_is_a_faint_foreground_on_every_theme() {
        let schemes = crate::color_scheme::bundled_schemes();
        for name in crate::color_scheme::theme_names(schemes) {
            let theme = crate::color_scheme::theme_for(schemes, name);
            let band = viewport_band(&theme);
            let ink = color(theme.foreground);
            assert_eq!(&band[..3], &ink[..3], "{name}");
            let page = color(theme.background);
            let dark = page[0] + page[1] + page[2] < 1.5;
            assert_eq!(band[3], if dark { 0.08 } else { 0.06 }, "{name}");
            // Subtle but present: two to thirty 8-bit levels off the well.
            let shift = (0..3)
                .map(|c| {
                    let well = page[c] * 0.95;
                    let mixed = band[c] * band[3] + well * (1.0 - band[3]);
                    ((mixed - well) * 255.0).abs()
                })
                .fold(0.0_f32, f32::max);
            assert!((2.0..=30.0).contains(&shift), "{name}: band shifts {shift}");
        }
    }
}

#[cfg(test)]
#[path = "minimap_marks_tests.rs"]
mod marks_tests;
