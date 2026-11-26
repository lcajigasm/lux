//! Find bar geometry and the option chords that need more than the bar.

use crate::app::App;

/// The bar docks under the text instead of over it: the editor loses
/// exactly the bar's change in height, and the caret is revealed again so
/// the line it sits on (often the last one) is never left behind the bar.
/// A live window then recomputes the whole layout from its real size.
pub(crate) fn fit_editor_to_bar(app: &mut App, bar_height_before: f32) {
    let grown = app.search_bar_height() - bar_height_before;
    app.viewport.height = (app.viewport.height - grown).max(1.0);
    app.update_viewport_dimensions();
    app.reveal_primary_cursor();
    app.scheduler.invalidate_all();
}

/// Closes the bar and hands the space and focus back to the editor.
pub(crate) fn close(app: &mut App) {
    let bar_height_before = app.search_bar_height();
    app.ui.search_bar = None;
    crate::focus::focus_editor(app);
    fit_editor_to_bar(app, bar_height_before);
}

fn bar_focused(app: &App) -> bool {
    app.ui.search_bar.is_some() && app.chrome.focus.current() == lux_ui::focus::FocusTarget::Search
}

/// Option+Command+S. Sublime binds it to In Selection while the find panel
/// has focus and to Save All everywhere else, so it only toggles here when
/// the bar is focused. `false` lets the caller treat it as Save All.
pub(crate) fn in_selection_chord(app: &mut App) -> bool {
    if !bar_focused(app) {
        return false;
    }
    // The native menu item for Save All carries the same key equivalent and
    // delivers this press too; whichever source arrives second is swallowed.
    if crate::menu::claim_key(crate::menu::MenuAction::SaveAll) {
        toggle_in_selection(app);
    }
    true
}

/// The macOS menu runs Save All for Option+Command+S before the view sees
/// the key. With the find bar focused and Command still down, that press is
/// the In Selection chord, so the menu toggles it instead of saving. A Save
/// All picked with the mouse (Command up) still saves.
pub(crate) fn menu_save_all_toggles_in_selection(app: &mut App) -> bool {
    #[cfg(target_os = "macos")]
    if bar_focused(app) && crate::menu::macos::command_key_is_down() {
        toggle_in_selection(app);
        return true;
    }
    let _ = app;
    false
}

fn toggle_in_selection(app: &mut App) {
    let primary = app.cursors.primary();
    let active_range = (!primary.is_empty()).then(|| primary.char_range());
    if let Some(search) = app.ui.search_bar.as_mut() {
        search.toggle_in_selection(&app.buffer, active_range);
    }
    app.scheduler.invalidate_all();
}
