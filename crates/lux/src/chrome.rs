//! Mouse click handling on desktop window chrome (tabs, sidebar, minimap, status bar).

use crate::app::{App, TAB_CLOSE_BTN_WIDTH};
use lux_platform::{MouseButton, PlatformContext};

pub(crate) fn handle_mouse_click_on_chrome(
    app: &mut App,
    ctx: &dyn PlatformContext,
    button: MouseButton,
) -> bool {
    let mouse_y = app.mouse.position.y as f32;
    let mouse_x = app.mouse.position.x as f32;

    if button == MouseButton::Left && crate::plugin::handle_plugin_click(app, mouse_x, mouse_y) {
        return true;
    }

    if button == MouseButton::Left
        && app.ui.language_picker.is_some()
        && crate::language_picker::handle_language_picker_click(app, mouse_x, mouse_y)
    {
        return true;
    }

    if button == MouseButton::Left && app.ui.settings.panel.is_some() {
        let mut settings = std::mem::take(&mut app.ui.settings);
        let handled = settings.handle_click(app, mouse_x, mouse_y);
        app.ui.settings = settings;
        if handled {
            return true;
        }
    }

    if let Some(menu) = app.documents.tab_context_menu.take() {
        if button == MouseButton::Left
            && let Some(item) = menu.item_at(mouse_x, mouse_y, app.scale_factor)
        {
            match item {
                crate::document::tab_ops::TabContextMenuItem::Close => {
                    app.close_tab(ctx, menu.tab_id);
                }
                crate::document::tab_ops::TabContextMenuItem::CloseOthers => {
                    app.close_other_tabs(ctx, menu.tab_id);
                }
                crate::document::tab_ops::TabContextMenuItem::CloseToRight => {
                    app.close_tabs_to_right(ctx, menu.tab_id);
                }
                crate::document::tab_ops::TabContextMenuItem::CloseSaved => {
                    app.close_saved_tabs(ctx);
                }
                crate::document::tab_ops::TabContextMenuItem::TogglePin => {
                    app.toggle_pin_tab(menu.tab_id);
                }
                crate::document::tab_ops::TabContextMenuItem::CopyFilePath => {
                    app.copy_tab_file_path(Some(ctx.clipboard()), menu.tab_id);
                }
                crate::document::tab_ops::TabContextMenuItem::RevealInFinder => {
                    app.reveal_tab_in_finder(menu.tab_id);
                }
            }
            app.scheduler.invalidate_all();
            return true;
        }
        app.scheduler.invalidate_all();
    }

    if mouse_y < app.tab_bar_height() {
        let sidebar_w = app.sidebar_width();
        let tab_mouse_x = mouse_x - sidebar_w;
        if tab_mouse_x >= 0.0 {
            let tab_w = app.tab_width();
            let tabs = app.documents.tabs.tabs().to_vec();
            let tab_idx = (tab_mouse_x / tab_w) as usize;
            if let Some(tab) = tabs.get(tab_idx) {
                let tab_x = sidebar_w + tab_idx as f32 * tab_w;
                let close_w = TAB_CLOSE_BTN_WIDTH * app.scale_factor;
                let on_close = mouse_x >= tab_x + tab_w - close_w;
                match button {
                    MouseButton::Left => {
                        if on_close {
                            app.close_tab(ctx, tab.id);
                        } else {
                            app.switch_to_tab(tab.id);
                        }
                    }
                    MouseButton::Middle => {
                        app.close_tab(ctx, tab.id);
                    }
                    MouseButton::Right => {
                        app.documents.tab_context_menu =
                            Some(crate::document::tab_ops::TabContextMenu::new(
                                tab.id,
                                tab.pinned,
                                mouse_x,
                                mouse_y,
                                app.scale_factor,
                            ));
                        app.scheduler.invalidate_all();
                    }
                    _ => {}
                }
            }
        }
        return true;
    }

    if button == MouseButton::Left
        && crate::conflict::handle_conflict_click(app, ctx, mouse_x, mouse_y)
    {
        return true;
    }

    if button == MouseButton::Left && crate::git::handle_git_click(app, ctx, mouse_x, mouse_y) {
        return true;
    }

    let sidebar_w = app.sidebar_width();
    if sidebar_w > 0.0 && mouse_x < sidebar_w && mouse_y >= app.tab_bar_height() {
        return crate::sidebar::handle_sidebar_click(app, ctx, mouse_x, mouse_y, button);
    }

    // Gutter click: toggle code fold if clicking fold indicator / line number, or open hunk popup if clicking a changed line
    if button == MouseButton::Left && mouse_y >= app.tab_bar_height() {
        let digits = app.buffer.len_lines().max(1).to_string().len();
        let gutter_w = app.editor_view.reference_width(&"0".repeat(digits));
        let gutter_end = sidebar_w + gutter_w + 16.0 * app.scale_factor;
        if mouse_x >= sidebar_w && mouse_x < gutter_end {
            let line_h = crate::app::LINE_HEIGHT * app.scale_factor;
            let visual_line =
                ((mouse_y - app.tab_bar_height() + app.viewport.scroll_y) / line_h) as usize;
            let total_lines = app.buffer.len_lines();
            let buffer_line = app
                .fold_state
                .visual_to_buffer_line(visual_line, total_lines);

            if app.fold_state.is_line_foldable(buffer_line)
                || app.fold_state.is_line_folded(buffer_line)
            {
                app.toggle_fold_at_line(buffer_line);
                app.scheduler.invalidate_all();
                return true;
            }

            let line_idx = (buffer_line as u32) + 1;
            if let Some(hunk) = app.ui.git.hunks.iter().find(|h| {
                if h.after_len == 0 {
                    h.anchor_line() == line_idx
                } else {
                    line_idx >= h.after_start && line_idx < h.after_start + h.after_len
                }
            }) {
                app.ui.git.active_popup = Some(crate::git::GutterHunkPopup {
                    hunk: *hunk,
                    line_index: line_idx,
                    rect: [0.0; 4],
                    stage_btn: [0.0; 4],
                    revert_btn: [0.0; 4],
                    close_btn: [0.0; 4],
                });
                app.scheduler.invalidate_all();
                return true;
            }
        }
    }

    // Minimap click hit-testing
    if button == MouseButton::Left && crate::minimap::handle_minimap_click(app, mouse_x, mouse_y) {
        return true;
    }

    let search_bar_h = app.search_bar_height();
    let search_bar_top = app.tab_bar_height() + app.viewport.height;
    let status_bar_top = search_bar_top + search_bar_h;

    if search_bar_h > 0.0 && mouse_y >= search_bar_top && mouse_y < status_bar_top {
        return crate::search::handle_search_click(app, mouse_x, mouse_y);
    }

    if button == MouseButton::Left && crate::term::input::handle_term_click(app, mouse_x, mouse_y) {
        return true;
    }

    if mouse_y >= status_bar_top && button == MouseButton::Left {
        handle_status_bar_click(app, mouse_x, mouse_y);
        return true;
    }

    if app.ui.split.is_active() && button == MouseButton::Left {
        let sidebar_w = app.sidebar_width();
        let editor_top = app.tab_bar_height();
        let primary_rect = [
            sidebar_w,
            editor_top,
            app.viewport.width,
            app.viewport.height,
        ];
        if let Some(div) = crate::split::hit_test_divider(
            &app.ui.split,
            primary_rect,
            app.scale_factor,
            mouse_x,
            mouse_y,
        ) {
            app.ui.split.dragging_divider = Some(div);
            return true;
        }
        if let Some(pane_idx) = crate::split::hit_test_pane(
            &app.ui.split,
            primary_rect,
            app.scale_factor,
            mouse_x,
            mouse_y,
        ) && pane_idx != app.ui.split.active_pane
        {
            app.focus_group(pane_idx);
            return true;
        }
    }

    false
}

pub(crate) fn handle_status_bar_click(app: &mut App, mouse_x: f32, mouse_y: f32) {
    let layout = app.ui.status_bar.layout.clone();
    if layout.eol_rect.contains(mouse_x, mouse_y) {
        crate::status_bar::toggle_eol(&mut app.buffer);
        app.scheduler.invalidate_all();
    } else if layout.indent_rect.contains(mouse_x, mouse_y) {
        crate::status_bar::toggle_indent(&mut app.buffer);
        app.scheduler.invalidate_all();
    } else if layout.encoding_rect.contains(mouse_x, mouse_y) {
        crate::status_bar::toggle_encoding(&mut app.buffer);
        app.scheduler.invalidate_all();
    } else if layout.cursor_rect.contains(mouse_x, mouse_y) {
        app.toggle_command_palette();
    } else if layout.language_rect.contains(mouse_x, mouse_y) {
        app.open_language_picker();
    }
}
