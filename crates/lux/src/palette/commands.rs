//! Palette commands execution.

use lux_platform::PlatformContext;

use crate::app::App;

pub(crate) fn execute_command(app: &mut App, ctx: &dyn PlatformContext, command_id: &str) {
    if let Some(name) = command_id.strip_prefix("theme.select:") {
        app.confirm_theme(name);
        return;
    }
    match command_id {
        "file.new" => app.request_document(ctx, crate::document::Request::New),
        "file.open" => app.request_document(ctx, crate::document::Request::Open(None)),
        "file.save" => app.request_document(ctx, crate::document::Request::Save { save_as: false }),
        "file.save_all" => app.save_all(ctx),
        "file.save_as" => {
            app.request_document(ctx, crate::document::Request::Save { save_as: true })
        }
        "file.trim_trailing_whitespace" => app.trim_trailing_whitespace(),
        "file.ensure_final_newline" => app.ensure_final_newline(),
        "file.close_tab" => app.request_document(ctx, crate::document::Request::Close),
        "file.reopen_closed_tab" => {
            app.reopen_last_closed_tab();
        }
        "file.clone_view_to_group" => {
            app.clone_view_to_group(None);
        }
        "file.new_view_into_file" => {
            app.new_view_into_file();
        }
        "edit.undo" => {
            if !app.buffer.is_huge_file_mode()
                && let Some(selections) = app.history.undo(&mut app.buffer)
            {
                app.cursors = lux_core::multicursor::MultiCursor::from_cursors(selections);
                app.reset_goal_columns();
                app.sync_syntax_full();
                crate::split::clone_view::clamp_cloned_panes_after_undo(app);
            }
        }
        "edit.redo" => {
            if !app.buffer.is_huge_file_mode()
                && let Some(selections) = app.history.redo(&mut app.buffer)
            {
                app.cursors = lux_core::multicursor::MultiCursor::from_cursors(selections);
                app.reset_goal_columns();
                app.sync_syntax_full();
                crate::split::clone_view::clamp_cloned_panes_after_undo(app);
            }
        }
        "edit.cut" => {
            if !app.editing_blocked() && !app.buffer.is_huge_file_mode() {
                let clipboard = Some(ctx.clipboard());
                if app.copy_to_clipboard(clipboard) {
                    app.insert_at_cursor("");
                }
            }
        }
        "edit.copy" => {
            let clipboard = Some(ctx.clipboard());
            app.copy_to_clipboard(clipboard);
        }
        "edit.paste" => {
            if !app.editing_blocked()
                && !app.buffer.is_huge_file_mode()
                && let Ok(text) = ctx.clipboard().read_text()
            {
                app.insert_at_cursor(&text);
            }
        }
        "edit.select_all" => {
            app.cursors =
                lux_core::multicursor::MultiCursor::new(lux_core::selection::Selection::new(
                    lux_core::position::Position::document_start(),
                    lux_core::position::Position::document_end(&app.buffer),
                ));
            app.reset_goal_columns();
        }
        "edit.duplicate_line" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::duplicate_line);
        }
        "edit.insert_line_after" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::insert_line_after);
        }
        "edit.insert_line_before" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::insert_line_before);
        }
        "edit.delete_line" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::delete_line);
        }
        "edit.swap_line_up" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::move_line_up);
        }
        "edit.swap_line_down" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::move_line_down);
        }
        "edit.join_lines" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::join_lines);
        }
        "edit.indent" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::indent);
        }
        "edit.dedent" => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::dedent);
        }
        "markdown.toggle_preview" => {
            crate::markdown_preview::toggle(app);
        }
        "edit.toggle_comment" => {
            if let Some(token) = app.line_comment_token() {
                app.apply_edit_operation(app.cursors.clone(), |buffer, selection| {
                    lux_core::edit_ops::toggle_line_comment(buffer, selection, token)
                });
            }
        }
        "find" => crate::search::open_find(app, false),
        "find.use_selection" => crate::search::use_selection_for_find(app),
        "find.replace" => crate::search::open_find(app, true),
        "find.next" => crate::search::find_next(app),
        "find.prev" => crate::search::find_prev(app),
        "find.all" => crate::search::find_all(app),
        "selection.expand_to_word" => app.expand_to_word(),

        "selection.next_occurrence" => app.select_next_occurrence(),
        "selection.skip_occurrence" => app.skip_current_occurrence(),
        "selection.all_occurrences" => app.select_all_occurrences(),
        "selection.split_into_lines" => app.split_selection_into_lines(),
        "selection.add_cursor_above" => app.add_cursor_above(),
        "selection.add_cursor_below" => app.add_cursor_below(),
        "selection.expand_to_line" => app.expand_to_line(),
        "selection.expand_to_scope" => app.expand_to_scope(),
        "selection.expand_to_tag" => app.expand_to_tag(),
        "selection.jump_to_matching_bracket" => app.jump_to_matching_bracket(),
        "bookmark.toggle" => app.toggle_bookmark(),
        "bookmark.next" => app.next_bookmark(),
        "bookmark.prev" => app.prev_bookmark(),
        "bookmark.clear" => app.clear_bookmarks(),
        "bookmark.select_all" => app.select_all_bookmarks(),
        "code.fold" => app.fold_at_cursor(),
        "code.unfold" => app.unfold_at_cursor(),
        "code.fold_all" => app.fold_all(),
        "code.unfold_all" => app.unfold_all(),
        "code.fold_level_1" => app.fold_by_level(1),
        "code.fold_level_2" => app.fold_by_level(2),
        "code.fold_level_3" => app.fold_by_level(3),
        "code.fold_level_4" => app.fold_by_level(4),
        "code.fold_level_5" => app.fold_by_level(5),
        "view.next_tab" => {
            app.cycle_tab(true);
        }
        "view.prev_tab" => {
            app.cycle_tab(false);
        }
        "view.toggle_line_endings" | lux_ui::status_bar::CHANGE_EOL_COMMAND => {
            crate::status_bar::toggle_eol(&mut app.buffer);
        }
        "view.toggle_indentation" | lux_ui::status_bar::CHANGE_INDENTATION_COMMAND => {
            crate::status_bar::toggle_indent(&mut app.buffer);
        }
        "view.toggle_encoding" | lux_ui::status_bar::CHANGE_ENCODING_COMMAND => {
            crate::status_bar::toggle_encoding(&mut app.buffer);
        }
        lux_ui::status_bar::SHOW_BRANCH_COMMAND => {
            if let Some(name) = app.ui.git.branch_name.clone() {
                app.documents.message = Some(format!("Git branch {name}"));
            }
        }
        "view.change_language_mode" | lux_ui::status_bar::CHANGE_LANGUAGE_COMMAND => {
            app.open_language_picker();
        }
        "view.toggle_side_bar" => {
            app.toggle_sidebar();
        }
        "view.reveal_active_file" => {
            crate::sidebar::reveal_open_document(app);
            app.chrome
                .focus
                .set_focus(lux_ui::focus::FocusTarget::FileTree);
            crate::focus::sync_surface_focus(app, lux_ui::focus::FocusTarget::FileTree);
            app.scheduler.invalidate_all();
        }
        "view.toggle_minimap" => {
            app.toggle_minimap();
        }
        "view.toggle_word_wrap" => {
            app.toggle_word_wrap();
            #[cfg(target_os = "macos")]
            crate::menu::macos::update_view_checkmarks(app);
        }
        "view.toggle_distraction_free" => {
            app.toggle_distraction_free();
            #[cfg(target_os = "macos")]
            crate::menu::macos::update_view_checkmarks(app);
        }
        "view.split_right" => {
            app.split_editor_right();
        }
        "view.split_down" => {
            app.split_editor_down();
        }
        "view.close_split" => {
            app.close_active_split();
        }
        "view.focus_next_split" => {
            app.focus_next_split();
        }
        "view.layout_single" => {
            app.set_layout(crate::split::LayoutPreset::Single);
        }
        "view.layout_columns_2" => {
            app.set_layout(crate::split::LayoutPreset::Columns2);
        }
        "view.layout_columns_3" => {
            app.set_layout(crate::split::LayoutPreset::Columns3);
        }
        "view.layout_columns_4" => {
            app.set_layout(crate::split::LayoutPreset::Columns4);
        }
        "view.layout_rows_2" => {
            app.set_layout(crate::split::LayoutPreset::Rows2);
        }
        "view.layout_rows_3" => {
            app.set_layout(crate::split::LayoutPreset::Rows3);
        }
        "view.layout_grid_2x2" => {
            app.set_layout(crate::split::LayoutPreset::Grid2x2);
        }
        "view.focus_group_1" => {
            app.focus_group(0);
        }
        "view.focus_group_2" => {
            app.focus_group(1);
        }
        "view.focus_group_3" => {
            app.focus_group(2);
        }
        "view.focus_group_4" => {
            app.focus_group(3);
        }
        "view.move_to_group_1" => {
            app.move_to_group(0);
        }
        "view.move_to_group_2" => {
            app.move_to_group(1);
        }
        "view.move_to_group_3" => {
            app.move_to_group(2);
        }
        "view.move_to_group_4" => {
            app.move_to_group(3);
        }
        "view.locale_en" => {
            app.chrome.locale = lux_ui::i18n::Locale::En;
            crate::accessibility::announce(app, "Language set to English");
            app.scheduler.invalidate_all();
        }
        "view.locale_es" => {
            app.chrome.locale = lux_ui::i18n::Locale::Es;
            crate::accessibility::announce(app, "Idioma cambiado a español");
            app.scheduler.invalidate_all();
        }
        "find.find_in_files" => {
            app.open_project_find();
        }

        "find.replace_in_files" => {
            app.open_project_replace();
        }
        "find.next_result" => {
            crate::find_results::next_result(app, Some(ctx));
        }
        "find.prev_result" => {
            crate::find_results::prev_result(app, Some(ctx));
        }
        "goto.anything" => {
            app.toggle_goto_anything();
        }
        "goto.line" => {
            app.open_goto_anything(":");
        }
        "goto.symbol" => {
            app.open_goto_anything("@");
        }
        "goto.symbol_in_project" => {
            app.open_goto_project_symbols();
        }
        "goto.word" => {
            app.open_goto_anything("#");
        }
        "file.open_folder" => {
            app.open_folder_dialog(ctx);
        }
        "git.view_diff" => {
            if let Some(doc_path) = app.documents.path.clone()
                && let Some(repo_root) = &app.ui.git.repo_root
                && let Ok(rel) = doc_path.strip_prefix(repo_root)
            {
                crate::git::open_file_diff(app, rel);
            }
        }
        "git.stage_file" => {
            if let Some(doc_path) = app.documents.path.clone()
                && let Some(repo_root) = &app.ui.git.repo_root
                && let Ok(rel) = doc_path.strip_prefix(repo_root)
            {
                crate::git::stage_or_unstage_file(app, rel, true);
            }
        }
        "git.unstage_file" => {
            if let Some(doc_path) = app.documents.path.clone()
                && let Some(repo_root) = &app.ui.git.repo_root
                && let Ok(rel) = doc_path.strip_prefix(repo_root)
            {
                crate::git::stage_or_unstage_file(app, rel, false);
            }
        }
        "git.commit_staged" => {
            crate::git::commit_all_staged(app, "Commit from Lux");
        }
        "lsp.goto_definition" => {
            crate::lsp::goto_definition(app, ctx);
        }
        "lsp.find_references" => {
            crate::lsp::find_references(app, ctx);
        }
        "lsp.format_document" => {
            crate::lsp::format_document(app, ctx);
        }
        "lsp.toggle_problems" => {
            app.ui.lsp.problems.visible = !app.ui.lsp.problems.visible;
            app.scheduler.invalidate_all();
        }
        "lsp.hover" => {
            crate::lsp::request_hover(app, ctx);
        }
        "terminal.toggle" => {
            app.ui.term.visible = !app.ui.term.visible;
            if app.ui.term.visible {
                app.ui.term.focused = true;
                let root = app.workspace_root();
                let _ = app.ui.term.ensure_session(root.as_deref());
            } else {
                app.ui.term.focused = false;
            }
            app.scheduler.invalidate_all();
        }
        "terminal.new" => {
            app.ui.term.visible = true;
            app.ui.term.focused = true;
            let root = app.workspace_root();
            let _ = app.ui.term.spawn_session(None, None, root.as_deref());
            app.scheduler.invalidate_all();
        }
        "terminal.close" => {
            app.ui.term.close_active_session();
            app.scheduler.invalidate_all();
        }
        "terminal.run_task" => {
            let mut cmd = lux_term::CommandBuilder::new("cargo");
            cmd.args(["check"]);
            if let Some(root) = app.workspace_root() {
                cmd.cwd(root);
            }
            let _ = app.ui.term.run_task("cargo check", cmd);
            app.scheduler.invalidate_all();
        }
        "terminal.cancel_task" => {
            app.ui.term.cancel_active_task();
            app.scheduler.invalidate_all();
        }
        "terminal.clear" => {
            if let Some(pty) = app.ui.term.active_pty_mut() {
                let _ = pty.write(b"\x0c");
            }
            app.scheduler.invalidate_all();
        }
        "plugin.install_samples" => {
            let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("lux-plugin")
                .join("examples");
            for sample in ["echo", "theme-lux-dark-plus", "identity-fmt"] {
                let _ = app.ui.plugin.install(&manifest_dir.join(sample));
            }
            app.scheduler.invalidate_all();
        }
        "plugin.run_echo" => {
            let ws = app.workspace_root();
            let _ = app.ui.plugin.request_run(
                "echo",
                "echo",
                &[lux_plugin::Capability::FileRead],
                ws.as_deref(),
            );
            app.scheduler.invalidate_all();
        }
        "plugin.run_fmt" => {
            let ws = app.workspace_root();
            let _ = app
                .ui
                .plugin
                .request_run("identity-fmt", "identity", &[], ws.as_deref());
            app.scheduler.invalidate_all();
        }
        "plugin.apply_theme" => {
            if let Some(plugin) = app.ui.plugin.get("lux-dark-plus")
                && let Some(theme_file) = plugin.themes.first()
            {
                let theme_path = app
                    .ui
                    .plugin
                    .plugins_dir
                    .join("lux-dark-plus")
                    .join(theme_file);
                if let Ok(bytes) = std::fs::read(&theme_path)
                    && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes)
                    && let Some(colors) = val.get("colors")
                {
                    if let Some(bg_str) = colors.get("editor.background").and_then(|v| v.as_str())
                        && let Some(bg) = lux_ui::Color::from_hex(bg_str)
                    {
                        app.chrome.theme.background = bg;
                    }
                    if let Some(fg_str) = colors.get("editor.foreground").and_then(|v| v.as_str())
                        && let Some(fg) = lux_ui::Color::from_hex(fg_str)
                    {
                        app.chrome.theme.foreground = fg;
                    }
                    app.scheduler.invalidate_all();
                }
            }
        }
        "preferences.color_theme" | "workbench.action.selectTheme" => {
            app.open_theme_picker();
        }
        "preferences.open_settings_json" => {
            let settings = std::mem::take(&mut app.ui.settings);
            settings.open_settings_json(app, ctx);
            app.ui.settings = settings;
        }
        "view.zoom_in" => app.zoom_font(1.0),
        "view.zoom_out" => app.zoom_font(-1.0),
        "view.zoom_reset" => app.reset_font_zoom(),
        "preferences.open_settings_ui" => {
            let mut settings = std::mem::take(&mut app.ui.settings);
            settings.open_settings_ui();
            app.ui.settings = settings;
            app.scheduler.invalidate_all();
        }
        "preferences.import_vscode"
        | "preferences.import_sublime"
        | "preferences.import_jetbrains" => {
            let mut settings = std::mem::take(&mut app.ui.settings);
            let msg = match command_id {
                "preferences.import_vscode" => settings.import_vscode(app),
                "preferences.import_sublime" => settings.import_sublime(app),
                _ => settings.import_jetbrains(app),
            };
            app.ui.settings = settings;
            app.ui.announcement = Some(msg);
            app.scheduler.invalidate_all();
        }
        "build.build" => {
            app.build(Some(ctx));
        }
        "build.build_with" => {
            app.build_with();
        }
        "build.cancel" => {
            app.cancel_build();
        }
        "project.open_project" => {
            app.open_project_dialog(ctx);
        }
        "project.save_project_as" => {
            app.save_project_as_dialog(ctx);
        }
        "project.quick_switch" => {
            app.quick_switch_project();
        }
        "project.close_project" => {
            app.close_project();
        }
        "snippet.insert" => {
            crate::snippets::dispatch::open_snippet_picker(app);
        }
        "snippet.next_tabstop" => {
            if let Some(session) = &mut app.snippet_session {
                if let Some(sel) = session.next(&app.buffer) {
                    app.cursors = lux_core::multicursor::MultiCursor::new(sel);
                    app.reset_goal_columns();
                } else {
                    app.snippet_session = None;
                }
            }
        }
        "snippet.prev_tabstop" => {
            if let Some(session) = &mut app.snippet_session
                && let Some(sel) = session.prev(&app.buffer)
            {
                app.cursors = lux_core::multicursor::MultiCursor::new(sel);
                app.reset_goal_columns();
            }
        }
        "macro.toggle_record" | "toggle_record_macro" => {
            crate::macros::dispatch::toggle_record_macro(app);
        }
        "macro.playback" | "playback_macro" => {
            crate::macros::dispatch::playback_macro(app);
        }
        "macro.save_as" | "save_macro" => {
            crate::macros::dispatch::save_macro_as(app, "macro");
        }
        "help.about" => {
            app.open_about_dialog();
        }
        _ => {}
    }
}
