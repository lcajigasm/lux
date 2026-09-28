use lux_core::movement;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_platform::NamedKey;

use crate::app::{App, LINE_HEIGHT};

impl App {
    pub(super) fn move_cursors(&mut self, key: NamedKey) -> bool {
        use NamedKey::*;
        if !matches!(
            key,
            ArrowLeft | ArrowRight | ArrowUp | ArrowDown | Home | End | PageUp | PageDown
        ) {
            return false;
        }
        let vertical = matches!(key, ArrowUp | ArrowDown | PageUp | PageDown);
        let word = if cfg!(target_os = "macos") {
            self.modifiers.alt
        } else {
            self.modifiers.control
        };
        let command = self.command_modifier();
        let primary = self.cursors.primary();
        let mut primary_after = None;
        let pages = (self.viewport.height / (LINE_HEIGHT * self.scale_factor)).max(1.0) as usize;
        let mut selections = Vec::new();
        for (index, &selection) in self.cursors.cursors().iter().enumerate() {
            let pos = selection.head;
            let goal = self.goal_columns.get(index).copied().unwrap_or(pos.column);
            let to = match key {
                Home if command => Position::document_start(),
                End if command => Position::document_end(&self.buffer),
                ArrowLeft if command && cfg!(target_os = "macos") => {
                    movement::line_start(&self.buffer, pos)
                }
                ArrowRight if command && cfg!(target_os = "macos") => {
                    movement::line_end(&self.buffer, pos)
                }
                ArrowUp if command => Position::document_start(),
                ArrowDown if command => Position::document_end(&self.buffer),
                ArrowLeft if word => movement::move_word_left(&self.buffer, pos),
                ArrowRight if word => movement::move_word_right(&self.buffer, pos),
                ArrowLeft if !self.modifiers.shift && !selection.is_empty() => {
                    selection.ordered().0
                }
                ArrowRight if !self.modifiers.shift && !selection.is_empty() => {
                    selection.ordered().1
                }
                ArrowLeft => movement::move_char_left(&self.buffer, pos),
                ArrowRight => movement::move_char_right(&self.buffer, pos),
                ArrowUp => movement::move_line_up(&self.buffer, pos, goal),
                ArrowDown => movement::move_line_down(&self.buffer, pos, goal),
                PageUp => movement::move_page_up(&self.buffer, pos, pages, goal),
                PageDown => movement::move_page_down(&self.buffer, pos, pages, goal),
                Home => movement::line_start(&self.buffer, pos),
                End => movement::line_end(&self.buffer, pos),
                _ => unreachable!(),
            };
            let moved = movement::extend_or_move(selection, to, self.modifiers.shift);
            if selection == primary {
                primary_after = Some(moved);
            } else {
                selections.push(moved);
            }
        }
        selections.push(primary_after.unwrap());
        let count = self.cursors.count();
        self.cursors = MultiCursor::from_cursors(selections);
        if !vertical || count != self.cursors.count() {
            self.reset_goal_columns();
        }
        self.reveal_primary_cursor();
        true
    }
}
