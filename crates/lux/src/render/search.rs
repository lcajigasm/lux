//! The docked find bar and the find-match highlights in the text.
//!
//! The bar's geometry comes from [`crate::search::geometry::layout`], the
//! same function the click handler reads, and it spans the editor column
//! that [`crate::frame_layout`] gives it. Match boxes use the selection
//! geometry of the current frame.

use swash::FontRef;
use swash::scale::ScaleContext;

use super::clip::{clip_glyphs, clip_rect};
use super::icons::{Icon, push_icon};
use super::paint::{color, push_glyph};
use super::panel::{distinct_fill, hint_ink, label_ink};
use super::text::{selection_quad, text_bounds};
use crate::search::geometry::{self, TEXT_INSET};
use crate::search::{SearchBarState, SearchField};
use lux_core::buffer::Buffer;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::{EditorView, EditorViewFrame, Viewport};

/// Fill of a find match. Lux themes carry no find-highlight color yet, so
/// this is the documented fallback: the accent at 30% alpha.
pub(crate) fn match_fill(theme: &lux_ui::Theme) -> [f32; 4] {
    let accent = color(theme.accent);
    [accent[0], accent[1], accent[2], 0.30]
}

/// Where the text of the editor sits, as `render_text_lines` uses it.
pub(crate) struct TextOrigin<'a> {
    pub viewport: &'a Viewport,
    pub editor_top: f32,
    pub sidebar_width: f32,
    pub text_left: f32,
    pub line_height: f32,
}

/// Window rectangles of every match on the visible lines, computed the way
/// selections are. Matches are in document order, so only the visible
/// slice is walked.
pub(crate) fn match_quads(
    search: &SearchBarState,
    buffer: &Buffer,
    frame: &EditorViewFrame,
    origin: &TextOrigin<'_>,
) -> Vec<(usize, [f32; 4])> {
    let (Some(first), Some(last)) = (frame.lines.first(), frame.lines.last()) else {
        return Vec::new();
    };
    let matches = search.search_bar.matches();
    let start = matches.partition_point(|m| m.ordered().1.line < first.index);
    let mut out = Vec::new();
    for (index, m) in matches.iter().enumerate().skip(start) {
        let (from, to) = m.ordered();
        if from.line > last.index {
            break;
        }
        for rect in frame.range_rects(buffer, from, to, origin.line_height) {
            let quad = selection_quad(
                &rect,
                origin.viewport,
                origin.editor_top,
                origin.text_left,
                origin.line_height,
            );
            out.push((index, quad));
        }
    }
    out
}

pub(crate) fn render_search_matches(
    search: &SearchBarState,
    buffer: &Buffer,
    frame: &EditorViewFrame,
    origin: &TextOrigin<'_>,
    stroke: f32,
    theme: &lux_ui::Theme,
    quads: &mut Vec<QuadInstance>,
) {
    let bounds = text_bounds(
        origin.viewport,
        origin.editor_top,
        origin.sidebar_width,
        origin.text_left,
    );
    let fill = match_fill(theme);
    let outline = color(theme.accent);
    let current = search.search_bar.current_index();
    let mut push = |rect: [f32; 4], ink: [f32; 4]| {
        if let Some(rect) = clip_rect(rect, bounds) {
            quads.push(QuadInstance::solid(rect, ink));
        }
    };
    for (index, [x, y, w, h]) in match_quads(search, buffer, frame, origin) {
        push([x, y, w, h], fill);
        if current == Some(index) {
            push([x, y, w, stroke], outline);
            push([x, y + h - stroke, w, stroke], outline);
            push([x, y, stroke, h], outline);
            push([x + w - stroke, y, stroke, h], outline);
        }
    }
}

/// Shapes and places the bar's short labels.
struct Labels<'a> {
    editor_view: &'a mut EditorView,
    atlas: &'a mut GlyphAtlas,
    atlas_dirty: &'a mut bool,
    scale_cx: &'a mut ScaleContext,
    font: FontRef<'static>,
    font_size: f32,
    glyphs: &'a mut Vec<GlyphInstance>,
}

