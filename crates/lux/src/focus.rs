//! Keyboard navigation and F6 focus ring integration (G19).
//!
//! Connects `lux_ui::focus::FocusRing` with visible UI surfaces,
//! routing keyboard inputs to the focused surface, providing visual focus indicators,
//! and untrapping to the editor on Escape.

use lux_platform::{ElementState, Key, KeyEvent, NamedKey, PlatformContext};
use lux_ui::focus::FocusTarget;

use crate::app::App;

/// Checks if a focus target panel or surface is currently visible.
pub(crate) fn is_target_visible(ui: &crate::ui::UiOverlays, target: FocusTarget) -> bool {
    match target {
        FocusTarget::Editor => true,
        FocusTarget::CommandPalette => ui.command_palette.is_some() || ui.goto_anything.is_some(),
        FocusTarget::Search => ui.search_bar.is_some() || ui.project_search.is_some(),
        FocusTarget::FileTree => ui
            .sidebar
            .as_ref()
            .is_some_and(|s| s.visible && s.tree.is_some()),
        FocusTarget::Git => ui.git.active_popup.is_some(),
        FocusTarget::Terminal => ui.term.visible,
        FocusTarget::Problems => ui.lsp.problems.visible,
    }
}

/// Cycles focus forward along the F6 ring among currently visible surfaces.
pub(crate) fn cycle_focus_forward(app: &mut App) -> FocusTarget {
    let ui = &app.ui;
    let target = app
        .chrome
        .focus
        .focus_next_visible(|t| is_target_visible(ui, t));
    sync_surface_focus(app, target);
    app.scheduler.invalidate_all();
    target
}

/// Cycles focus backward along the F6 ring among currently visible surfaces.
pub(crate) fn cycle_focus_backward(app: &mut App) -> FocusTarget {
    let ui = &app.ui;
    let target = app
        .chrome
        .focus
        .focus_prev_visible(|t| is_target_visible(ui, t));
    sync_surface_focus(app, target);
    app.scheduler.invalidate_all();
    target
}

/// Synchronizes state flags across surfaces to reflect the active focus target.
pub(crate) fn sync_surface_focus(app: &mut App, target: FocusTarget) {
    if let Some(sidebar) = app.ui.sidebar.as_mut() {
        sidebar.focused = target == FocusTarget::FileTree;
    }
    app.ui.term.focused = target == FocusTarget::Terminal;
    app.ui.lsp.problems.focused = target == FocusTarget::Problems;
}

/// Puts the caret back in the editor after a floating panel closes.
pub(crate) fn focus_editor(app: &mut App) {
    app.chrome.focus.escape_to_editor();
    sync_surface_focus(app, FocusTarget::Editor);
}

/// Untraps focus from any panel and immediately returns focus to the active Editor view,
/// dismissing any active floating popups or overlays.
pub(crate) fn escape_to_editor(app: &mut App) -> bool {
    let mut changed = false;
    if app.chrome.focus.current() != FocusTarget::Editor {
        app.chrome.focus.escape_to_editor();
        sync_surface_focus(app, FocusTarget::Editor);
        changed = true;
    }
    if app.ui.command_palette.take().is_some() {
        changed = true;
    }
    if app.ui.goto_anything.take().is_some() {
        changed = true;
    }
    if app.ui.search_bar.take().is_some() {
        changed = true;
    }
    if app.ui.project_search.take().is_some() {
        changed = true;
    }
    if app.ui.git.active_popup.take().is_some() {
        changed = true;
    }
    if app.ui.lsp.hover_popup.take().is_some() {
        changed = true;
    }
    if app.ui.lsp.completion_popup.take().is_some() {
        changed = true;
    }
    if app.ui.settings.panel.take().is_some() {
        changed = true;
    }
    if changed {
        app.scheduler.invalidate_all();
    }
    changed
}

