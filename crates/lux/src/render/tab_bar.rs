//! Render pass for the top multi-document tab bar.

use crate::document::Documents;
use crate::render::clip::{clip_glyphs, clip_rect};
use crate::render::paint::{color, push_glyph};
use crate::tab_strip::StripLayout;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::tabs::Tab;
use lux_ui::{EditorView, Theme};
use swash::FontRef;
use swash::scale::ScaleContext;

/// Tab titles are the system UI face at 12 pt, like Sublime Text's.
const TAB_FONT_PT: f32 = 12.0;
/// Diameter of the unsaved dot that stands in for a dirty tab's `×`.
const DIRTY_DOT_PT: f32 = 8.0;

/// How one tab is drawn besides its title.
#[derive(Clone, Copy)]
struct Look {
    active: bool,
    dirty: bool,
    /// Goto Anything's transient preview. The tab font has no italic face
    /// and the text pipeline cannot slant glyphs, so the title is
    /// underlined with a dashed rule instead.
    transient: bool,
    /// The pointer is on this tab's close mark: a dirty tab shows `×`
    /// there instead of its dot.
    close_hovered: bool,
}

/// Everything one tab, the `▼` button, or the context menu draws into.
struct Painter<'a> {
    theme: &'a Theme,
    stroke: f32,
    scale_factor: f32,
    height: f32,
    font_size: f32,
    family: String,
    font: FontRef<'static>,
    editor_view: &'a mut EditorView,
    atlas: &'a mut GlyphAtlas,
    atlas_dirty: &'a mut bool,
    scale_cx: &'a mut ScaleContext,
    quads: &'a mut Vec<QuadInstance>,
    glyphs: &'a mut Vec<GlyphInstance>,
}

impl Painter<'_> {
    /// Draws `text` from `x`, vertically centered in a band at `top` of
    /// `band` height, stopping before `max_width`.
    fn text(&mut self, text: &str, x: f32, top: f32, band: f32, max_width: f32, ink: [f32; 4]) {
        let style = lux_render::LineStyle {
            family: &self.family,
            font_size: self.font_size,
            ligatures: false,
        };
        let layout = self
            .editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[]);
        let y = top + (band - self.font_size) * 0.5 + layout.baseline();
        for cluster in layout.clusters() {
            if cluster.x > max_width {
                break;
            }
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
    }

    /// Advance width of `text` in the painter's face.
    fn measure(&mut self, text: &str) -> f32 {
        let style = lux_render::LineStyle {
            family: &self.family,
            font_size: self.font_size,
            ligatures: false,
        };
        self.editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[])
            .width()
    }

    fn tab(&mut self, tab: &Tab, x: f32, width: f32, look: Look, close_w: f32) {
        let Look {
            active,
            dirty,
            transient,
            close_hovered,
        } = look;
        let stroke = self.stroke;
        let height = self.height;
        let theme = self.theme;
        let fill = if active {
            theme.background
        } else {
            theme.chrome.tab_inactive
        };
        self.quads.push(QuadInstance::solid(
            [x, 0.0, width, height - stroke],
            color(fill),
        ));
        if active {
            self.quads.push(QuadInstance::solid(
                [x, 0.0, width, 2.0 * stroke],
                color(theme.accent),
            ));
        }
        self.quads.push(QuadInstance::solid(
            [x + width - stroke, 0.0, stroke, height - stroke],
            color(theme.chrome.border),
        ));
        // Inactive wells are `#34352F`. Monokai muted (`#75715E`) on that
        // fill is about 2.5:1, so the title and the close mark use the
        // foreground. The well and the accent bar distinguish the active tab.
        let foreground = color(theme.foreground);
        let title_x = x + 10.0 * stroke;
        let max_title_w = (width - close_w - 14.0 * stroke).max(0.0);
        let title =
            crate::tab_strip::title::elide(&tab.title, max_title_w, &mut |text| self.measure(text));
        self.text(&title, title_x, 0.0, height, max_title_w, foreground);
        if transient {
            let underline = self.measure(&title).min(max_title_w);
            self.dashed_rule(title_x, height - 7.0 * stroke, underline);
        }
        if tab.pinned {
            self.quads.push(QuadInstance::solid(
                [
                    x + 2.0 * stroke,
                    4.0 * stroke,
                    3.0 * stroke,
                    height - 8.0 * stroke,
                ],
                color(theme.accent),
            ));
        }
        let close_center = x + width - close_w * 0.5;
        if dirty && !close_hovered {
            // Sublime's unsaved dot takes the close mark's place; hovering
            // it shows the `×` that closes (with the unsaved prompt).
            let dot = DIRTY_DOT_PT * self.scale_factor;
            let [r, g, b, a] = foreground;
            self.quads.push(QuadInstance::rounded(
                [close_center - dot * 0.5, (height - dot) * 0.5, dot, dot],
                [r, g, b, a * 0.7],
                dot * 0.5,
            ));
        } else {
            let mark_w = self.measure("×");
            self.text(
                "×",
                close_center - mark_w * 0.5,
                0.0,
                height,
                f32::MAX,
                foreground,
            );
        }
    }

    /// A one-stroke dashed rule from `x`, `width` long.
    fn dashed_rule(&mut self, x: f32, y: f32, width: f32) {
        let unit = self.stroke.max(1.0);
        let (dash, gap) = (4.0 * unit, 3.0 * unit);
        let ink = color(self.theme.foreground);
        let mut at = 0.0;
        while at < width {
            let w = dash.min(width - at);
            self.quads
                .push(QuadInstance::solid([x + at, y, w, unit], ink));
            at += dash + gap;
        }
    }

    /// The `▼` button: a well with a left rule and a triangle of bars, so
    /// it does not depend on the editor font having the glyph.
    fn overflow_button(&mut self, [x, width]: [f32; 2], open: bool) {
        let stroke = self.stroke;
        let height = self.height;
        let fill = if open {
            self.theme.background
        } else {
            self.theme.chrome.tab_inactive
        };
        self.quads.push(QuadInstance::solid(
            [x, 0.0, width, height - stroke],
            color(fill),
        ));
        self.quads.push(QuadInstance::solid(
            [x, 0.0, stroke, height - stroke],
            color(self.theme.chrome.border),
        ));
        let ink = color(self.theme.foreground);
        let unit = stroke.max(1.0);
        let center = x + width * 0.5;
        let top = (height - 4.0 * unit) * 0.5;
        for row in 0..4 {
            let half = (4 - row) as f32 * unit;
            self.quads.push(QuadInstance::solid(
                [center - half, top + row as f32 * unit, 2.0 * half, unit],
                ink,
            ));
        }
    }
}

