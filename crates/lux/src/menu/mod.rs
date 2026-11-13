//! Native Menu Bar and action dispatching (T03).

use crate::app::App;
use lux_platform::PlatformContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(isize)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) enum MenuAction {
    About = 1,
    Preferences = 2,
    NewFile = 10,
    OpenFile = 11,
    OpenFolder = 12,
    Save = 13,
    SaveAs = 14,
    CloseTab = 15,
    CloneViewToGroup = 16,
    NewViewIntoFile = 17,
    Undo = 20,
    Redo = 21,
    Cut = 22,
    Copy = 23,
    Paste = 24,
    SelectAll = 25,
    ToggleComment = 26,
    DuplicateLine = 27,
    DeleteLine = 28,
    JoinLines = 29,
    AddNextOccurrence = 30,
    SelectAllOccurrences = 31,
    SkipOccurrence = 32,
    SplitIntoLines = 33,
    AddCursorAbove = 34,
    AddCursorBelow = 35,
    ExpandToLine = 36,
    ExpandToScope = 37,
    ExpandToTag = 38,
    Find = 40,
    Replace = 41,
    FindInFiles = 42,
    ReplaceInFiles = 43,
    FindNext = 46,
    FindPrev = 47,
    FindNextResult = 48,
    FindPrevResult = 49,
    SwapLineUp = 44,
    SwapLineDown = 45,

    ToggleSidebar = 50,
    ToggleMinimap = 51,
    ToggleTerminal = 52,
    ChangeLanguageMode = 53,
    NextTab = 54,
    PrevTab = 55,
    ToggleWordWrap = 56,
    SplitRight = 57,
    SplitDown = 58,
    CloseSplit = 59,
    GotoAnything = 60,
    GotoSymbolInProject = 61,
    ToggleDistractionFree = 62,
    CommandPalette = 70,
    Fold = 80,
    Unfold = 81,
    FoldAll = 82,
    UnfoldAll = 83,
    JumpToMatchingBracket = 84,
    ToggleBookmark = 90,
    NextBookmark = 91,
    PrevBookmark = 92,
    ClearBookmarks = 93,
    SelectAllBookmarks = 94,
    LayoutSingle = 110,
    LayoutColumns2 = 111,
    LayoutColumns3 = 112,
    LayoutColumns4 = 113,
    LayoutRows2 = 114,
    LayoutRows3 = 115,
    LayoutGrid2x2 = 116,
    FocusGroup1 = 120,
    FocusGroup2 = 121,
    FocusGroup3 = 122,
    FocusGroup4 = 123,
    MoveToGroup1 = 130,
    MoveToGroup2 = 131,
    MoveToGroup3 = 132,
    MoveToGroup4 = 133,
    Build = 140,
    BuildWith = 141,
    CancelBuild = 142,
    OpenProject = 150,
    SaveProjectAs = 151,
    QuickSwitchProject = 152,
    CloseProject = 153,
    ToggleRecordMacro = 160,
    PlaybackMacro = 161,
    SaveMacro = 162,
    InstallCommand = 163,
    UninstallCommand = 164,
    SelectColorTheme = 3,
    SaveAll = 210,
    ReopenClosedTab = 211,
    NewWindow = 212,
    ClearRecent = 213,
}