impl Labels<'_> {
    fn width(&mut self, text: &str) -> f32 {
        let family = self.editor_view.family().to_string();
        let style = lux_render::LineStyle {
            family: &family,
            font_size: self.font_size,
            ligatures: false,
        };
        self.editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[])
            .width()
    }

    /// Draws `text` from `x`, vertically centered in `row` (`[y, h]`), and
    /// returns its width.
    fn draw(&mut self, text: &str, x: f32, row: [f32; 2], ink: [f32; 4]) -> f32 {
        let family = self.editor_view.family().to_string();
        let style = lux_render::LineStyle {
            family: &family,
            font_size: self.font_size,
            ligatures: false,
        };
        let line = self
            .editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[]);
        let y = row[0] + (row[1] - self.font_size) * 0.5 + line.baseline();
        for cluster in line.clusters() {
            push_glyph(
                self.atlas,
                self.atlas_dirty,
                self.scale_cx,
                self.font,
                self.font_size,
                &cluster,
                x,
                y,
                ink,
                self.glyphs,
            );
        }
        line.width()
    }

    /// Draws `text` centered in `rect`, cut to it.
    fn centered(&mut self, text: &str, rect: [f32; 4], ink: [f32; 4]) {
        let start = self.glyphs.len();
        let x = rect[0] + (rect[2] - self.width(text)) * 0.5;
        self.draw(text, x, [rect[1], rect[3]], ink);
        clip_glyphs(self.glyphs, start, rect);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_search_bar(
    search: &mut SearchBarState,
    bar: [f32; 4],
    scale_factor: f32,
    stroke: f32,
    font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let layout = geometry::layout(bar, scale_factor, search.replace_mode);
    search.layout = layout.clone();
    let page = color(theme.chrome.panel);
    let control = distinct_fill(color(theme.chrome.panel_input), page);
    let accent = color(theme.accent);
    let mut labels = Labels {
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        font_size: font_size * 0.78,
        glyphs,
    };

    quads.push(QuadInstance::solid(bar, page));
    quads.push(QuadInstance::solid(
        [bar[0], bar[1], bar[2], stroke],
        color(theme.chrome.border),
    ));

    let toggles = [
        (layout.regex_btn_rect, ".*", search.regex),
        (layout.case_btn_rect, "Aa", search.case_sensitive),
        (layout.word_btn_rect, "\"w\"", search.whole_word),
        (layout.selection_btn_rect, "", search.in_selection),
        (
            layout.preserve_case_btn_rect,
            "AB",
            search.replace_mode && search.preserve_case,
        ),
    ];
    for (rect, label, active) in toggles {
        if rect[2] <= 0.0 {
            continue;
        }
        let fill = if active { accent } else { control };
        let ink = label_ink(theme, fill);
        quads.push(QuadInstance::rounded(rect, fill, 3.0 * scale_factor));
        if label.is_empty() {
            let size = (14.0 * scale_factor).min(rect[2] - 4.0 * scale_factor);
            let (x, y) = (
                rect[0] + (rect[2] - size) * 0.5,
                rect[1] + (rect[3] - size) * 0.5,
            );
            push_icon(Icon::Selection, quads, x, y, size, ink, fill);
        } else {
            labels.centered(label, rect, ink);
        }
    }

    let counter = match_counter_label(
        search.find_query.is_empty(),
        search.search_bar.matches().len(),
        search.search_bar.current_index(),
        search.in_selection,
    );
    let find_focused = search.focused_field == SearchField::Find;
    paint_field(
        &mut labels,
        quads,
        theme,
        control,
        layout.find_input_rect,
        (search.find_query.as_str(), "Find"),
        &counter,
        find_focused,
        stroke,
        scale_factor,
    );
    if search.replace_mode {
        paint_field(
            &mut labels,
            quads,
            theme,
            control,
            layout.replace_input_rect,
            (search.replace_text.as_str(), "Replace"),
            "",
            !find_focused,
            stroke,
            scale_factor,
        );
    }

    let buttons = [
        (layout.next_btn_rect, "Find"),
        (layout.prev_btn_rect, "Prev"),
        (layout.find_all_btn_rect, "Find All"),
        (layout.replace_btn_rect, "Replace"),
        (layout.replace_all_btn_rect, "Replace All"),
    ];
    let button_ink = label_ink(theme, control);
    for (rect, label) in buttons {
        if rect[2] > 0.0 {
            quads.push(QuadInstance::rounded(rect, control, 3.0 * scale_factor));
            labels.centered(label, rect, button_ink);
        }
    }

    push_cross(
        quads,
        layout.close_btn_rect,
        8.0 * scale_factor,
        1.5 * stroke,
        label_ink(theme, page),
    );
}

/// One text field: fill, border (accent when focused), the text or its
/// placeholder inset `TEXT_INSET`, the caret, and an optional muted note
/// right-aligned inside (the match counter).
#[allow(clippy::too_many_arguments)]
fn paint_field(
    labels: &mut Labels<'_>,
    quads: &mut Vec<QuadInstance>,
    theme: &lux_ui::Theme,
    fill: [f32; 4],
    rect: [f32; 4],
    (text, placeholder): (&str, &str),
    note: &str,
    focused: bool,
    stroke: f32,
    scale: f32,
) {
    let [x, y, w, h] = rect;
    if w <= 0.0 {
        return;
    }
    quads.push(QuadInstance::solid(rect, fill));
    let border = if focused {
        color(theme.accent)
    } else {
        color(theme.chrome.border)
    };
    for edge in [
        [x, y, w, stroke],
        [x, y + h - stroke, w, stroke],
        [x, y, stroke, h],
        [x + w - stroke, y, stroke, h],
    ] {
        quads.push(QuadInstance::solid(edge, border));
    }
    let inset = TEXT_INSET * scale;
    let mut text_right = x + w - inset;
    if !note.is_empty() {
        let note_w = labels.width(note);
        text_right -= note_w + inset;
        labels.draw(note, x + w - inset - note_w, [y, h], hint_ink(theme, fill));
    }
    let start = labels.glyphs.len();
    let (shown, ink) = if text.is_empty() {
        (placeholder, hint_ink(theme, fill))
    } else {
        (text, label_ink(theme, fill))
    };
    let width = labels.draw(shown, x + inset, [y, h], ink);
    clip_glyphs(
        labels.glyphs,
        start,
        [x + inset, y, (text_right - x - inset).max(0.0), h],
    );
    if focused {
        let caret_x = if text.is_empty() {
            x + inset
        } else {
            (x + inset + width + stroke).min(text_right)
        };
        quads.push(QuadInstance::solid(
            [caret_x, y + 3.0 * scale, 2.0 * stroke, h - 6.0 * scale],
            color(theme.accent),
        ));
    }
}

/// A × drawn as two diagonals of small squares, centered in `rect`.
fn push_cross(quads: &mut Vec<QuadInstance>, rect: [f32; 4], size: f32, dot: f32, ink: [f32; 4]) {
    if rect[2] <= 0.0 {
        return;
    }
    let (cx, cy) = (rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5);
    let half = size * 0.5;
    let steps = ((size / (dot * 0.5)).ceil() as usize).max(2);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = cx - half + size * t - dot * 0.5;
        for y in [cy - half + size * t, cy + half - size * t] {
            quads.push(QuadInstance::solid([x, y - dot * 0.5, dot, dot], ink));
        }
    }
}

pub(crate) fn match_counter_label(
    query_empty: bool,
    total: usize,
    current: Option<usize>,
    in_selection: bool,
) -> String {
    if query_empty {
        String::new()
    } else if total == 0 {
        "No results".to_string()
    } else {
        let cur = current.map_or(1, |i| i + 1);
        let in_sel = if in_selection { " in sel" } else { "" };
        format!("{cur} of {total}{in_sel}")
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