fn look(documents: &Documents, tab: &Tab, is_current_dirty: bool) -> Look {
    let active = Some(tab.id) == documents.active_tab;
    Look {
        active,
        dirty: tab_dirty(documents, tab, active, is_current_dirty),
        transient: crate::goto::preview::is_transient(documents, tab.id),
        close_hovered: documents.strip.hover == Some((tab.id, true)),
    }
}

fn tab_dirty(documents: &Documents, tab: &Tab, active: bool, is_current_dirty: bool) -> bool {
    if active {
        return is_current_dirty;
    }
    documents
        .entries
        .iter()
        .find(|e| e.tab_id == tab.id)
        .is_some_and(|e| e.is_dirty())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_tab_bar(
    documents: &Documents,
    is_current_dirty: bool,
    strip_left: f32,
    strip_width: f32,
    tab_width: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let (font, family) = crate::font::ui_font(font, editor_view.family());
    let mut painter = Painter {
        theme,
        stroke,
        scale_factor,
        height: tab_bar_height,
        font_size: TAB_FONT_PT * scale_factor,
        family,
        font,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        quads: quad_instances,
        glyphs: glyph_instances,
    };
    let close_w = crate::app::TAB_CLOSE_BTN_WIDTH * scale_factor;
    let tabs = documents.tabs.tabs();
    let strip = StripLayout::new(strip_left, strip_width, tab_width, tabs.len(), scale_factor);
    let scroll = documents.tabs.scroll();
    let origin = strip_left - scroll;
    let strip_right = strip.right();
    let drag = documents.strip.drag.filter(|d| d.moving);
    let (quads_from, glyphs_from) = (painter.quads.len(), painter.glyphs.len());

    for (i, tab) in tabs.iter().enumerate() {
        let tab_x = origin + i as f32 * tab_width;
        // A dragged tab's slot stays empty; the tab follows the pointer.
        if drag.is_some_and(|d| d.id == tab.id)
            || tab_x + tab_width <= strip_left
            || tab_x >= strip_right
        {
            continue;
        }
        let look = look(documents, tab, is_current_dirty);
        painter.tab(tab, tab_x, tab_width, look, close_w);
    }

    if let Some(drag) = drag {
        let slot = strip.drop_slot(scroll, drag.pointer_x);
        let marker_w = 2.0 * stroke.max(1.0);
        let marker_x = (origin + slot as f32 * tab_width - marker_w * 0.5)
            .clamp(strip_left, (strip_right - marker_w).max(strip_left));
        if let Some(tab) = tabs.iter().find(|t| t.id == drag.id) {
            let x = (drag.pointer_x - drag.grab_offset)
                .clamp(strip_left, (strip_right - tab_width).max(strip_left));
            let look = look(documents, tab, is_current_dirty);
            painter.tab(tab, x, tab_width, look, close_w);
        }
        painter.quads.push(QuadInstance::solid(
            [marker_x, 0.0, marker_w, tab_bar_height - stroke],
            color(theme.accent),
        ));
    }
    // A tab scrolled half out of the strip is cut at the strip's edges, so
    // it never covers the sidebar column, the window controls, or `▼`.
    let bounds = [strip_left, 0.0, strip.visible_width, tab_bar_height];
    clip_quads(painter.quads, quads_from, bounds);
    clip_glyphs(painter.glyphs, glyphs_from, bounds);

    if let Some(button) = strip.button {
        painter.overflow_button(button, documents.strip.list.is_some());
    }

    if let Some(menu) = &documents.tab_context_menu {
        let [mx, my, mw, mh] = menu.rect;
        painter.quads.push(QuadInstance::solid(
            [
                mx - stroke,
                my - stroke,
                mw + 2.0 * stroke,
                mh + 2.0 * stroke,
            ],
            color(theme.chrome.panel),
        ));
        painter.quads.push(QuadInstance::solid(
            [mx, my, mw, mh],
            color(theme.chrome.panel_input),
        ));
        let item_h = crate::document::tab_ops::TAB_MENU_ITEM_HEIGHT * scale_factor;
        let foreground = color(theme.foreground);
        for (idx, item) in menu.items.iter().enumerate() {
            let item_y = my + 4.0 * scale_factor + idx as f32 * item_h;
            let label = item.label(menu.is_pinned);
            painter.text(
                label,
                mx + 12.0 * stroke,
                item_y,
                item_h,
                f32::MAX,
                foreground,
            );
        }
    }
}

fn clip_quads(quads: &mut Vec<QuadInstance>, start: usize, bounds: [f32; 4]) {
    let mut index = start;
    while index < quads.len() {
        match clip_rect(quads[index].rect, bounds) {
            Some(rect) => {
                quads[index].rect = rect;
                index += 1;
            }
            None => {
                quads.remove(index);
            }
        }
    }
}

#[cfg(test)]
#[path = "tab_bar_tests.rs"]
mod tests;