impl MenuAction {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn tag(self) -> isize {
        self as isize
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn from_tag(tag: isize) -> Option<Self> {
        tags::from_tag(tag)
    }
}

mod chords;
pub(crate) mod file;
mod queue;
pub(crate) mod recent;
mod tags;

pub(crate) use recent::note;

#[cfg(test)]
pub(crate) use chords::clear_claim;
pub(crate) use chords::{action_for_command, claim_document_request, claim_key};
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
pub(crate) use queue::post_action;
pub(crate) use queue::{poll_actions, register_waker};

pub(crate) fn handle_menu_action(app: &mut App, ctx: &dyn PlatformContext, action: MenuAction) {
    if !chords::claim_menu(action) {
        return;
    }
    if let Some(pending) = app.ui.pending_chord.clone()
        && let Some(command) = chords::menu_completes_pending(&app.keymap, &pending, action)
    {
        app.ui.pending_chord = None;
        app.ui.announcement = None;
        crate::input::dispatch::execute_keymap_command(
            app,
            &command,
            Some(ctx.clipboard()),
            Some(ctx),
        );
        return;
    }
    match action {
        MenuAction::About => {
            app.open_about_dialog();
        }
        MenuAction::Preferences => {
            app.ui.settings.open_settings_ui();
        }
        MenuAction::SelectColorTheme => {
            app.open_theme_picker();
        }
        MenuAction::NewFile => {
            app.request_document(ctx, crate::document::Request::New);
        }
        MenuAction::OpenFile => {
            app.request_document(ctx, crate::document::Request::Open(None));
        }
        MenuAction::OpenFolder => {
            app.open_folder_dialog(ctx);
        }
        MenuAction::Save => {
            app.request_document(ctx, crate::document::Request::Save { save_as: false });
        }
        MenuAction::SaveAs => {
            app.request_document(ctx, crate::document::Request::Save { save_as: true });
        }
        MenuAction::CloseTab => {
            app.request_document(ctx, crate::document::Request::Close);
        }
        MenuAction::SaveAll => {
            app.save_all(ctx);
        }
        MenuAction::ReopenClosedTab => {
            app.reopen_last_closed_tab();
        }
        MenuAction::NewWindow => {
            file::open_new_window(app);
        }
        MenuAction::ClearRecent => {
            recent::clear();
        }
        MenuAction::CloneViewToGroup => {
            app.clone_view_to_group(None);
        }
        MenuAction::NewViewIntoFile => {
            app.new_view_into_file();
        }
        MenuAction::Undo => {
            if !app.buffer.is_huge_file_mode()
                && let Some(selections) = app.history.undo(&mut app.buffer)
            {
                app.cursors = lux_core::multicursor::MultiCursor::from_cursors(selections);
                app.reset_goal_columns();
                app.sync_syntax_full();
                crate::split::clone_view::clamp_cloned_panes_after_undo(app);
            }
        }
        MenuAction::Redo => {
            if !app.buffer.is_huge_file_mode()
                && let Some(selections) = app.history.redo(&mut app.buffer)
            {
                app.cursors = lux_core::multicursor::MultiCursor::from_cursors(selections);
                app.reset_goal_columns();
                app.sync_syntax_full();
                crate::split::clone_view::clamp_cloned_panes_after_undo(app);
            }
        }
        MenuAction::Cut => {
            if !app.editing_blocked()
                && !app.buffer.is_huge_file_mode()
                && app.copy_to_clipboard(Some(ctx.clipboard()))
            {
                app.replace_selections(app.cursors.clone(), "");
            }
        }
        MenuAction::Copy => {
            app.copy_to_clipboard(Some(ctx.clipboard()));
        }
        MenuAction::Paste => {
            if !app.editing_blocked()
                && !app.buffer.is_huge_file_mode()
                && let Ok(text) = ctx.clipboard().read_text()
            {
                app.insert_at_cursor(&text);
            }
        }
        MenuAction::SelectAll => {
            app.cursors =
                lux_core::multicursor::MultiCursor::new(lux_core::selection::Selection::new(
                    lux_core::position::Position::document_start(),
                    lux_core::position::Position::document_end(&app.buffer),
                ));
            app.reset_goal_columns();
        }
        MenuAction::ToggleComment => {
            crate::input::comment::toggle_line_or_block(app);
        }
        MenuAction::DuplicateLine => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::duplicate_line);
        }
        MenuAction::DeleteLine => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::delete_line);
        }
        MenuAction::JoinLines => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::join_lines);
        }
        MenuAction::SwapLineUp => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::move_line_up);
        }
        MenuAction::SwapLineDown => {
            app.apply_edit_operation(app.cursors.clone(), lux_core::edit_ops::move_line_down);
        }
        MenuAction::AddNextOccurrence => {
            app.select_next_occurrence();
        }
        MenuAction::SkipOccurrence => {
            app.skip_current_occurrence();
        }
        MenuAction::SelectAllOccurrences => {
            app.select_all_occurrences();
        }
        MenuAction::SplitIntoLines => {
            app.split_selection_into_lines();
        }
        MenuAction::AddCursorAbove => {
            app.add_cursor_above();
        }
        MenuAction::AddCursorBelow => {
            app.add_cursor_below();
        }
        MenuAction::ExpandToLine => {
            app.expand_to_line();
        }
        MenuAction::ExpandToScope => {
            app.expand_to_scope();
        }
        MenuAction::ExpandToTag => {
            app.expand_to_tag();
        }
        MenuAction::Find => {
            app.open_find();
        }
        MenuAction::FindNext => {
            crate::search::find_next(app);
        }
        MenuAction::FindPrev => {
            crate::search::find_prev(app);
        }
        MenuAction::Replace => {
            app.open_replace();
        }
        MenuAction::FindInFiles => {
            app.open_project_find();
        }
        MenuAction::ReplaceInFiles => {
            app.open_project_replace();
        }
        MenuAction::FindNextResult => {
            crate::find_results::next_result(app, Some(ctx));
        }
        MenuAction::FindPrevResult => {
            crate::find_results::prev_result(app, Some(ctx));
        }
        MenuAction::ToggleSidebar => {
            app.toggle_sidebar();
            #[cfg(target_os = "macos")]
            macos::update_view_checkmarks(app);
        }
        MenuAction::ToggleMinimap => {
            app.toggle_minimap();
            #[cfg(target_os = "macos")]
            macos::update_view_checkmarks(app);
        }
        MenuAction::ToggleTerminal => {
            app.ui.term.visible = !app.ui.term.visible;
            if app.ui.term.visible {
                app.ui.term.focused = true;
                let root = app.workspace_root();
                let _ = app.ui.term.ensure_session(root.as_deref());
            } else {
                app.ui.term.focused = false;
            }
            app.scheduler.invalidate_all();
            #[cfg(target_os = "macos")]
            macos::update_view_checkmarks(app);
        }
        MenuAction::ChangeLanguageMode => {
            app.open_language_picker();
        }
        MenuAction::NextTab => {
            app.cycle_tab(true);
        }
        MenuAction::PrevTab => {
            app.cycle_tab(false);
        }
        MenuAction::ToggleWordWrap => {
            app.toggle_word_wrap();
            #[cfg(target_os = "macos")]
            macos::update_view_checkmarks(app);
        }
        MenuAction::ToggleDistractionFree => {
            app.toggle_distraction_free();
            #[cfg(target_os = "macos")]
            macos::update_view_checkmarks(app);
        }
        MenuAction::SplitRight => {
            app.split_editor_right();
        }
        MenuAction::SplitDown => {
            app.split_editor_down();
        }
        MenuAction::CloseSplit => {
            app.close_active_split();
        }
        MenuAction::GotoAnything => {
            app.toggle_goto_anything();
        }
        MenuAction::GotoSymbolInProject => {
            app.open_goto_project_symbols();
        }
        MenuAction::CommandPalette => {
            app.toggle_command_palette();
        }
        MenuAction::InstallCommand => {
            app.ui.announcement = Some(match crate::cli::install_command() {
                Ok(message) => message,
                Err(message) => message,
            });
        }
        MenuAction::UninstallCommand => {
            app.ui.announcement = Some(match crate::cli::uninstall_command() {
                Ok(message) => message,
                Err(message) => message,
            });
        }
        MenuAction::Fold => {
            app.fold_at_cursor();
        }
        MenuAction::Unfold => {
            app.unfold_at_cursor();
        }
        MenuAction::FoldAll => {
            app.fold_all();
        }
        MenuAction::UnfoldAll => {
            app.unfold_all();
        }
        MenuAction::JumpToMatchingBracket => {
            app.jump_to_matching_bracket();
        }
        MenuAction::ToggleBookmark => {
            app.toggle_bookmark();
        }
        MenuAction::NextBookmark => {
            app.next_bookmark();
        }
        MenuAction::PrevBookmark => {
            app.prev_bookmark();
        }
        MenuAction::ClearBookmarks => {
            app.clear_bookmarks();
        }
        MenuAction::SelectAllBookmarks => {
            app.select_all_bookmarks();
        }
        MenuAction::LayoutSingle => {
            app.set_layout(crate::split::LayoutPreset::Single);
        }
        MenuAction::LayoutColumns2 => {
            app.set_layout(crate::split::LayoutPreset::Columns2);
        }
        MenuAction::LayoutColumns3 => {
            app.set_layout(crate::split::LayoutPreset::Columns3);
        }
        MenuAction::LayoutColumns4 => {
            app.set_layout(crate::split::LayoutPreset::Columns4);
        }
        MenuAction::LayoutRows2 => {
            app.set_layout(crate::split::LayoutPreset::Rows2);
        }
        MenuAction::LayoutRows3 => {
            app.set_layout(crate::split::LayoutPreset::Rows3);
        }
        MenuAction::LayoutGrid2x2 => {
            app.set_layout(crate::split::LayoutPreset::Grid2x2);
        }
        MenuAction::FocusGroup1 => {
            app.focus_group(0);
        }
        MenuAction::FocusGroup2 => {
            app.focus_group(1);
        }
        MenuAction::FocusGroup3 => {
            app.focus_group(2);
        }
        MenuAction::FocusGroup4 => {
            app.focus_group(3);
        }
        MenuAction::MoveToGroup1 => {
            app.move_to_group(0);
        }
        MenuAction::MoveToGroup2 => {
            app.move_to_group(1);
        }
        MenuAction::MoveToGroup3 => {
            app.move_to_group(2);
        }
        MenuAction::MoveToGroup4 => {
            app.move_to_group(3);
        }
        MenuAction::Build => {
            app.build(Some(ctx));
        }
        MenuAction::BuildWith => {
            app.build_with();
        }
        MenuAction::CancelBuild => {
            app.cancel_build();
        }
        MenuAction::OpenProject => {
            app.open_project_dialog(ctx);
        }
        MenuAction::SaveProjectAs => {
            app.save_project_as_dialog(ctx);
        }
        MenuAction::QuickSwitchProject => {
            app.quick_switch_project();
        }
        MenuAction::CloseProject => {
            app.close_project();
        }
        MenuAction::ToggleRecordMacro => {
            crate::macros::dispatch::toggle_record_macro(app);
        }
        MenuAction::PlaybackMacro => {
            crate::macros::dispatch::playback_macro(app);
        }
        MenuAction::SaveMacro => {
            crate::macros::dispatch::save_macro_as(app, "macro");
        }
    }
    app.scheduler.invalidate_all();
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
}

pub(crate) fn poll_and_dispatch(app: &mut App, ctx: &dyn PlatformContext) {
    for index in queue::poll_recent() {
        file::open_recent(app, ctx, index);
    }
    for action in poll_actions() {
        handle_menu_action(app, ctx, action);
    }
}

pub(crate) fn init_native_menu(app: &App) {
    #[cfg(target_os = "macos")]
    macos::install_main_menu(app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(target_os = "macos")]
pub(crate) mod macos_builder;
#[cfg(target_os = "macos")]
pub(crate) mod macos_view_menus;

#[cfg(test)]
mod tests;
