//! Mouse hits on the docked find bar.

use super::{SearchField, find_all, select_match};

pub(crate) fn handle_search_click(app: &mut crate::app::App, mouse_x: f32, mouse_y: f32) -> bool {
    let (layout, replace_mode) = {
        let Some(search) = app.ui.search_bar.as_ref() else {
            return false;
        };
        (search.layout.clone(), search.replace_mode)
    };

    if !rect_contains(layout.bar_rect, mouse_x, mouse_y) {
        return false;
    }

    if rect_contains(layout.close_btn_rect, mouse_x, mouse_y) {
        super::toggles::close(app);
        return true;
    }

    if rect_contains(layout.find_input_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.focused_field = SearchField::Find;
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if replace_mode && rect_contains(layout.replace_input_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.focused_field = SearchField::Replace;
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.prev_btn_rect, mouse_x, mouse_y) {
        if let Some(sel) = app.ui.search_bar.as_mut().and_then(|s| s.previous_match()) {
            select_match(app, sel);
        }
        return true;
    }

    if rect_contains(layout.next_btn_rect, mouse_x, mouse_y) {
        if let Some(sel) = app.ui.search_bar.as_mut().and_then(|s| s.next_match()) {
            select_match(app, sel);
        }
        return true;
    }

    if rect_contains(layout.case_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.toggle_case(&app.buffer);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.word_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.toggle_word(&app.buffer);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.regex_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.toggle_regex(&app.buffer);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.selection_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            let active_range =
                (!app.cursors.primary().is_empty()).then(|| app.cursors.primary().char_range());
            s.toggle_in_selection(&app.buffer, active_range);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if replace_mode && rect_contains(layout.preserve_case_btn_rect, mouse_x, mouse_y) {
        if let Some(s) = app.ui.search_bar.as_mut() {
            s.toggle_preserve_case(&app.buffer);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    if rect_contains(layout.find_all_btn_rect, mouse_x, mouse_y) {
        find_all(app);
        return true;
    }

    if replace_mode && rect_contains(layout.replace_btn_rect, mouse_x, mouse_y) {
        let (replaced_tx, next_sel) = {
            let Some(s) = app.ui.search_bar.as_mut() else {
                return true;
            };
            let tx = s.replace_current(&mut app.buffer);
            let sel = s.search_bar.current();
            (tx, sel)
        };
        if let Some(tx) = replaced_tx {
            app.sync_syntax_incremental(&tx.edits);
            app.history.record(tx);
            if let Some(sel) = next_sel {
                select_match(app, sel);
            }
        }
        return true;
    }

    if replace_mode && rect_contains(layout.replace_all_btn_rect, mouse_x, mouse_y) {
        let replaced_tx = app
            .ui
            .search_bar
            .as_mut()
            .and_then(|s| s.replace_all(&mut app.buffer));
        if let Some(tx) = replaced_tx {
            app.sync_syntax_incremental(&tx.edits);
            app.history.record(tx);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    true
}

fn rect_contains(rect: [f32; 4], x: f32, y: f32) -> bool {
    let [rx, ry, rw, rh] = rect;
    x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
}
