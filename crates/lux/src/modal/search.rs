//! Hit-testing and mouse interactions for Project Search & Replace overlay (ST-FIX-01).

use lux_platform::PlatformContext;

use super::{ModalGeometry, ModalMetrics};
use crate::app::App;

fn geom_project_search(
    m: ModalMetrics,
    state: &crate::project_search::ProjectSearchState,
) -> ModalGeometry {
    let width = (640.0 * m.scale).min(m.vp_width - 32.0 * m.scale);
    let left = (m.vp_width - width) * 0.5;
    let top = m.tab_bar_height + 24.0 * m.scale;
    let row_h = 24.0 * m.scale;
    let hdr_h = if state.is_replace {
        row_h * 4.0
    } else {
        row_h * 2.8
    };
    let visible = state.matches.len().min(10);
    let total_h = hdr_h + (visible as f32 * row_h) + 16.0 * m.scale;
    let start = state.selected_index.saturating_sub(9);
    ModalGeometry {
        left,
        top,
        width,
        total_height: total_h,
        rows_top: top + hdr_h,
        row_height: row_h,
        visible_count: visible,
        start_idx: start,
    }
}

pub(super) fn click_project_search(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mx: f32,
    my: f32,
) -> bool {
    let m = ModalMetrics::from_app(app);
    let Some(state) = app.ui.project_search.as_ref() else {
        return false;
    };
    let geom = geom_project_search(m, state);

    if !geom.contains(mx, my) {
        app.ui.project_search = None;
        crate::focus::focus_editor(app);
        app.scheduler.invalidate_all();
        return true;
    }

    let s = m.scale;
    let input_h = 24.0 * s;
    let find_y = geom.top + 10.0 * s;
    let where_y = geom.top + 38.0 * s;
    let replace_y = geom.top + 66.0 * s;

    if my >= find_y && my <= find_y + input_h {
        if let Some(st) = app.ui.project_search.as_mut() {
            st.focused_field = crate::project_search::SearchField::Find;
            app.scheduler.invalidate_all();
        }
        return true;
    } else if my >= where_y && my <= where_y + input_h {
        if let Some(st) = app.ui.project_search.as_mut() {
            st.focused_field = crate::project_search::SearchField::Where;
            app.scheduler.invalidate_all();
        }
        return true;
    } else if state.is_replace && my >= replace_y && my <= replace_y + input_h {
        if let Some(st) = app.ui.project_search.as_mut() {
            st.focused_field = crate::project_search::SearchField::Replace;
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if let Some(idx) = geom.row_index_at(my)
        && idx < state.matches.len()
    {
        let match_item = state.matches[idx].clone();
        app.ui.project_search = None;
        crate::find_results::open_match_in_editor(app, Some(ctx), &match_item);
        crate::focus::focus_editor(app);
        app.scheduler.invalidate_all();
    }
    true
}

pub(super) fn hover_project_search(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let (total, sel_idx) = {
        let Some(state) = app.ui.project_search.as_ref() else {
            return false;
        };
        (state.matches.len(), state.selected_index)
    };
    let geom = {
        let Some(state) = app.ui.project_search.as_ref() else {
            return false;
        };
        geom_project_search(m, state)
    };

    if geom.contains(mx, my)
        && let Some(idx) = geom.row_index_at(my)
        && idx < total
        && idx != sel_idx
    {
        if let Some(state) = app.ui.project_search.as_mut() {
            state.selected_index = idx;
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}
