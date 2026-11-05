//! Keymap command and chord sequence dispatching.

use lux_core::edit_ops;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{Clipboard, Key, NamedKey, PlatformContext};

use crate::app::App;
use crate::input::motion;

pub(crate) fn dispatch_keymap_or_chord(
    app: &mut App,
    event: &KeyEvent,
    clipboard: Option<&dyn Clipboard>,
    ctx: Option<&dyn PlatformContext>,
) -> bool {
    let Some(chord) = motion::key_event_to_chord(event, &app.modifiers) else {
        return false;
    };

    let platform = if cfg!(target_os = "macos") {
        lux_config::Platform::Mac
    } else if cfg!(target_os = "windows") {
        lux_config::Platform::Windows
    } else {
        lux_config::Platform::Linux
    };

    // If there is a pending chord waiting for completion
    if let Some(first_chord) = app.ui.pending_chord.take() {
        if matches!(&event.key, Key::Named(NamedKey::Escape)) {
            app.ui.announcement = None;
            app.scheduler.invalidate_all();
            return true;
        }

        let seq = lux_config::ChordSequence(vec![first_chord, chord.clone()]);
        if let Some(cmd) = app.keymap.command_for(&seq, platform).map(str::to_owned) {
            app.ui.announcement = None;
            execute_keymap_command(app, &cmd, clipboard, ctx);
            return true;
        }

        app.ui.announcement = None;
        app.scheduler.invalidate_all();

        // Fall back to attempting the second chord as a standalone single-chord command
        let single = lux_config::ChordSequence(vec![chord]);
        if let Some(cmd) = app.keymap.command_for(&single, platform).map(str::to_owned) {
            execute_keymap_command(app, &cmd, clipboard, ctx);
            return true;
        }
        return false;
    }

    // Check if this chord starts a multi-chord sequence in the active keymap
    let is_chord_prefix = app.keymap.bindings.iter().any(|b| {
        let resolved = b.resolve(platform);
        resolved.0.len() > 1 && resolved.0[0] == chord
    });

    if is_chord_prefix {
        let chord_str = chord.to_string();
        app.ui.pending_chord = Some(chord);
        app.ui.announcement = Some(format!(
            "({chord_str}) was pressed. Waiting for second key of chord..."
        ));
        app.scheduler.invalidate_all();
        return true;
    }

    let seq = lux_config::ChordSequence(vec![chord]);
    if let Some(cmd) = app.keymap.command_for(&seq, platform).map(str::to_owned) {
        execute_keymap_command(app, &cmd, clipboard, ctx);
        return true;
    }

    false
}

