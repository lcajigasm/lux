//! Tab strip + status bar painting for the desktop shell (G05).

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use lux_ui::status_bar::StatusBar;
use lux_ui::tabs::TabBar;
use lux_ui::Chrome;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::glyph_draw::{color, push_glyph};
use crate::tab_session::{STATUS_BAR_HEIGHT, TAB_CLOSE_WIDTH, TAB_STRIP_HEIGHT};

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_tab_strip(
    chrome: &Chrome,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    font_size: f32,
    bar: &TabBar,
    window_w: f32,
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    quads.push(QuadInstance {
        rect: [0.0, 0.0, window_w, TAB_STRIP_HEIGHT],
        color: [
            chrome.theme.background.r as f32 / 255.0,
            chrome.theme.background.g as f32 / 255.0,
            chrome.theme.background.b as f32 / 255.0,
            1.0,
        ],
    });
    // Subtle top chrome separation from the editor.
    quads.push(QuadInstance {
        rect: [0.0, TAB_STRIP_HEIGHT - 1.0, window_w, 1.0],
        color: color(chrome.theme.muted),
    });
    let tabs = bar.tabs();
    if tabs.is_empty() {
        return;
    }
    let tab_w = window_w / tabs.len() as f32;
    let active_id = bar.active().map(|t| t.id);
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: font_size * 0.85,
        ligatures: false,
    };
    for (i, tab) in tabs.iter().enumerate() {
        let x = tab_w * i as f32;
        let is_active = Some(tab.id) == active_id;
        if is_active {
            quads.push(QuadInstance {
                rect: [x, 0.0, tab_w, TAB_STRIP_HEIGHT],
                color: color(chrome.theme.background),
            });
            quads.push(QuadInstance {
                rect: [x, TAB_STRIP_HEIGHT - 2.0, tab_w, 2.0],
                color: color(chrome.theme.accent),
            });
        } else {
            quads.push(QuadInstance {
                rect: [x, 0.0, tab_w, TAB_STRIP_HEIGHT],
                color: [
                    chrome.theme.muted.r as f32 / 255.0 * 0.25
                        + chrome.theme.background.r as f32 / 255.0 * 0.75,
                    chrome.theme.muted.g as f32 / 255.0 * 0.25
                        + chrome.theme.background.g as f32 / 255.0 * 0.75,
                    chrome.theme.muted.b as f32 / 255.0 * 0.25
                        + chrome.theme.background.b as f32 / 255.0 * 0.75,
                    1.0,
                ],
            });
        }
        let layout = editor_view
            .text_layout_mut()
            .layout_line(&style, &tab.title, &[]);
        let text_color = if is_active {
            color(chrome.theme.foreground)
        } else {
            color(chrome.theme.muted)
        };
        let text_x = x + 8.0;
        let text_y = (TAB_STRIP_HEIGHT - font_size * 0.85) * 0.5;
        for cluster in layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                &cluster,
                text_x,
                text_y,
                font_size * 0.85,
                text_color,
                glyphs,
            );
        }
        let close_layout = editor_view
            .text_layout_mut()
            .layout_line(&style, "×", &[]);
        let close_x = x + tab_w - TAB_CLOSE_WIDTH + 2.0;
        for cluster in close_layout.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                &cluster,
                close_x,
                text_y,
                font_size * 0.85,
                color(chrome.theme.muted),
                glyphs,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_status_bar(
    chrome: &Chrome,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    font_size: f32,
    status: &StatusBar,
    window_w: f32,
    window_h: f32,
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let y0 = window_h - STATUS_BAR_HEIGHT;
    quads.push(QuadInstance {
        rect: [0.0, y0, window_w, STATUS_BAR_HEIGHT],
        color: [
            chrome.theme.background.r as f32 / 255.0 * 0.55
                + chrome.theme.muted.r as f32 / 255.0 * 0.45,
            chrome.theme.background.g as f32 / 255.0 * 0.55
                + chrome.theme.muted.g as f32 / 255.0 * 0.45,
            chrome.theme.background.b as f32 / 255.0 * 0.55
                + chrome.theme.muted.b as f32 / 255.0 * 0.45,
            1.0,
        ],
    });
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: font_size * 0.8,
        ligatures: false,
    };
    let joined = [
        status.cursor.label.as_str(),
        status.language.label.as_str(),
        status.encoding.label.as_str(),
        status.eol.label.as_str(),
        status.indent.label.as_str(),
    ]
    .join("  |  ");
    let layout = editor_view
        .text_layout_mut()
        .layout_line(&style, &joined, &[]);
    let text_x = (window_w - layout.width() - 10.0).max(8.0);
    let text_y = y0 + (STATUS_BAR_HEIGHT - font_size * 0.8) * 0.5;
    for cluster in layout.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &cluster,
            text_x,
            text_y,
            font_size * 0.8,
            color(chrome.theme.foreground),
            glyphs,
        );
    }
}
