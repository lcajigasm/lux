//! Wheel and trackpad movement while a floating panel is open.
//!
//! The event goes to that panel. The editor viewport is left where it was.

use std::cell::Cell;

use lux_platform::ScrollDelta;

use crate::app::App;

thread_local! {
    static REMAINDER: Cell<(u8, f32)> = const { Cell::new((0, 0.0)) };
}

pub(crate) fn scroll_open_overlay(app: &mut App, delta: ScrollDelta) -> bool {
    if app.ui.dialogs.conflict_prompt.is_some() || app.ui.dialogs.about.is_some() {
        return true;
    }
    if crate::settings::scroll_settings(app, delta) {
        return true;
    }
    if scroll_palette(app, delta) {
        return true;
    }
    if crate::git::branch_picker::scroll_branch_picker(app, delta) {
        return true;
    }
    if crate::language_picker::handle_language_picker_scroll(app, delta) {
        return true;
    }
    if scroll_indexed(app, delta, Which::Build, 2) {
        return true;
    }
    if scroll_indexed(app, delta, Which::Project, 3) {
        return true;
    }
    if scroll_indexed(app, delta, Which::Theme, 4) {
        return true;
    }
    if scroll_indexed(app, delta, Which::Snippets, 5) {
        return true;
    }
    if scroll_indexed(app, delta, Which::Goto, 6) {
        return true;
    }
    if app.ui.search_bar.is_some() {
        let steps = row_delta(9, delta, 32.0 * app.scale_factor.max(1.0));
        for _ in 0..steps.unsigned_abs() {
            if steps > 0 {
                crate::search::find_next(app);
            } else {
                crate::search::find_prev(app);
            }
        }
        return true;
    }
    if app.ui.project_search.is_some() {
        let steps = row_delta(10, delta, 28.0 * app.scale_factor.max(1.0));
        for _ in 0..steps.unsigned_abs() {
            if let Some(state) = app.ui.project_search.as_mut() {
                if steps > 0 {
                    state.select_next();
                } else {
                    state.select_prev();
                }
            }
        }
        app.scheduler.invalidate_all();
        return true;
    }
    if app.ui.lsp.completion_popup.is_some() {
        let steps = row_delta(11, delta, 28.0 * app.scale_factor.max(1.0));
        if let Some(popup) = app.ui.lsp.completion_popup.as_mut() {
            nudge(&mut popup.selected_index, popup.items.len(), steps);
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

#[derive(Clone, Copy)]
enum Which {
    Build,
    Project,
    Theme,
    Snippets,
    Goto,
}

fn scroll_indexed(app: &mut App, delta: ScrollDelta, which: Which, slot: u8) -> bool {
    let open = match which {
        Which::Build => app.ui.build.picker.is_some(),
        Which::Project => app.ui.project.picker.is_some(),
        Which::Theme => app.ui.theme_picker.is_some(),
        Which::Snippets => app.ui.snippets.picker.is_some(),
        Which::Goto => app.ui.goto_anything.is_some(),
    };
    if !open {
        return false;
    }
    let steps = row_delta(slot, delta, 36.0 * app.scale_factor.max(1.0));
    match which {
        Which::Build => {
            if let Some(picker) = app.ui.build.picker.as_mut() {
                let len = picker.filtered().len();
                nudge(&mut picker.selected, len, steps);
            }
        }
        Which::Project => {
            if let Some(picker) = app.ui.project.picker.as_mut() {
                let len = picker.filtered().len();
                nudge(&mut picker.selected, len, steps);
            }
        }
        Which::Theme => {
            if let Some(picker) = app.ui.theme_picker.as_mut() {
                let len = picker.filtered().len();
                nudge(&mut picker.selected_index, len, steps);
            }
        }
        Which::Snippets => {
            if let Some(picker) = app.ui.snippets.picker.as_mut() {
                let len = picker.filtered().len();
                nudge(&mut picker.selected_index, len, steps);
            }
        }
        Which::Goto => {
            if let Some(goto) = app.ui.goto_anything.as_mut() {
                let len = goto.items.len();
                nudge(&mut goto.selected_index, len, steps);
            }
        }
    }
    app.scheduler.invalidate_all();
    true
}

fn scroll_palette(app: &mut App, delta: ScrollDelta) -> bool {
    if app.ui.command_palette.is_none() {
        return false;
    }
    let steps = row_delta(1, delta, 36.0 * app.scale_factor.max(1.0));
    if let Some(palette) = app.ui.command_palette.as_mut() {
        let mut context = lux_ui::command_palette::Context::new();
        context.set("editorFocus", true);
        let len = palette.results(&context).len();
        nudge(&mut palette.selected_index, len, steps);
    }
    app.scheduler.invalidate_all();
    true
}

fn nudge(index: &mut usize, len: usize, steps: isize) {
    if len == 0 || steps == 0 {
        return;
    }
    *index = index.saturating_add_signed(steps).min(len - 1);
}

fn row_delta(slot: u8, delta: ScrollDelta, row_height: f32) -> isize {
    match delta {
        ScrollDelta::Lines { y, .. } => (-y).round() as isize,
        ScrollDelta::Pixels(position) => REMAINDER.with(|cell| {
            let (prev, mut remainder) = cell.get();
            if prev != slot {
                remainder = 0.0;
            }
            let row_height = row_height.max(1.0);
            remainder += -(position.y as f32);
            let steps = (remainder / row_height).trunc() as isize;
            remainder -= steps as f32 * row_height;
            cell.set((slot, remainder));
            steps
        }),
    }
}
