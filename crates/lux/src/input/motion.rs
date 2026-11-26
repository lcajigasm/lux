use std::collections::HashMap;

use lux_core::movement;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_platform::NamedKey;

use crate::app::App;

mod visual_rows;
#[cfg(test)]
mod visual_rows_tests;

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
        let pages = (self.viewport.height / self.editor_view.metrics().line_height.max(1.0))
            .max(1.0) as usize;
        let mut selections = Vec::new();
        let mut goals = HashMap::new();
        let current = self.cursors.cursors().to_vec();
        for (index, &selection) in current.iter().enumerate() {
            let pos = selection.head;
            let mut goal = self
                .carets
                .goal_columns
                .get(index)
                .copied()
                .unwrap_or(pos.column);
            let to = match key {
                Home if command => Position::document_start(),
                End if command => Position::document_end(&self.buffer),
                ArrowLeft if command && cfg!(target_os = "macos") => self.row_start_target(pos),
                ArrowRight if command && cfg!(target_os = "macos") => self.row_end_target(pos),
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
                ArrowUp | ArrowDown => {
                    let (target, next_goal) = self.vertical_target(pos, goal, key == ArrowDown);
                    goal = next_goal;
                    target
                }
                PageUp => movement::move_page_up(&self.buffer, pos, pages, goal),
                PageDown => movement::move_page_down(&self.buffer, pos, pages, goal),
                Home => self.row_start_target(pos),
                End => self.row_end_target(pos),
                _ => unreachable!(),
            };
            let moved = movement::extend_or_move(selection, to, self.modifiers.shift);
            goals.insert(moved.head.char, goal);
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
        } else {
            self.carets.goal_columns = self
                .cursors
                .cursors()
                .iter()
                .map(|s| goals.get(&s.head.char).copied().unwrap_or(s.head.column))
                .collect();
        }
        self.reveal_primary_cursor();
        true
    }
}

/// Letter a document shortcut names. A command chord whose logical key
/// winit could not identify still carries the physical key.
pub(crate) fn shortcut_letter(event: &lux_platform::event::KeyEvent) -> Option<String> {
    match &event.key {
        lux_platform::Key::Character(key) => Some(key.to_lowercase()),
        lux_platform::Key::Unidentified => event.physical.map(|key| key.chord_token()),
        _ => None,
    }
}

pub(super) fn key_event_to_chord(
    event: &lux_platform::event::KeyEvent,
    modifiers: &lux_platform::Modifiers,
) -> Option<lux_config::Chord> {
    let shortcut = modifiers.meta || modifiers.control || modifiers.alt;
    let key = match &event.key {
        lux_platform::Key::Character(_) | lux_platform::Key::Unidentified
            if shortcut && event.physical.is_some() =>
        {
            event.physical.unwrap().chord_token()
        }
        lux_platform::Key::Character(s) => s.to_lowercase(),
        lux_platform::Key::Named(named) => match named {
            NamedKey::Enter => "enter".to_string(),
            NamedKey::Tab => "tab".to_string(),
            NamedKey::Space => "space".to_string(),
            NamedKey::Backspace => "backspace".to_string(),
            NamedKey::Delete => "delete".to_string(),
            NamedKey::ArrowUp => "up".to_string(),
            NamedKey::ArrowDown => "down".to_string(),
            NamedKey::ArrowLeft => "left".to_string(),
            NamedKey::ArrowRight => "right".to_string(),
            NamedKey::Home => "home".to_string(),
            NamedKey::End => "end".to_string(),
            NamedKey::PageUp => "pageup".to_string(),
            NamedKey::PageDown => "pagedown".to_string(),
            NamedKey::Escape => "escape".to_string(),
            NamedKey::F(n) => format!("f{n}"),
            _ => return None,
        },
        _ => return None,
    };
    Some(lux_config::Chord {
        ctrl: modifiers.control,
        alt: modifiers.alt,
        shift: modifiers.shift,
        cmd: modifiers.meta,
        key,
    })
}
