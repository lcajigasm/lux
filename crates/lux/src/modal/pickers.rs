//! Hit-testing and mouse interactions for all item/choice selector pickers (ST-FIX-01).

use lux_platform::PlatformContext;

use super::{ModalGeometry, ModalMetrics};
use crate::app::App;

// 1. Language Picker
pub(super) fn click_language_picker(app: &mut App, mx: f32, my: f32) -> bool {
    crate::language_picker::handle_language_picker_click(app, mx, my)
}

pub(super) fn hover_language_picker(app: &mut App, mx: f32, my: f32) -> bool {
    crate::language_picker::handle_language_picker_hover(app, mx, my)
}

// 2. Theme Picker
fn geom_theme_picker(m: ModalMetrics, total_items: usize, selected: usize) -> ModalGeometry {
    use crate::render::panel::{self, THEME_VISIBLE};
    let box_ = panel::selector_box(m.vp_width, m.tab_bar_height, m.scale);
    let visible = total_items.min(THEME_VISIBLE);
    let start = if selected >= visible {
        selected + 1 - visible
    } else {
        0
    };
    ModalGeometry {
        left: box_.left,
        top: box_.top,
        width: box_.width,
        total_height: panel::panel_height(box_.input_height, box_.row_height, visible, m.scale),
        rows_top: panel::rows_top(box_.top, box_.input_height, m.scale),
        row_height: box_.row_height,
        visible_count: visible,
        start_idx: start,
    }
}

pub(super) fn click_theme_picker(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let Some(picker) = app.ui.theme_picker.as_ref() else {
        return false;
    };
    let filtered = picker.filtered();
    let geom = geom_theme_picker(m, filtered.len(), picker.selected_index);

    if !geom.contains(mx, my) {
        let orig = picker.original_theme_name;
        app.set_theme(orig);
        app.ui.theme_picker = None;
        crate::focus::focus_editor(app);
        app.scheduler.invalidate_all();
        return true;
    }
    if let Some(idx) = geom.row_index_at(my)
        && let Some(&candidate) = filtered.get(idx)
    {
        app.confirm_theme(candidate);
        app.ui.theme_picker = None;
        crate::focus::focus_editor(app);
        app.scheduler.invalidate_all();
    }
    true
}