/// Handles F6 navigation, Escape untrapping, and input routing to the focused panel.
pub(crate) fn handle_focus_key(app: &mut App, ctx: &dyn PlatformContext, event: &KeyEvent) -> bool {
    if event.state != ElementState::Pressed {
        return false;
    }

    // 0. Settings UI panel routing
    if app.ui.settings.panel.is_some() {
        let mut settings = std::mem::take(&mut app.ui.settings);
        let handled = settings.handle_key(app, event);
        app.ui.settings = settings;
        if handled {
            return true;
        }
    }

    // 1. F6 / Shift+F6 cycling
    if matches!(&event.key, Key::Named(NamedKey::F(6))) {
        if app.modifiers.shift {
            cycle_focus_backward(app);
        } else {
            cycle_focus_forward(app);
        }
        return true;
    }

    // 2. Escape untrap to Editor if not currently focused on Editor
    if matches!(&event.key, Key::Named(NamedKey::Escape))
        && app.chrome.focus.current() != FocusTarget::Editor
    {
        escape_to_editor(app);
        return true;
    }

    // 3. Route keyboard input exclusively to the currently focused surface
    let current = app.chrome.focus.current();
    match current {
        FocusTarget::CommandPalette => {
            if crate::palette::handle_palette_key(app, ctx, event)
                || crate::goto::handle_goto_key(app, ctx, event)
            {
                return true;
            }
        }
        FocusTarget::Search => {
            if crate::search::handle_search_key(app, ctx, event)
                || crate::project_search::handle_project_search_key(app, ctx, event)
            {
                return true;
            }
        }
        FocusTarget::FileTree => {
            if crate::sidebar::handle_sidebar_key(app, ctx, event) {
                return true;
            }
        }
        FocusTarget::Git => {
            if crate::conflict::handle_conflict_key(app, ctx, event) {
                return true;
            }
        }
        FocusTarget::Terminal => {
            if crate::term::input::handle_term_key(app, ctx, event) {
                return true;
            }
        }
        FocusTarget::Problems => {
            if handle_problems_key(app, ctx, event) {
                return true;
            }
        }
        FocusTarget::Editor => {
            if app.is_find_results_active() && event.key == Key::Named(NamedKey::Enter) {
                let line_idx = app.cursors.primary().head.line;
                if crate::find_results::open_find_result_at_line(app, Some(ctx), line_idx) {
                    return true;
                }
            }
        }
    }

    false
}

/// Handles keyboard navigation when the LSP Problems panel is focused.
fn handle_problems_key(app: &mut App, _ctx: &dyn PlatformContext, event: &KeyEvent) -> bool {
    let total_problems = app
        .ui
        .lsp
        .all_diagnostics
        .iter()
        .map(|(_, diags)| diags.len())
        .sum::<usize>();

    if total_problems == 0 {
        return false;
    }

    match &event.key {
        Key::Named(NamedKey::ArrowDown) => {
            if app.ui.lsp.problems.selected_index + 1 < total_problems {
                app.ui.lsp.problems.selected_index += 1;
                app.scheduler.invalidate_all();
                return true;
            }
        }
        Key::Named(NamedKey::ArrowUp) => {
            if app.ui.lsp.problems.selected_index > 0 {
                app.ui.lsp.problems.selected_index -= 1;
                app.scheduler.invalidate_all();
                return true;
            }
        }
        Key::Named(NamedKey::Enter) => {
            let mut count = 0;
            let target_idx = app.ui.lsp.problems.selected_index;
            for (url, diags) in &app.ui.lsp.all_diagnostics {
                if target_idx < count + diags.len() {
                    let diag = &diags[target_idx - count];
                    if let Ok(file_path) = url.to_file_path() {
                        crate::lsp::jump_to_location(
                            app,
                            &file_path,
                            diag.range.start.line as usize,
                        );
                        app.chrome.focus.escape_to_editor();
                        sync_surface_focus(app, FocusTarget::Editor);
                        app.scheduler.invalidate_all();
                        return true;
                    }
                }
                count += diags.len();
            }
        }
        Key::Named(NamedKey::Escape) => {
            app.chrome.focus.escape_to_editor();
            sync_surface_focus(app, FocusTarget::Editor);
            app.scheduler.invalidate_all();
            return true;
        }
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_is_always_visible() {
        let ui = crate::ui::UiOverlays::default();
        assert!(is_target_visible(&ui, FocusTarget::Editor));
        assert!(!is_target_visible(&ui, FocusTarget::Terminal));
        assert!(!is_target_visible(&ui, FocusTarget::Problems));
        assert!(!is_target_visible(&ui, FocusTarget::Search));
        assert!(!is_target_visible(&ui, FocusTarget::CommandPalette));
    }

    #[test]
    fn f6_cycle_skips_hidden_panels() {
        let mut ui = crate::ui::UiOverlays::default();
        ui.term.visible = true;
        let is_vis = |t| is_target_visible(&ui, t);

        let mut ring = lux_ui::focus::FocusRing::new();
        assert_eq!(ring.current(), FocusTarget::Editor);
        assert_eq!(ring.focus_next_visible(is_vis), FocusTarget::Terminal);
        assert_eq!(ring.focus_next_visible(is_vis), FocusTarget::Editor);
        assert_eq!(ring.focus_prev_visible(is_vis), FocusTarget::Terminal);
        assert_eq!(ring.escape_to_editor(), FocusTarget::Editor);
    }
}
