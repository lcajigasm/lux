//! Settings page. Painted in the overlay pass so editor glyphs stay under it.
//! The frame is the neutral chrome border. Monokai's accent is the caret
//! color; it is not the outline of this page.

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, overlay_text, push_glyph};
use crate::render::panel::selected_fill;
use crate::settings::layout::{
    ControlGeom, SettingsBounds, SettingsLayout, format_number, layout_settings,
};
use crate::settings::{SettingKind, SettingsPanelState};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_settings_panel(
    panel: &SettingsPanelState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    viewport_width: f32,
    viewport_height: f32,
    tab_bar_height: f32,
    status_bar_height: f32,
    scale_factor: f32,
    theme: &lux_ui::Theme,
) {
    let _text = overlay_text();
    let layout = layout_settings(
        panel,
        SettingsBounds {
            window_width: viewport_width,
            window_height: viewport_height,
            tab_bar: tab_bar_height,
            status_bar: status_bar_height,
            scale: scale_factor,
        },
    );
    let scale = layout.scale;
    let page_fill = color(theme.chrome.panel);
    let border = color(theme.chrome.border);
    let foreground = color(theme.foreground);
    let muted = color(theme.muted);
    let well = color(theme.chrome.panel_input);
    let on_track = color(theme.accent);

    quad_instances.push(QuadInstance::solid(
        [layout.page.x, layout.page.y, layout.page.w, layout.page.h],
        page_fill,
    ));
    quad_instances.push(QuadInstance::solid(
        [layout.page.x, layout.page.y, layout.page.w, scale.max(1.0)],
        border,
    ));

    let family = editor_view.family().to_string();
    draw_line(
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        &family,
        17.0 * scale,
        "Settings",
        layout.title_x,
        layout.title_y,
        foreground,
        glyph_instances,
    );
    draw_line(
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        &family,
        12.0 * scale,
        "Changes apply immediately  ·  Esc closes  ·  ← → changes the control",
        layout.hint_x,
        layout.hint_y,
        muted,
        glyph_instances,
    );

    for (index, row) in layout.rows.iter().enumerate() {
        if index == panel.selected_index
            && let Some(selection) = layout.painted_selection(row)
        {
            quad_instances.push(QuadInstance::rounded(
                [selection.x, selection.y, selection.w, selection.h],
                selected_fill(),
                8.0 * scale,
            ));
        }
        if !layout.row_is_visible(row) {
            continue;
        }
        let item = &panel.items[row.index];
        draw_line(
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &family,
            13.5 * scale,
            item.label,
            row.label.x,
            row.label.y,
            foreground,
            glyph_instances,
        );
        draw_line(
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &family,
            11.5 * scale,
            item.description,
            row.description.x,
            row.description.y,
            muted,
            glyph_instances,
        );
        paint_control(
            &item.kind,
            &row.kind,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &family,
            scale,
            foreground,
            muted,
            well,
            border,
            on_track,
            quad_instances,
            glyph_instances,
        );
    }

    if let Some(menu) = &layout.menu {
        paint_menu(
            menu,
            panel,
            &layout,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &family,
            foreground,
            well,
            border,
            quad_instances,
            glyph_instances,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_control(
    kind: &SettingKind,
    geom: &ControlGeom,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    family: &str,
    scale: f32,
    foreground: [f32; 4],
    muted: [f32; 4],
    well: [f32; 4],
    border: [f32; 4],
    on_track: [f32; 4],
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    match (kind, geom) {
        (SettingKind::Boolean { value }, ControlGeom::Toggle { track, knob }) => {
            let fill = if *value { on_track } else { border };
            quads.push(QuadInstance::rounded(
                [track.x, track.y, track.w, track.h],
                fill,
                track.h * 0.5,
            ));
            quads.push(QuadInstance::rounded(
                [knob.x, knob.y, knob.w, knob.h],
                foreground,
                knob.h * 0.5,
            ));
        }
        (
            SettingKind::Number { value, .. },
            ControlGeom::Stepper {
                minus,
                value: well_rect,
                plus,
            },
        ) => {
            paint_button(*minus, well, border, scale, quads);
            paint_button(*plus, well, border, scale, quads);
            centered(
                editor_view,
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                family,
                13.0 * scale,
                &format_number(*value),
                *well_rect,
                foreground,
                glyphs,
            );
            centered(
                editor_view,
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                family,
                16.0 * scale,
                "−",
                *minus,
                foreground,
                glyphs,
            );
            centered(
                editor_view,
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                family,
                16.0 * scale,
                "+",
                *plus,
                foreground,
                glyphs,
            );
        }
        (SettingKind::Enum { value, .. }, ControlGeom::Menu { button }) => {
            paint_button(*button, well, border, scale, quads);
            draw_line(
                editor_view,
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                family,
                13.0 * scale,
                value,
                button.x + 12.0 * scale,
                button.y + (button.h - 13.0 * scale) * 0.5,
                foreground,
                glyphs,
            );
            draw_line(
                editor_view,
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                family,
                11.0 * scale,
                "▾",
                button.right() - 18.0 * scale,
                button.y + (button.h - 11.0 * scale) * 0.5,
                muted,
                glyphs,
            );
        }
        _ => {}
    }
}

fn paint_button(
    rect: crate::settings::layout::Rect,
    fill: [f32; 4],
    border: [f32; 4],
    scale: f32,
    quads: &mut Vec<QuadInstance>,
) {
    let radius = 6.0 * scale;
    quads.push(QuadInstance::rounded(
        [rect.x, rect.y, rect.w, rect.h],
        border,
        radius,
    ));
    let inset = scale.max(1.0);
    quads.push(QuadInstance::rounded(
        [
            rect.x + inset,
            rect.y + inset,
            (rect.w - inset * 2.0).max(1.0),
            (rect.h - inset * 2.0).max(1.0),
        ],
        fill,
        (radius - inset).max(0.0),
    ));
}

#[allow(clippy::too_many_arguments)]
fn paint_menu(
    menu: &crate::settings::layout::MenuGeom,
    panel: &SettingsPanelState,
    layout: &SettingsLayout,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    family: &str,
    foreground: [f32; 4],
    well: [f32; 4],
    border: [f32; 4],
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let Some(SettingKind::Enum { options, .. }) = panel.items.get(menu.item).map(|item| &item.kind)
    else {
        return;
    };
    let scale = layout.scale;
    paint_button(menu.panel, well, border, scale, quads);
    let highlight = panel.menu.map(|state| state.highlight);
    for (index, option) in options.iter().enumerate() {
        let Some(rect) = menu.options.get(index) else {
            continue;
        };
        if highlight == Some(index) {
            quads.push(QuadInstance::rounded(
                [
                    rect.x,
                    rect.y + scale,
                    rect.w,
                    (rect.h - 2.0 * scale).max(1.0),
                ],
                selected_fill(),
                5.0 * scale,
            ));
        }
        draw_line(
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            family,
            13.0 * scale,
            option,
            rect.x + 8.0 * scale,
            rect.y + (rect.h - 13.0 * scale) * 0.5,
            foreground,
            glyphs,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    family: &str,
    font_size: f32,
    text: &str,
    x: f32,
    top: f32,
    ink: [f32; 4],
    glyphs: &mut Vec<GlyphInstance>,
) {
    let style = lux_render::LineStyle {
        family,
        font_size,
        ligatures: false,
    };
    let line = editor_view.text_layout_mut().layout_line(&style, text, &[]);
    let origin_y = top + line.baseline();
    for cluster in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            x,
            origin_y,
            ink,
            glyphs,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn centered(
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    family: &str,
    font_size: f32,
    text: &str,
    rect: crate::settings::layout::Rect,
    ink: [f32; 4],
    glyphs: &mut Vec<GlyphInstance>,
) {
    let style = lux_render::LineStyle {
        family,
        font_size,
        ligatures: false,
    };
    let line = editor_view.text_layout_mut().layout_line(&style, text, &[]);
    let x = rect.x + (rect.w - line.width()).max(0.0) * 0.5;
    let top = rect.y + (rect.h - font_size).max(0.0) * 0.5;
    let origin_y = top + line.baseline();
    for cluster in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            x,
            origin_y,
            ink,
            glyphs,
        );
    }
}
