//! Hit-testing and mouse interactions for all item/choice selector pickers (ST-FIX-01).

use lux_platform::PlatformContext;

use super::{ModalGeometry, ModalMetrics};
use crate::app::App;

// 1. Language Picker
pub(super) fn click_branch_picker(app: &mut App, mx: f32, my: f32) -> bool {
    crate::git::branch_picker::handle_branch_picker_click(app, mx, my)
}

pub(super) fn click_language_picker(app: &mut App, mx: f32, my: f32) -> bool {
    crate::language_picker::handle_language_picker_click(app, mx, my)
}

pub(super) fn hover_language_picker(app: &mut App, mx: f32, my: f32) -> bool {
    crate::language_picker::handle_language_picker_hover(app, mx, my)
}

// 2. Theme Picker
fn geom_theme_picker(m: ModalMetrics, total_items: usize, selected: usize) -> ModalGeometry {
    super::list_geometry(m, total_items, selected)
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
    super::list_geometry(m, total_items, selected)
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
    super::list_geometry(m, total_items, selected)
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
    super::list_geometry(m, total_items, selected)
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