pub(super) fn hover_theme_picker(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let (total, sel_idx) = {
        let Some(picker) = app.ui.theme_picker.as_ref() else {
            return false;
        };
        (picker.filtered().len(), picker.selected_index)
    };
    let geom = geom_theme_picker(m, total, sel_idx);

    if geom.contains(mx, my)
        && let Some(idx) = geom.row_index_at(my)
        && idx < total
        && idx != sel_idx
    {
        let candidate = if let Some(picker) = app.ui.theme_picker.as_mut() {
            picker.selected_index = idx;
            picker.current_candidate()
        } else {
            None
        };
        if let Some(cand) = candidate {
            app.preview_theme(cand);
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

// 3. Snippet Picker
fn geom_snippet_picker(m: ModalMetrics, total_items: usize, selected: usize) -> ModalGeometry {
    let width = (m.vp_width * 0.58).clamp(380.0 * m.scale, 620.0 * m.scale);
    let left = (m.vp_width - width) * 0.5;
    let top = m.tab_bar_height + 12.0 * m.scale;
    let input_h = 34.0 * m.scale;
    let row_h = 26.0 * m.scale;
    let visible = total_items.min(9);
    let total_h = input_h + (visible as f32 * row_h) + 4.0 * m.scale;
    let start = if selected >= 9 { selected - 9 + 1 } else { 0 };
    ModalGeometry {
        left,
        top,
        width,
        total_height: total_h,
        rows_top: top + input_h + 2.0 * m.scale,
        row_height: row_h,
        visible_count: visible,
        start_idx: start,
    }
}

pub(super) fn click_snippet_picker(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let Some(picker) = app.ui.snippets.picker.as_ref() else {
        return false;
    };
    let filtered = picker.filtered();
    let geom = geom_snippet_picker(m, filtered.len(), picker.selected_index);

    if !geom.contains(mx, my) {
        app.ui.snippets.picker = None;
        crate::focus::focus_editor(app);
        app.scheduler.invalidate_all();
        return true;
    }
    if let Some(idx) = geom.row_index_at(my)
        && let Some(snip) = filtered.get(idx)
    {
        let tmpl = snip.content.clone();
        app.ui.snippets.picker = None;
        crate::focus::focus_editor(app);
        app.expand_snippet(&tmpl);
        app.scheduler.invalidate_all();
    }
    true
}

pub(super) fn hover_snippet_picker(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let (total, sel_idx) = {
        let Some(picker) = app.ui.snippets.picker.as_ref() else {
            return false;
        };
        (picker.filtered().len(), picker.selected_index)
    };
    let geom = geom_snippet_picker(m, total, sel_idx);

    if geom.contains(mx, my)
        && let Some(idx) = geom.row_index_at(my)
        && idx < total
        && idx != sel_idx
    {
        if let Some(picker) = app.ui.snippets.picker.as_mut() {
            picker.selected_index = idx;
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

// 4. Build Picker
fn geom_build_picker(m: ModalMetrics, total_items: usize, selected: usize) -> ModalGeometry {
    let width = (m.vp_width * 0.52).clamp(360.0 * m.scale, 600.0 * m.scale);
    let left = (m.vp_width - width) * 0.5;
    let top = m.tab_bar_height + 12.0 * m.scale;
    let hdr_h = 24.0 * m.scale;
    let input_h = 32.0 * m.scale;
    let row_h = 28.0 * m.scale;
    let visible = total_items.min(9);
    let total_h = hdr_h + input_h + (visible as f32 * row_h) + 6.0 * m.scale;
    let start = if selected >= visible {
        selected + 1 - visible
    } else {
        0
    };
    ModalGeometry {
        left,
        top,
        width,
        total_height: total_h,
        rows_top: top + hdr_h + input_h + 4.0 * m.scale,
        row_height: row_h,
        visible_count: visible,
        start_idx: start,
    }
}

pub(super) fn click_build_picker(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mx: f32,
    my: f32,
) -> bool {
    let m = ModalMetrics::from_app(app);
    let Some(picker) = app.ui.build.picker.as_ref() else {
        return false;
    };
    let filtered = picker.filtered();
    let geom = geom_build_picker(m, filtered.len(), picker.selected);

    if !geom.contains(mx, my) {
        app.ui.build.picker = None;
        crate::focus::focus_editor(app);
        app.scheduler.invalidate_all();
        return true;
    }
    if let Some(idx) = geom.row_index_at(my)
        && let Some(variant) = filtered.get(idx).copied().cloned()
    {
        app.ui.build.picker = None;
        crate::focus::focus_editor(app);
        app.execute_build_variant(variant, Some(ctx));
        app.scheduler.invalidate_all();
    }
    true
}

pub(super) fn hover_build_picker(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let (total, sel_idx) = {
        let Some(picker) = app.ui.build.picker.as_ref() else {
            return false;
        };
        (picker.filtered().len(), picker.selected)
    };
    let geom = geom_build_picker(m, total, sel_idx);

    if geom.contains(mx, my)
        && let Some(idx) = geom.row_index_at(my)
        && idx < total
        && idx != sel_idx
    {
        if let Some(picker) = app.ui.build.picker.as_mut() {
            picker.selected = idx;
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

// 5. Project Picker
fn geom_project_picker(m: ModalMetrics, total_items: usize, selected: usize) -> ModalGeometry {
    let width = (m.vp_width * 0.52).clamp(380.0 * m.scale, 640.0 * m.scale);
    let left = (m.vp_width - width) * 0.5;
    let top = m.tab_bar_height + 12.0 * m.scale;
    let hdr_h = 24.0 * m.scale;
    let input_h = 32.0 * m.scale;
    let row_h = 30.0 * m.scale;
    let visible = total_items.min(9);
    let total_h = hdr_h + input_h + (visible as f32 * row_h) + 6.0 * m.scale;
    let start = if selected >= 9 { selected - 9 + 1 } else { 0 };
    ModalGeometry {
        left,
        top,
        width,
        total_height: total_h,
        rows_top: top + hdr_h + input_h + 2.0 * m.scale,
        row_height: row_h,
        visible_count: visible,
        start_idx: start,
    }
}

pub(super) fn click_project_picker(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mx: f32,
    my: f32,
) -> bool {
    let m = ModalMetrics::from_app(app);
    let Some(picker) = app.ui.project.picker.as_ref() else {
        return false;
    };
    let filtered = picker.filtered();
    let geom = geom_project_picker(m, filtered.len(), picker.selected);

    if !geom.contains(mx, my) {
        app.ui.project.picker = None;
        crate::focus::focus_editor(app);
        app.scheduler.invalidate_all();
        return true;
    }
    if let Some(idx) = geom.row_index_at(my)
        && let Some(path) = filtered.get(idx).copied().cloned()
    {
        app.ui.project.picker = None;
        crate::focus::focus_editor(app);
        app.open_project_file(&path, Some(ctx));
        app.scheduler.invalidate_all();
    }
    true
}

pub(super) fn hover_project_picker(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let (total, sel_idx) = {
        let Some(picker) = app.ui.project.picker.as_ref() else {
            return false;
        };
        (picker.filtered().len(), picker.selected)
    };
    let geom = geom_project_picker(m, total, sel_idx);

    if geom.contains(mx, my)
        && let Some(idx) = geom.row_index_at(my)
        && idx < total
        && idx != sel_idx
    {
        if let Some(picker) = app.ui.project.picker.as_mut() {
            picker.selected = idx;
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}
