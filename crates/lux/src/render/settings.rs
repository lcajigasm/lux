//! Settings page. Painted in the overlay pass so editor glyphs stay under it.
//! The frame is the neutral chrome border. Monokai's accent is the caret
//! color; it is not the outline of this page.

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, overlay_text, push_glyph};
use crate::render::panel::settings_selection;
use crate::settings::layout::{ControlGeom, SettingsBounds, format_number, layout_settings};
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
    system: lux_ui::SystemScheme,
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
    let row_fill = settings_selection(theme);
    let well = super::panel::distinct_fill(color(theme.chrome.panel_input), page_fill);
    let label_on_page = super::panel::label_ink(theme, page_fill);
    let hint_on_page = super::panel::hint_ink(theme, page_fill);
    let label_on_row = super::panel::label_ink(theme, row_fill);
    let hint_on_row = super::panel::hint_ink(theme, row_fill);
    let label_on_well = super::panel::label_ink(theme, well);
    let hint_on_well = super::panel::hint_ink(theme, well);
    let on_track = color(theme.accent);

    quad_instances.push(QuadInstance::solid(
        [layout.page.x, layout.page.y, layout.page.w, layout.page.h],
        page_fill,
    ));
    quad_instances.push(QuadInstance::solid(
        [layout.page.x, layout.page.y, layout.page.w, scale.max(1.0)],
        border,
    ));

    let (font, family) = crate::font::ui_font(font, editor_view.family());
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
        label_on_page,
        glyph_instances,
    );
    draw_line(
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        &family,
        13.0 * scale,
        "Type to search  ·  Esc closes  ·  ← → changes the control",
        layout.hint_x,
        layout.hint_y,
        hint_on_page,
        glyph_instances,
    );

    quad_instances.push(QuadInstance::solid(
        [
            layout.sidebar.right() - scale.max(1.0),
            layout.sidebar.y,
            scale.max(1.0),
            layout.sidebar.h,
        ],
        border,
    ));
    let names = panel.categories();
    for (index, rect) in layout.categories.iter().enumerate() {
        if index == panel.category && panel.query.is_empty() {
            quad_instances.push(QuadInstance::rounded(
                [rect.x, rect.y, rect.w, rect.h],
                row_fill,
                6.0 * scale,
            ));
        }
        let name = names.get(index).copied().unwrap_or("");
        let pill = index == panel.category && panel.query.is_empty();
        draw_line(
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &family,
            13.0 * scale,
            name,
            rect.x + 10.0 * scale,
            rect.y + (rect.h - 13.0 * scale) * 0.5,
            if pill {
                label_on_row
            } else if index == panel.category {
                label_on_page
            } else {
                hint_on_page
            },
            glyph_instances,
        );
    }
    quad_instances.push(QuadInstance::rounded(
        [
            layout.search.x,
            layout.search.y,
            layout.search.w,
            layout.search.h,
        ],
        well,
        6.0 * scale,
    ));
    let search_text = if panel.query.is_empty() {
        "Search settings"
    } else {
        panel.query.as_str()
    };
    draw_line(
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        &family,
        13.0 * scale,
        search_text,
        layout.search.x + 10.0 * scale,
        layout.search.y + (layout.search.h - 13.0 * scale) * 0.5,
        if panel.query.is_empty() {
            hint_on_well
        } else {
            label_on_well
        },
        glyph_instances,
    );
    if let Some(bar) = layout.scrollbar {
        quad_instances.push(QuadInstance::rounded(
            [bar.x, bar.y, bar.w, bar.h],
            hint_on_page,
            bar.w * 0.5,
        ));
    }

    for row in &layout.rows {
        if row.index == panel.selected_index
            && let Some(selection) = layout.painted_selection(row)
        {
            quad_instances.push(QuadInstance::rounded(
                [selection.x, selection.y, selection.w, selection.h],
                row_fill,
                8.0 * scale,
            ));
        }
        if !layout.row_is_visible(row) {
            continue;
        }
        let item = &panel.items[row.index];
        let selected = row.index == panel.selected_index;
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
            if selected {
                label_on_row
            } else {
                label_on_page
            },
            glyph_instances,
        );
        draw_line(
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &family,
            13.0 * scale,
            item.description,
            row.description.x,
            row.description.y,
            if selected { hint_on_row } else { hint_on_page },
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
            label_on_well,
            hint_on_well,
            well,
            border,
            on_track,
            page_fill,
            if selected { Some(row_fill) } else { None },
            theme,
            quad_instances,
            glyph_instances,
        );
    }

    if let Some(menu) = &layout.menu {
        let covered = glyph_instances.len();
        super::theme_sample::paint_menu(
            menu,
            panel,
            &layout,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            &family,
            label_on_well,
            well,
            border,
            theme,
            system,
            quad_instances,
            glyph_instances,
        );
        super::clip::remove_glyphs_under(
            glyph_instances,
            covered,
            [menu.panel.x, menu.panel.y, menu.panel.w, menu.panel.h],
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
    page: [f32; 4],
    on_selection: Option<[f32; 4]>,
    theme: &lux_ui::Theme,
    quads: &mut Vec<QuadInstance>,
    glyphs: &mut Vec<GlyphInstance>,
) {
    let paint = selected_control_colors(theme, on_selection, foreground, muted, well, border);
    match (kind, geom) {
        (SettingKind::Boolean { value }, ControlGeom::Toggle { track, knob }) => {
            let fill = super::panel::toggle_track(if *value { on_track } else { border }, page);
            let knob_fill = super::panel::toggle_knob(foreground, fill);
            quads.push(QuadInstance::rounded(
                [track.x, track.y, track.w, track.h],
                fill,
                track.h * 0.5,
            ));
            quads.push(QuadInstance::rounded(
                [knob.x, knob.y, knob.w, knob.h],
                knob_fill,
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
            paint_button(*minus, paint.chip, paint.edge, scale, quads);
            paint_button(*plus, paint.chip, paint.edge, scale, quads);
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
                paint.value,
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
                paint.mark,
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
                paint.mark,
                glyphs,
            );
        }
        (SettingKind::Enum { value, .. }, ControlGeom::Menu { button }) => {
            paint_button(*button, paint.chip, paint.edge, scale, quads);
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
                if on_selection.is_some() {
                    paint.mark
                } else {
                    paint.value
                },
                glyphs,
            );
            draw_line(
                editor_view,
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                family,
                13.0 * scale,
                "▾",
                button.right() - 18.0 * scale,
                button.y + (button.h - 13.0 * scale) * 0.5,
                paint.caret,
                glyphs,
            );
        }
        _ => {}
    }
}

struct ControlPaint {
    value: [f32; 4],
    chip: [f32; 4],
    edge: [f32; 4],
    mark: [f32; 4],
    caret: [f32; 4],
}

/// Value ink sits on the selection. The chip fill and its − / + marks
/// sit on each other. An unselected row keeps the page well.
fn selected_control_colors(
    theme: &lux_ui::Theme,
    on_selection: Option<[f32; 4]>,
    foreground: [f32; 4],
    muted: [f32; 4],
    well: [f32; 4],
    border: [f32; 4],
) -> ControlPaint {
    let Some(selection) = on_selection else {
        return ControlPaint {
            value: foreground,
            chip: well,
            edge: border,
            mark: foreground,
            caret: muted,
        };
    };
    let value = super::panel::label_ink(theme, selection);
    let chip = super::panel::selection_chip(selection, value);
    let mark = super::panel::label_ink(theme, chip);
    ControlPaint {
        value,
        chip,
        edge: value,
        mark,
        caret: mark,
    }
}

pub(super) fn paint_button(
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
pub(super) fn draw_line(
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