pub(crate) fn execute_keymap_command(
    app: &mut App,
    cmd: &str,
    clipboard: Option<&dyn Clipboard>,
    ctx: Option<&dyn PlatformContext>,
) {
    if let Some(action) = crate::menu::action_for_command(cmd)
        && !crate::menu::claim_key(action)
    {
        return;
    }
    match cmd {
        "copy" => {
            app.copy_to_clipboard(clipboard);
        }
        "cut" if !app.editing_blocked() && !app.buffer.is_huge_file_mode() => {
            if app.copy_to_clipboard(clipboard) {
                app.replace_selections(app.cursors.clone(), "");
            }
        }
        "paste" if !app.editing_blocked() && !app.buffer.is_huge_file_mode() => {
            if let Some(clipboard) = clipboard {
                match clipboard.read_text() {
                    Ok(text) => app.insert_at_cursor(&text),
                    Err(error) => app.documents.message = Some(error.to_string()),
                }
            }
        }
        "undo" if !app.buffer.is_huge_file_mode() => {
            let restored = app.history.undo(&mut app.buffer);
            if let Some(selections) = restored {
                app.cursors = MultiCursor::from_cursors(selections);
                app.reset_goal_columns();
                app.sync_syntax_full();
                crate::split::clone_view::clamp_cloned_panes_after_undo(app);
            }
        }
        "redo" if !app.buffer.is_huge_file_mode() => {
            let restored = app.history.redo(&mut app.buffer);
            if let Some(selections) = restored {
                app.cursors = MultiCursor::from_cursors(selections);
                app.reset_goal_columns();
                app.sync_syntax_full();
                crate::split::clone_view::clamp_cloned_panes_after_undo(app);
            }
        }
        "select_all" => {
            app.cursors = MultiCursor::new(Selection::new(
                Position::document_start(),
                Position::document_end(&app.buffer),
            ));
            app.reset_goal_columns();
        }
        "duplicate_line" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::duplicate_line);
        }
        "insert_line_after" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::insert_line_after);
        }
        "insert_line_before" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::insert_line_before);
        }
        "use_selection_for_find" => {
            crate::search::use_selection_for_find(app);
        }
        "select_next_occurrence" => {
            app.select_next_occurrence();
        }
        "skip_next_occurrence" => {
            app.skip_current_occurrence();
        }
        "select_all_occurrences" => {
            app.select_all_occurrences();
        }
        "split_selection_into_lines" => {
            app.split_selection_into_lines();
        }
        "add_cursor_above" => {
            app.add_cursor_above();
        }
        "add_cursor_below" => {
            app.add_cursor_below();
        }
        "expand_to_line" => {
            app.expand_to_line();
        }
        "expand_to_scope" => {
            app.expand_to_scope();
        }
        "expand_to_tag" => {
            app.expand_to_tag();
        }
        "swap_line_up" | "move_line_up" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::move_line_up);
        }
        "swap_line_down" | "move_line_down" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::move_line_down);
        }
        "delete_line" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::delete_line);
        }
        "join_lines" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::join_lines);
        }
        "indent" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::indent);
        }
        "dedent" => {
            app.apply_edit_operation(app.cursors.clone(), edit_ops::dedent);
        }
        "markdown_preview" => {
            crate::markdown_preview::toggle(app);
        }
        "toggle_comment" => {
            super::comment::toggle_line_or_block(app);
        }
        "toggle_block_comment" => {
            super::comment::toggle_block(app);
        }
        "fold" => {
            app.fold_at_cursor();
        }
        "unfold" => {
            app.unfold_at_cursor();
        }
        "fold_all" => {
            app.fold_all();
        }
        "unfold_all" => {
            app.unfold_all();
        }
        "fold_level_1" => {
            app.fold_by_level(1);
        }
        "fold_level_2" => {
            app.fold_by_level(2);
        }
        "fold_level_3" => {
            app.fold_by_level(3);
        }
        "fold_level_4" => {
            app.fold_by_level(4);
        }
        "fold_level_5" => {
            app.fold_by_level(5);
        }
        "jump_to_matching_bracket" => {
            app.jump_to_matching_bracket();
        }
        "command_palette" => {
            app.toggle_command_palette();
        }
        "quick_open" => {
            app.open_goto_anything("");
        }
        "goto_line" => {
            app.open_goto_anything(":");
        }
        "goto_symbol" => {
            app.open_goto_anything("@");
        }
        "goto_symbol_in_project" => {
            app.open_goto_project_symbols();
        }
        "goto_word" => {
            app.open_goto_anything("#");
        }
        "toggle_bookmark" => {
            app.toggle_bookmark();
        }
        "next_bookmark" => {
            app.next_bookmark();
        }
        "prev_bookmark" => {
            app.prev_bookmark();
        }
        "clear_bookmarks" => {
            app.clear_bookmarks();
        }
        "select_all_bookmarks" => {
            app.select_all_bookmarks();
        }
        "find" => {
            crate::search::open_find(app, false);
        }
        "replace" | "find_replace" => {
            crate::search::open_find(app, true);
        }
        "find_next" => {
            crate::search::find_next(app);
        }
        "find_prev" => {
            crate::search::find_prev(app);
        }
        "find_all" => {
            crate::search::find_all(app);
        }
        "find_in_files" => {
            crate::project_search::open_project_find(app);
        }
        "next_result" => {
            if app.is_find_results_active() || !app.next_build_error(None) {
                crate::find_results::next_result(app, None);
            }
        }
        "prev_result" => {
            if app.is_find_results_active() || !app.prev_build_error(None) {
                crate::find_results::prev_result(app, None);
            }
        }
        "build" => {
            app.build(None);
        }
        "build_with" => {
            app.build_with();
        }
        "cancel_build" => {
            app.cancel_build();
        }
        "toggle_sidebar" => {
            app.toggle_sidebar();
        }
        "toggle_distraction_free" => {
            app.toggle_distraction_free();
        }
        "trim_trailing_whitespace" => {
            app.trim_trailing_whitespace();
        }
        "ensure_final_newline" => {
            app.ensure_final_newline();
        }
        "set_layout_single" => {
            app.set_layout(crate::split::LayoutPreset::Single);
        }
        "set_layout_columns_2" => {
            app.set_layout(crate::split::LayoutPreset::Columns2);
        }
        "set_layout_columns_3" => {
            app.set_layout(crate::split::LayoutPreset::Columns3);
        }
        "set_layout_columns_4" => {
            app.set_layout(crate::split::LayoutPreset::Columns4);
        }
        "set_layout_rows_2" => {
            app.set_layout(crate::split::LayoutPreset::Rows2);
        }
        "set_layout_rows_3" => {
            app.set_layout(crate::split::LayoutPreset::Rows3);
        }
        "set_layout_grid_2x2" | "set_layout_grid_4" => {
            app.set_layout(crate::split::LayoutPreset::Grid2x2);
        }
        "focus_group_1" => {
            app.focus_group(0);
        }
        "focus_group_2" => {
            app.focus_group(1);
        }
        "focus_group_3" => {
            app.focus_group(2);
        }
        "focus_group_4" => {
            app.focus_group(3);
        }
        "move_to_group_1" => {
            app.move_to_group(0);
        }
        "move_to_group_2" => {
            app.move_to_group(1);
        }
        "move_to_group_3" => {
            app.move_to_group(2);
        }
        "move_to_group_4" => {
            app.move_to_group(3);
        }
        "clone_view_to_group" => {
            app.clone_view_to_group(None);
        }
        "new_view_into_file" => {
            app.new_view_into_file();
        }
        "quick_switch_project" => {
            app.quick_switch_project();
        }
        "close_project" => {
            app.close_project();
        }
        "color_theme" | "select_color_theme" | "select_theme" => {
            app.open_theme_picker();
        }
        "insert_snippet" => {
            crate::snippets::dispatch::open_snippet_picker(app);
        }
        "next_field" | "next_tabstop" => {
            if let Some(session) = &mut app.snippet_session {
                if let Some(sel) = session.next(&app.buffer) {
                    app.cursors = MultiCursor::new(sel);
                    app.reset_goal_columns();
                } else {
                    app.snippet_session = None;
                }
            }
        }
        "prev_field" | "prev_tabstop" => {
            if let Some(session) = &mut app.snippet_session
                && let Some(sel) = session.prev(&app.buffer)
            {
                app.cursors = MultiCursor::new(sel);
                app.reset_goal_columns();
            }
        }
        "toggle_record_macro" | "macro.toggle_record" => {
            crate::macros::dispatch::toggle_record_macro(app);
            return;
        }
        "playback_macro" | "macro.playback" => {
            crate::macros::dispatch::playback_macro(app);
            return;
        }
        "save_all" | "file.save_all" => {
            if let Some(ctx) = ctx {
                app.save_all(ctx);
            }
        }
        "reopen_closed_tab" | "file.reopen_closed_tab" => {
            app.reopen_last_closed_tab();
        }
        "prev_tab" | "view.prev_tab" => {
            app.switch_to_prev_tab();
        }
        "next_tab" | "view.next_tab" => {
            app.switch_to_next_tab();
        }
        "zoom_in" | "view.zoom_in" => app.zoom_font(1.0),
        "zoom_out" | "view.zoom_out" => app.zoom_font(-1.0),
        "zoom_reset" | "view.zoom_reset" => app.reset_font_zoom(),
        "open_settings" | "preferences.open_settings_ui" => {
            app.ui.settings.open_settings_ui();
            app.scheduler.invalidate_all();
        }
        "save_macro" | "save_macro_as" | "macro.save_as" => {
            crate::macros::dispatch::save_macro_as(app, "macro");
            return;
        }
        _ => {}
    }

    crate::macros::dispatch::record_command_if_recording(app, cmd);
}
