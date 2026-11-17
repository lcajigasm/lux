//! Buffer context menu. Right click and Control-click open it. A double
//! click selects the word under the pointer.

use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, MouseButton, NamedKey, PlatformContext};

use crate::app::App;

const ITEM_H: f32 = 24.0;
const SEP_H: f32 = 9.0;
const PAD_Y: f32 = 4.0;
const PAD_X: f32 = 12.0;
const LABEL_SIZE: f32 = 13.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditMenuAction {
    Cut,
    Copy,
    Paste,
    SelectAll,
    ToggleComment,
    GotoDefinition,
    StageHunk,
    UnstageHunk,
    CommandPalette,
}

impl EditMenuAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::ToggleComment => "Toggle Comment",
            Self::GotoDefinition => "Go to Definition",
            Self::StageHunk => "Stage Hunk",
            Self::UnstageHunk => "Unstage Hunk",
            Self::CommandPalette => "Command Palette",
        }
    }

    fn command(self) -> Option<&'static str> {
        match self {
            Self::Cut => Some("edit.cut"),
            Self::Copy => Some("edit.copy"),
            Self::Paste => Some("edit.paste"),
            Self::SelectAll => Some("edit.select_all"),
            Self::ToggleComment => Some("edit.toggle_comment"),
            Self::GotoDefinition => Some("lsp.goto_definition"),
            Self::StageHunk => Some("git.stage_hunk"),
            Self::UnstageHunk => Some("git.unstage_hunk"),
            Self::CommandPalette => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditMenuRow {
    Action {
        action: EditMenuAction,
        enabled: bool,
    },
    Separator,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BufferContextMenu {
    /// Top-left and size in logical points. Physical pixels are `rect(scale)`.
    origin: [f32; 2],
    size: [f32; 2],
    pub(crate) rows: Vec<EditMenuRow>,
}

impl BufferContextMenu {
    pub(crate) fn at(app: &mut App, x: f32, y: f32) -> Self {
        let rows = menu_rows(app);
        let scale = app.scale_factor.max(0.5);
        let family = app.editor_view.family().to_string();
        let style = lux_render::LineStyle {
            family: &family,
            font_size: LABEL_SIZE,
            ligatures: false,
        };
        let mut width = 0.0_f32;
        for row in &rows {
            if let EditMenuRow::Action { action, .. } = row {
                let measured = app
                    .editor_view
                    .text_layout_mut()
                    .layout_line(&style, action.label(), &[])
                    .width();
                width = width.max(measured);
            }
        }
        width += PAD_X * 2.0;
        let mut height = PAD_Y * 2.0;
        for row in &rows {
            height += match row {
                EditMenuRow::Separator => SEP_H,
                EditMenuRow::Action { .. } => ITEM_H,
            };
        }
        let (window_w, window_h) = window_size(app);
        let mut origin_x = x / scale;
        let mut origin_y = y / scale;
        if origin_x + width > window_w / scale {
            origin_x = (window_w / scale - width).max(0.0);
        }
        if origin_y + height > window_h / scale {
            origin_y = (window_h / scale - height).max(0.0);
        }
        Self {
            origin: [origin_x, origin_y],
            size: [width, height],
            rows,
        }
    }

    /// Panel rectangle in physical pixels at `scale`.
    pub(crate) fn rect(&self, scale: f32) -> [f32; 4] {
        let scale = scale.max(0.5);
        [
            self.origin[0] * scale,
            self.origin[1] * scale,
            self.size[0] * scale,
            self.size[1] * scale,
        ]
    }

    pub(crate) fn row_rects(&self, scale: f32) -> Vec<[f32; 4]> {
        let [x, mut y, w, _] = self.rect(scale);
        let scale = scale.max(0.5);
        y += PAD_Y * scale;
        let mut rects = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            let h = match row {
                EditMenuRow::Separator => SEP_H,
                EditMenuRow::Action { .. } => ITEM_H,
            } * scale;
            rects.push([x, y, w, h]);
            y += h;
        }
        rects
    }

    pub(crate) fn action_at(&self, mouse_x: f32, mouse_y: f32, scale: f32) -> Option<EditMenuRow> {
        let [x, y, w, h] = self.rect(scale);
        if mouse_x < x || mouse_x > x + w || mouse_y < y || mouse_y > y + h {
            return None;
        }
        self.row_rects(scale)
            .into_iter()
            .zip(self.rows.iter().copied())
            .find(|(rect, _)| {
                mouse_x >= rect[0]
                    && mouse_x <= rect[0] + rect[2]
                    && mouse_y >= rect[1]
                    && mouse_y <= rect[1] + rect[3]
            })
            .map(|(_, row)| row)
    }
}

pub(crate) fn dismiss_on_escape(app: &mut App, event: &KeyEvent) -> bool {
    if event.state != ElementState::Pressed || event.key != Key::Named(NamedKey::Escape) {
        return false;
    }
    let tab = app.documents.tab_context_menu.take().is_some();
    let edit = app.mouse.context_menu.take().is_some();
    if tab || edit {
        app.scheduler.invalidate_all();
        true
    } else {
        false
    }
}

/// A click on an open menu. `true` means the click is consumed.
pub(crate) fn take_menu_click(
    app: &mut App,
    ctx: &dyn PlatformContext,
    button: MouseButton,
) -> bool {
    let Some(menu) = app.mouse.context_menu.clone() else {
        return false;
    };
    if button != MouseButton::Left {
        app.mouse.context_menu = None;
        return false;
    }
    let mouse_x = app.mouse.position.x as f32;
    let mouse_y = app.mouse.position.y as f32;
    match menu.action_at(mouse_x, mouse_y, app.scale_factor) {
        Some(EditMenuRow::Action { action, enabled }) if enabled => {
            app.mouse.context_menu = None;
            run_action(app, ctx, action);
            app.scheduler.invalidate_all();
            true
        }
        Some(_) => true,
        None => {
            app.mouse.context_menu = None;
            false
        }
    }
}

pub(crate) fn press_editor(app: &mut App, ctx: &dyn PlatformContext, button: MouseButton) {
    // Control-click is the context-menu gesture. Command-click (Meta) still
    // adds a cursor, including on macOS where Command is not Control.
    let contextual = button == MouseButton::Right
        || (button == MouseButton::Left && app.modifiers.control && !app.modifiers.meta);
    if contextual {
        open_context_menu(app);
        return;
    }
    if button != MouseButton::Left {
        return;
    }
    dismiss_pickers(app);
    crate::focus::focus_editor(app);
    let pos = app.position_at_mouse();
    let now = std::time::Instant::now();
    let clicks = match app.mouse.last_click {
        Some((last, line, count))
            if now.duration_since(last).as_millis() < 400 && line == pos.line =>
        {
            count.saturating_add(1)
        }
        _ => 1,
    };
    app.mouse.last_click = Some((now, pos.line, clicks));
    app.mouse.word_anchor = None;
    app.mouse.line_anchor = None;
    if clicks == 2 && app.is_find_results_active() {
        crate::find_results::open_find_result_at_line(app, Some(ctx), pos.line);
        return;
    }
    app.begin_mouse_selection(clicks);
    if app.mouse.selection_anchor.is_none() {
        return;
    }
    if clicks >= 3 {
        app.expand_to_line();
        let line = app.cursors.primary();
        app.mouse.line_anchor = Some(line);
        app.mouse.selection_anchor = Some(line.anchor);
    } else if clicks == 2 {
        app.expand_to_word();
        let word = app.cursors.primary();
        app.mouse.word_anchor = Some(word);
        app.mouse.selection_anchor = Some(word.anchor);
    }
}

fn open_context_menu(app: &mut App) {
    dismiss_pickers(app);
    crate::focus::focus_editor(app);
    let pos = app.position_at_mouse();
    if !click_keeps_selection(app, pos.char) {
        app.cursors = lux_core::multicursor::MultiCursor::new(Selection::cursor(pos));
        app.reset_goal_columns();
    }
    let x = app.mouse.position.x as f32;
    let y = app.mouse.position.y as f32;
    app.mouse.context_menu = Some(BufferContextMenu::at(app, x, y));
    app.mouse.selection_anchor = None;
    app.mouse.word_anchor = None;
    app.mouse.line_anchor = None;
    app.scheduler.invalidate_all();
}

fn click_keeps_selection(app: &App, char_index: usize) -> bool {
    app.cursors.cursors().iter().any(|selection| {
        if selection.is_empty() {
            return false;
        }
        let (start, end) = selection.ordered();
        char_index >= start.char && char_index <= end.char
    })
}

fn dismiss_pickers(app: &mut App) {
    app.ui.project_search = None;
    app.ui.command_palette = None;
    app.ui.goto_anything = None;
    app.ui.language_picker = None;
    app.ui.build.picker = None;
    app.ui.project.picker = None;
    app.ui.theme_picker = None;
    app.ui.snippets.picker = None;
    app.ui.git.branch_picker = None;
}

fn run_action(app: &mut App, ctx: &dyn PlatformContext, action: EditMenuAction) {
    if action == EditMenuAction::CommandPalette {
        app.toggle_command_palette();
        return;
    }
    if let Some(id) = action.command() {
        crate::palette::commands::execute_command(app, ctx, id);
    }
}

fn menu_rows(app: &App) -> Vec<EditMenuRow> {
    let selected = app
        .cursors
        .cursors()
        .iter()
        .any(|selection| !selection.is_empty());
    let blocked = app.editing_blocked() || app.buffer.is_huge_file_mode();
    let mut rows = vec![
        EditMenuRow::Action {
            action: EditMenuAction::Cut,
            enabled: selected && !blocked,
        },
        EditMenuRow::Action {
            action: EditMenuAction::Copy,
            enabled: selected,
        },
        EditMenuRow::Action {
            action: EditMenuAction::Paste,
            enabled: !blocked,
        },
        EditMenuRow::Action {
            action: EditMenuAction::SelectAll,
            enabled: true,
        },
        EditMenuRow::Action {
            action: EditMenuAction::ToggleComment,
            enabled: app.line_comment_token().is_some(),
        },
    ];
    if crate::palette::command_exists("lsp.goto_definition") {
        rows.push(EditMenuRow::Action {
            action: EditMenuAction::GotoDefinition,
            enabled: true,
        });
    }
    let on_hunk = caret_on_hunk(app);
    if crate::palette::command_exists("git.stage_hunk") {
        rows.push(EditMenuRow::Action {
            action: EditMenuAction::StageHunk,
            enabled: on_hunk,
        });
    }
    if crate::palette::command_exists("git.unstage_hunk") {
        rows.push(EditMenuRow::Action {
            action: EditMenuAction::UnstageHunk,
            enabled: on_hunk,
        });
    }
    rows.push(EditMenuRow::Separator);
    rows.push(EditMenuRow::Action {
        action: EditMenuAction::CommandPalette,
        enabled: true,
    });
    rows
}

fn caret_on_hunk(app: &App) -> bool {
    let line = app.cursors.primary().head.line as u32 + 1;
    app.ui.git.hunks.iter().any(|hunk| {
        if hunk.after_len == 0 {
            hunk.anchor_line() == line
        } else {
            line >= hunk.after_start && line < hunk.after_start + hunk.after_len
        }
    })
}

pub(crate) fn geometry_probe(app: &App) -> String {
    let Some(menu) = &app.mouse.context_menu else {
        return "0".to_string();
    };
    let scale = app.scale_factor;
    let (window_w, window_h) = window_size(app);
    let [x, y, w, h] = menu.rect(scale);
    let mut out = format!(
        "{window_w:.1},{window_h:.1},{:.1},{:.1};{x:.1},{y:.1},{w:.1},{h:.1}",
        app.mouse.position.x, app.mouse.position.y
    );
    for rect in menu.row_rects(scale) {
        out.push(';');
        out.push_str(&format!(
            "{:.1},{:.1},{:.1},{:.1}",
            rect[0], rect[1], rect[2], rect[3]
        ));
    }
    out
}

fn window_size(app: &App) -> (f32, f32) {
    if let Some(gpu) = &app.gpu_window {
        let size = gpu.window.inner_size();
        return (size.width as f32, size.height as f32);
    }
    (
        app.viewport.width + app.sidebar_width() + app.minimap_width(),
        app.viewport.height
            + app.tab_bar_height()
            + app.status_bar_height()
            + app.search_bar_height()
            + app.term_dock_height(),
    )
}

#[cfg(test)]
mod tests;
