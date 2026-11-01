//! Hit-testing and mouse interactions for Command Palette and Goto Anything (ST-FIX-01).

use lux_platform::PlatformContext;

use super::{ModalGeometry, ModalMetrics};
use crate::app::App;

fn geom_palette(m: ModalMetrics, total_results: usize, selected: usize) -> ModalGeometry {
    use crate::render::panel::{self, SELECTOR_VISIBLE};
    let box_ = panel::selector_box(m.vp_width, m.tab_bar_height, m.scale);
    let visible = total_results.min(SELECTOR_VISIBLE);
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

pub(super) fn click_command_palette(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mx: f32,
    my: f32,
) -> bool {
    let m = ModalMetrics::from_app(app);
    let has_sel = !app.cursors.primary().is_empty();
    let Some(palette) = app.ui.command_palette.as_ref() else {
        return false;
    };
    let mut context = lux_ui::command_palette::Context::new();
    context.set("editorFocus", true);
    context.set("hasSelection", has_sel);
    let results = palette.results(&context);
    let geom = geom_palette(m, results.len(), palette.selected_index);

    if !geom.contains(mx, my) {
        app.ui.command_palette = None;
        app.scheduler.invalidate_all();
        return true;
    }
    if let Some(idx) = geom.row_index_at(my)
        && idx < results.len()
    {
        let cmd_id = results[idx].command.id.clone();
        if let Some(p) = app.ui.command_palette.as_mut() {
            p.registry.record_use(&cmd_id);
        }
        app.ui.command_palette = None;
        crate::palette::execute_command(app, ctx, &cmd_id);
        app.scheduler.invalidate_all();
    }
    true
}

pub(super) fn hover_command_palette(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let has_sel = !app.cursors.primary().is_empty();
    let (total, sel_idx) = {
        let Some(palette) = app.ui.command_palette.as_ref() else {
            return false;
        };
        let mut context = lux_ui::command_palette::Context::new();
        context.set("editorFocus", true);
        context.set("hasSelection", has_sel);
        (palette.results(&context).len(), palette.selected_index)
    };
    let geom = geom_palette(m, total, sel_idx);

    if geom.contains(mx, my)
        && let Some(idx) = geom.row_index_at(my)
        && idx < total
        && idx != sel_idx
    {
        if let Some(palette) = app.ui.command_palette.as_mut() {
            palette.selected_index = idx;
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

fn geom_goto(m: ModalMetrics, total_items: usize, selected: usize) -> ModalGeometry {
    use crate::render::panel::{self, SELECTOR_VISIBLE};
    let box_ = panel::selector_box(m.vp_width, m.tab_bar_height, m.scale);
    let visible = total_items.min(SELECTOR_VISIBLE);
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

pub(super) fn click_goto_anything(
    app: &mut App,
    ctx: &dyn PlatformContext,
    mx: f32,
    my: f32,
) -> bool {
    let m = ModalMetrics::from_app(app);
    let Some(goto) = app.ui.goto_anything.as_ref() else {
        return false;
    };
    let geom = geom_goto(m, goto.items.len(), goto.selected_index);

    if !geom.contains(mx, my) {
        if let Some(mut g) = app.ui.goto_anything.take()
            && let Some(snap) = g.snapshot.take()
        {
            if let Some(orig_tab) = snap.active_tab
                && app.documents.active_tab != Some(orig_tab)
            {
                app.switch_to_tab(orig_tab);
            }
            app.cursors = snap.cursors;
            app.viewport.scroll_x = snap.scroll_x;
            app.viewport.scroll_y = snap.scroll_y;
            app.reset_goal_columns();
        }
        app.chrome.focus.escape_to_editor();
        app.scheduler.invalidate_all();
        return true;
    }

    if let Some(idx) = geom.row_index_at(my)
        && let Some(item) = goto.items.get(idx)
    {
        let (target, line, col) = (item.path.clone(), item.line, item.col);
        if let Some(mut g) = app.ui.goto_anything.take()
            && !target.is_empty()
        {
            g.record_open(&target);
            crate::goto::execute_goto(app, ctx, &target, line, col);
        }
        app.chrome.focus.escape_to_editor();
        app.scheduler.invalidate_all();
    }
    true
}

pub(super) fn hover_goto_anything(app: &mut App, mx: f32, my: f32) -> bool {
    let m = ModalMetrics::from_app(app);
    let (total, sel_idx) = {
        let Some(goto) = app.ui.goto_anything.as_ref() else {
            return false;
        };
        (goto.items.len(), goto.selected_index)
    };
    let geom = geom_goto(m, total, sel_idx);

    if geom.contains(mx, my)
        && let Some(idx) = geom.row_index_at(my)
        && idx < total
        && idx != sel_idx
    {
        if let Some(goto) = app.ui.goto_anything.as_mut() {
            goto.selected_index = idx;
        }
        crate::goto::update_live_preview(app);
        app.scheduler.invalidate_all();
        return true;
    }
    false
}
