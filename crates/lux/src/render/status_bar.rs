//! Rendering logic for the interactive status bar at the bottom of the desktop window.

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, push_glyph};
use crate::status_bar::{
    Rect, STATUS_BAR_HEIGHT, STATUS_BAR_ITEM_SPACING, STATUS_BAR_PADDING, StatusBarLayout,
    build_status_bar,
};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_status_bar(
    window_width: f32,
    window_height: f32,
    scale_factor: f32,
    stroke: f32,
    font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    buffer: &lux_core::buffer::Buffer,
    cursor_pos: lux_core::position::Position,
    language_id: Option<lux_syntax::LanguageId>,
    is_recording_macro: bool,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) -> StatusBarLayout {
    let bar_height = STATUS_BAR_HEIGHT * scale_factor;
    let bar_y = window_height - bar_height;
    let status_font_size = font_size * 0.78;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: status_font_size,
        ligatures: false,
    };

    // 1. Background quad
    quad_instances.push(QuadInstance {
        rect: [0.0, bar_y, window_width, bar_height],
        color: color(theme.chrome.status_bar),
    });

    // 2. Top separator border
    quad_instances.push(QuadInstance {
        rect: [0.0, bar_y, window_width, stroke],
        color: color(theme.chrome.border),
    });

    let status = build_status_bar(buffer, cursor_pos, language_id);
    let foreground = color(theme.foreground);
    let muted = color(theme.muted);

    let mut layout = StatusBarLayout {
        bar_rect: Rect {
            x: 0.0,
            y: bar_y,
            width: window_width,
            height: bar_height,
        },
        ..Default::default()
    };

    let text_baseline_offset = (bar_height - status_font_size) * 0.5;

    // 3. Left item: Cursor Position
    let cursor_x = STATUS_BAR_PADDING * scale_factor;
    let cursor_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &status.cursor.label, &[]);
    let cursor_w = cursor_line.width();
    let cursor_baseline = cursor_line.baseline();
    for cluster in cursor_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            status_font_size,
            &cluster,
            cursor_x,
            bar_y + text_baseline_offset + cursor_baseline,
            foreground,
            glyph_instances,
        );
    }
    layout.cursor_rect = Rect {
        x: cursor_x - 4.0 * stroke,
        y: bar_y,
        width: cursor_w + 8.0 * stroke,
        height: bar_height,
    };

    // Recording Macro indicator (Sublime Text 3 parity)
    if is_recording_macro {
        let macro_text = "● Recording Macro";
        let macro_x = cursor_x + cursor_w + STATUS_BAR_ITEM_SPACING * scale_factor;
        let macro_line = editor_view
            .text_layout_mut()
            .layout_line(&style, macro_text, &[]);
        let macro_baseline = macro_line.baseline();
        let macro_color = [0.95, 0.28, 0.28, 1.0];
        for cluster in macro_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                status_font_size,
                &cluster,
                macro_x,
                bar_y + text_baseline_offset + macro_baseline,
                macro_color,
                glyph_instances,
            );
        }
    }

    // 4. Right items: Language, EOL, Encoding, Indent
    let spacing = STATUS_BAR_ITEM_SPACING * scale_factor;
    let mut right_cursor = window_width - STATUS_BAR_PADDING * scale_factor;

    // Items from right to left:
    // (a) Language
    let lang_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &status.language.label, &[]);
    let lang_w = lang_line.width();
    let lang_x = right_cursor - lang_w;
    let lang_baseline = lang_line.baseline();
    for cluster in lang_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            status_font_size,
            &cluster,
            lang_x,
            bar_y + text_baseline_offset + lang_baseline,
            muted,
            glyph_instances,
        );
    }
    layout.language_rect = Rect {
        x: lang_x - 4.0 * stroke,
        y: bar_y,
        width: lang_w + 8.0 * stroke,
        height: bar_height,
    };
    right_cursor = lang_x - spacing;

    // (b) EOL
    let eol_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &status.eol.label, &[]);
    let eol_w = eol_line.width();
    let eol_x = right_cursor - eol_w;
    let eol_baseline = eol_line.baseline();
    for cluster in eol_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            status_font_size,
            &cluster,
            eol_x,
            bar_y + text_baseline_offset + eol_baseline,
            muted,
            glyph_instances,
        );
    }
    layout.eol_rect = Rect {
        x: eol_x - 4.0 * stroke,
        y: bar_y,
        width: eol_w + 8.0 * stroke,
        height: bar_height,
    };
    right_cursor = eol_x - spacing;

    // (c) Encoding
    let enc_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &status.encoding.label, &[]);
    let enc_w = enc_line.width();
    let enc_x = right_cursor - enc_w;
    let enc_baseline = enc_line.baseline();
    for cluster in enc_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            status_font_size,
            &cluster,
            enc_x,
            bar_y + text_baseline_offset + enc_baseline,
            muted,
            glyph_instances,
        );
    }
    layout.encoding_rect = Rect {
        x: enc_x - 4.0 * stroke,
        y: bar_y,
        width: enc_w + 8.0 * stroke,
        height: bar_height,
    };
    right_cursor = enc_x - spacing;

    // (d) Indent
    let ind_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &status.indent.label, &[]);
    let ind_w = ind_line.width();
    let ind_x = right_cursor - ind_w;
    let ind_baseline = ind_line.baseline();
    for cluster in ind_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            status_font_size,
            &cluster,
            ind_x,
            bar_y + text_baseline_offset + ind_baseline,
            muted,
            glyph_instances,
        );
    }
    layout.indent_rect = Rect {
        x: ind_x - 4.0 * stroke,
        y: bar_y,
        width: ind_w + 8.0 * stroke,
        height: bar_height,
    };

    layout
}
