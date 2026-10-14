//! Render pass for the top multi-document tab bar.

use crate::document::Documents;
use crate::render::paint::{color, push_glyph};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::{EditorView, Theme};
use swash::FontRef;
use swash::scale::ScaleContext;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_tab_bar(
    documents: &Documents,
    is_current_dirty: bool,
    sidebar_width: f32,
    tab_width: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let foreground = color(theme.foreground);
    let tab_font_size = font_size * 0.82;
    let family = editor_view.family().to_string();
    let tab_style = lux_render::LineStyle {
        family: &family,
        font_size: tab_font_size,
        ligatures: false,
    };
    let close_btn_w = crate::app::TAB_CLOSE_BTN_WIDTH * scale_factor;

    for (i, tab) in documents.tabs.tabs().iter().enumerate() {
        let tab_x = sidebar_width + i as f32 * tab_width;
        let is_active = Some(tab.id) == documents.active_tab;
        let tab_bg = if is_active {
            color(theme.background)
        } else {
            [0.075, 0.075, 0.10, 1.0]
        };
        quad_instances.push(QuadInstance {
            rect: [tab_x, 0.0, tab_width, tab_bar_height - stroke],
            color: tab_bg,
        });
        if is_active {
            quad_instances.push(QuadInstance {
                rect: [tab_x, 0.0, tab_width, 2.0 * stroke],
                color: color(theme.accent),
            });
        }
        quad_instances.push(QuadInstance {
            rect: [
                tab_x + tab_width - stroke,
                0.0,
                stroke,
                tab_bar_height - stroke,
            ],
            color: [0.15, 0.15, 0.20, 1.0],
        });

        let is_tab_dirty = if is_active {
            is_current_dirty
        } else {
            documents
                .entries
                .iter()
                .find(|e| e.tab_id == tab.id)
                .is_some_and(|e| e.is_dirty())
        };

        let title_color = if is_active {
            foreground
        } else {
            color(theme.muted)
        };

        let max_title_w = (tab_width - close_btn_w - 18.0 * stroke).max(0.0);
        let layout = editor_view
            .text_layout_mut()
            .layout_line(&tab_style, &tab.title, &[]);
        let title_y = (tab_bar_height - tab_font_size) * 0.5;
        for cluster in layout.clusters() {
            if cluster.x + 8.0 * stroke > max_title_w {
                break;
            }
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                tab_font_size,
                &cluster,
                tab_x + 8.0 * stroke,
                title_y + layout.baseline(),
                title_color,
                glyph_instances,
            );
        }

        if tab.pinned {
            let pin_indicator_w = 3.0 * stroke;
            quad_instances.push(QuadInstance {
                rect: [
                    tab_x + 2.0 * stroke,
                    4.0 * stroke,
                    pin_indicator_w,
                    tab_bar_height - 8.0 * stroke,
                ],
                color: color(theme.accent),
            });
        }

        if is_tab_dirty {
            let dot_size = 5.0 * stroke;
            let dot_x = tab_x + tab_width - close_btn_w - dot_size - 2.0 * stroke;
            let dot_y = (tab_bar_height - dot_size) * 0.5;
            quad_instances.push(QuadInstance {
                rect: [dot_x, dot_y, dot_size, dot_size],
                color: color(theme.accent),
            });
        }

        let close_layout = editor_view
            .text_layout_mut()
            .layout_line(&tab_style, "×", &[]);
        let close_x = tab_x + tab_width - close_btn_w * 0.7;
        for cluster in close_layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                tab_font_size,
                &cluster,
                close_x,
                title_y + close_layout.baseline(),
                color(theme.muted),
                glyph_instances,
            );
        }
    }

    if let Some(menu) = &documents.tab_context_menu {
        let [mx, my, mw, mh] = menu.rect;
        // Background and border
        quad_instances.push(QuadInstance {
            rect: [
                mx - stroke,
                my - stroke,
                mw + 2.0 * stroke,
                mh + 2.0 * stroke,
            ],
            color: [0.06, 0.06, 0.10, 0.98],
        });
        quad_instances.push(QuadInstance {
            rect: [mx, my, mw, mh],
            color: [0.14, 0.14, 0.20, 1.0],
        });
        let item_h = crate::document::tab_ops::TAB_MENU_ITEM_HEIGHT * scale_factor;
        for (idx, item) in menu.items.iter().enumerate() {
            let item_y = my + 4.0 * scale_factor + idx as f32 * item_h;
            let label = item.label(menu.is_pinned);
            let layout = editor_view
                .text_layout_mut()
                .layout_line(&tab_style, label, &[]);
            let text_y = item_y + (item_h - tab_font_size) * 0.5;
            for cluster in layout.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    tab_font_size,
                    &cluster,
                    mx + 12.0 * stroke,
                    text_y + layout.baseline(),
                    foreground,
                    glyph_instances,
                );
            }
        }
    }
}
