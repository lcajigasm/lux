//! Macro playback engine executing recorded steps against the App state (ST19).

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::{edit_ops, movement};

use super::command::{MacroCommand, MoveBy, MoveTo};
use super::macro_def::Macro;
use crate::app::App;

/// Plays back the given macro against the editor application state.
pub fn execute_macro(app: &mut App, m: &Macro) {
    if m.commands.is_empty() {
        return;
    }

    // Suspend recording during playback to avoid recursive recording of playback actions
    let was_recording = app.ui.macros.is_recording;
    app.ui.macros.is_recording = false;

    for command in &m.commands {
        execute_macro_command(app, command);
    }

    app.ui.macros.is_recording = was_recording;
    app.reset_goal_columns();
    app.reveal_primary_cursor();
    app.scheduler.invalidate_all();
}

/// Plays back the most recently recorded macro.
pub fn playback_last_macro(app: &mut App) -> bool {
    let macro_to_play = match &app.ui.macros.last_macro {
        Some(m) => m.clone(),
        None => return false,
    };

    execute_macro(app, &macro_to_play);
    true
}

fn execute_macro_command(app: &mut App, command: &MacroCommand) {
    match command {
        MacroCommand::InsertText { text } => {
            app.insert_at_cursor(text);
        }
        MacroCommand::InsertNewline => {
            app.insert_newline();
        }
        MacroCommand::DeleteBackward => {
            app.delete_at_cursors(true);
        }
        MacroCommand::DeleteForward => {
            app.delete_at_cursors(false);
        }
        MacroCommand::DeleteWord { forward } => {
            let op = if *forward {
                edit_ops::delete_word_forward
            } else {
                edit_ops::delete_word_backward
            };
            app.apply_edit_operation(app.cursors.clone(), op);
        }
        MacroCommand::Move {
            by,
            forward,
            extend,
        } => {
            execute_move(app, *by, *forward, *extend);
        }
        MacroCommand::MoveTo { to, extend } => {
            execute_move_to(app, *to, *extend);
        }
        MacroCommand::Action { name, .. } => {
            crate::input::dispatch::execute_keymap_command(app, name, None);
        }
    }
}

fn execute_move(app: &mut App, by: MoveBy, forward: bool, extend: bool) {
    let primary = app.cursors.primary();
    let mut primary_after = None;
    let mut selections = Vec::new();

    for (index, &selection) in app.cursors.cursors().iter().enumerate() {
        let pos = selection.head;
        let goal = app.goal_columns.get(index).copied().unwrap_or(pos.column);
        let to = match by {
            MoveBy::Characters => {
                if forward {
                    movement::move_char_right(&app.buffer, pos)
                } else {
                    movement::move_char_left(&app.buffer, pos)
                }
            }
            MoveBy::Words | MoveBy::Stops => {
                if forward {
                    movement::move_word_right(&app.buffer, pos)
                } else {
                    movement::move_word_left(&app.buffer, pos)
                }
            }
            MoveBy::Lines => {
                if forward {
                    movement::move_line_down(&app.buffer, pos, goal)
                } else {
                    movement::move_line_up(&app.buffer, pos, goal)
                }
            }
        };

        let moved = movement::extend_or_move(selection, to, extend);
        if selection == primary {
            primary_after = Some(moved);
        } else {
            selections.push(moved);
        }
    }

    if let Some(prim) = primary_after {
        selections.push(prim);
    }
    app.cursors = MultiCursor::from_cursors(selections);
    app.reset_goal_columns();
}

fn execute_move_to(app: &mut App, to: MoveTo, extend: bool) {
    let primary = app.cursors.primary();
    let mut primary_after = None;
    let mut selections = Vec::new();

    for &selection in app.cursors.cursors().iter() {
        let pos = selection.head;
        let target_pos = match to {
            MoveTo::Bol => movement::line_start(&app.buffer, pos),
            MoveTo::Eol => movement::line_end(&app.buffer, pos),
            MoveTo::Bof => Position::document_start(),
            MoveTo::Eof => Position::document_end(&app.buffer),
        };

        let moved = movement::extend_or_move(selection, target_pos, extend);
        if selection == primary {
            primary_after = Some(moved);
        } else {
            selections.push(moved);
        }
    }

    if let Some(prim) = primary_after {
        selections.push(prim);
    }
    app.cursors = MultiCursor::from_cursors(selections);
    app.reset_goal_columns();
}
