//! Rendering logic for the floating Command Palette overlay (G08).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::palette::CommandPaletteState;
use crate::render::paint::{
    color, faded, fuzzy_spans, overlay_fade, overlay_text, push_bold_glyph, push_glyph, span_covers,
};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::command_palette::Context;
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_command_palette(
    palette: &mut CommandPaletteState,
    viewport_width: f32,
    tab_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
    _font_size: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    has_selection: bool,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let _overlay = overlay_text();
    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let _fade = overlay_fade(palette.motion.open_fade(now, reduced));

    let mut context = Context::new();
    context.set("editorFocus", true);
    context.set("hasSelection", has_selection);
    let visual = palette.motion.row(palette.selected_index, now, reduced);
    let results = palette.results(&context);
    let visible_count = results.len().min(super::panel::SELECTOR_VISIBLE);
    let open_offset = palette
        .motion
        .open_offset(now, reduced, -8.0 * scale_factor);
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

    let palette_font_size = 13.0 * scale_factor;
    let (font, family) = crate::font::ui_font(font, editor_view.family());
    let style = lux_render::LineStyle {
        family: &family,
        font_size: palette_font_size,
        ligatures: false,
    };

    let foreground =
        super::panel::readable_ink(color(theme.foreground), super::panel::card_fill(theme));
    let input_baseline = (chrome.input_height - palette_font_size) * 0.5;
    let text_x = chrome.left + 22.0 * scale_factor;
    let text_y = chrome.top + stroke + input_baseline;
    let (display_text, text_color) = if palette.query.is_empty() {
        ("Execute a command…", super::panel::placeholder_ink(theme))
    } else {
        (palette.query.as_str(), foreground)
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
            palette_font_size,
            &cluster,
            text_x,
            text_y + input_line.baseline(),
            text_color,
            glyph_instances,
        );
    }
    if !palette.query.is_empty() {
        quad_instances.push(QuadInstance::solid(
            [
                text_x + input_line.width() + 2.0 * stroke,
                chrome.top + stroke + (chrome.input_height - palette_font_size) * 0.5,
                2.0 * stroke,
                palette_font_size,
            ],
            faded(color(theme.accent)),
        ));
    }

    let row_baseline = (chrome.row_height - palette_font_size) * 0.5;
    let start_idx = if palette.selected_index >= visible_count {
        palette.selected_index + 1 - visible_count
    } else {
        0
    };
    super::panel::push_moving_row(
        quad_instances,
        chrome.left,
        chrome.rows_top,
        chrome.width,
        chrome.row_height,
        visual,
        start_idx,
        visible_count,
        super::panel::selected_fill(theme),
        color(theme.accent),
        scale_factor,
    );

    for (i, entry) in results
        .iter()
        .skip(start_idx)
        .take(visible_count)
        .enumerate()
    {
        let row_y = chrome.rows_top + i as f32 * chrome.row_height;
        let actual_idx = start_idx + i;
        let is_selected = actual_idx == palette.selected_index;
        let icon_size = 16.0 * scale_factor;
        let icon_x = chrome.left + 16.0 * scale_factor;
        let hole = if is_selected {
            super::panel::selected_fill(theme)
        } else {
            super::panel::card_fill(theme)
        };
        super::icons::push_icon(
            super::icons::icon_for_category(&entry.command.category),
            quad_instances,
            icon_x,
            row_y + (chrome.row_height - icon_size) * 0.5,
            icon_size,
            super::panel::readable_ink(color(theme.foreground), hole),
            hole,
        );

        let title_line =
            editor_view
                .text_layout_mut()
                .layout_line(&style, &entry.command.title, &[]);
        let title_x = icon_x + icon_size + 10.0 * scale_factor;
        let hits = fuzzy_spans(&entry.command.title, &palette.query);
        for cluster in title_line.clusters() {
            let hit = span_covers(&hits, &cluster.byte_range);
            let row_fill = if is_selected {
                super::panel::selected_fill(theme)
            } else {
                super::panel::card_fill(theme)
            };
            let glyph_color = if hit {
                super::panel::hit_ink(is_selected, theme)
            } else {
                super::panel::readable_ink(color(theme.foreground), row_fill)
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
                palette_font_size,
                &cluster,
                title_x,
                row_y + row_baseline + title_line.baseline(),
                glyph_color,
                glyph_instances,
            );
        }

        let Some(shortcut) = palette.shortcuts.get(&entry.command.id) else {
            continue;
        };
        let shortcut_line = editor_view
            .text_layout_mut()
            .layout_line(&style, shortcut, &[]);
        let shortcut_x = chrome.left + chrome.width - shortcut_line.width() - 18.0 * scale_factor;
        for cluster in shortcut_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                palette_font_size,
                &cluster,
                shortcut_x,
                row_y + row_baseline + shortcut_line.baseline(),
                super::panel::shortcut_ink(theme),
                glyph_instances,
            );
        }
    }
    crate::overlay_motion::note_if_animating(&palette.motion, now);
}
