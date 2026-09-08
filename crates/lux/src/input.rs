//! Keyboard editing shared by the native event handler and integration tests.

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_core::{edit_ops, movement, selection_ops};
use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey};

use crate::app::{App, LINE_HEIGHT};

// History stores ordered vectors. Put the primary last so reconstructing a
// MultiCursor restores the active selection as well as its text ranges.
fn with_primary_last(cursors: &MultiCursor) -> Vec<Selection> {
    let primary = cursors.primary();
    let mut selections: Vec<_> = cursors
        .cursors()
        .iter()
        .copied()
        .filter(|s| *s != primary)
        .collect();
    selections.push(primary);
    selections
}

impl App {
    pub(crate) fn reset_goal_columns(&mut self) {
        self.goal_columns = self
            .cursors
            .cursors()
            .iter()
            .map(|s| s.head.column)
            .collect();
        self.reveal_primary_cursor();
    }

    fn reveal_primary_cursor(&mut self) {
        let line_height = LINE_HEIGHT * self.scale_factor;
        let y = self.cursors.primary().head.line as f32 * line_height;
        if y < self.viewport.scroll_y {
            self.viewport.scroll_y = y;
        } else if y + line_height > self.viewport.scroll_y + self.viewport.height {
            self.viewport.scroll_y = (y + line_height - self.viewport.height).max(0.0);
        }
        self.start = std::time::Instant::now();
        self.scheduler.invalidate_all();
    }

    pub(crate) fn insert_at_cursor(&mut self, text: &str) {
        if text.is_empty() || self.buffer.is_huge_file_mode() || self.editing_blocked() {
            return;
        }
        self.replace_selections(self.cursors.clone(), text);
    }

    fn replace_selections(&mut self, ranges: MultiCursor, text: &str) {
        if self.buffer.is_huge_file_mode() {
            return;
        }
        let before = with_primary_last(&self.cursors);
        let primary_index = ranges
            .cursors()
            .iter()
            .position(|s| *s == ranges.primary())
            .unwrap();
        let (mut transaction, after) =
            edit_ops::apply_to_each(&mut self.buffer, &ranges, |buffer, selection| {
                edit_ops::insert_text(buffer, selection, text)
            });
        if transaction.edits.is_empty() {
            return;
        }
        let mut selections = transaction.selections_after.clone();
        let primary = selections.remove(primary_index);
        selections.push(primary);
        self.cursors = MultiCursor::from_cursors(selections);
        transaction.selections_before = before;
        transaction.selections_after = with_primary_last(&self.cursors);
        self.history.record(transaction);
        // `after` is normalized, but its default primary is document-last.
        debug_assert_eq!(self.cursors.cursors(), after.cursors());
        self.reset_goal_columns();
    }

    fn delete_at_cursors(&mut self, backward: bool) {
        let selections = with_primary_last(&self.cursors)
            .into_iter()
            .map(|selection| {
                if !selection.is_empty() {
                    return selection;
                }
                let head = selection.head;
                let to = if backward {
                    head.prev_grapheme(&self.buffer)
                } else {
                    head.next_grapheme(&self.buffer)
                };
                Selection::new(head, to)
            })
            .collect();
        // Merge overlapping deletion ranges before applying edits so a shared
        // grapheme cannot be deleted twice by adjacent cursors.
        self.replace_selections(MultiCursor::from_cursors(selections), "");
    }

    pub(crate) fn handle_key(&mut self, event: &KeyEvent) {
        if event.state != ElementState::Pressed || self.ime.is_composing() || self.editing_blocked()
        {
            return;
        }
        let command = if cfg!(target_os = "macos") {
            self.modifiers.meta
        } else {
            self.modifiers.control
        };
        if command && let Key::Character(key) = &event.key {
            match key.to_lowercase().as_str() {
                "z" if !self.buffer.is_huge_file_mode() => {
                    let restored = if self.modifiers.shift {
                        self.history.redo(&mut self.buffer)
                    } else {
                        self.history.undo(&mut self.buffer)
                    };
                    if let Some(selections) = restored {
                        self.cursors = MultiCursor::from_cursors(selections);
                        self.reset_goal_columns();
                    }
                }
                "a" => {
                    self.cursors = MultiCursor::new(Selection::new(
                        Position::document_start(),
                        Position::document_end(&self.buffer),
                    ));
                    self.reset_goal_columns();
                }
                "d" => {
                    if self.cursors.primary().is_empty() {
                        let selections = with_primary_last(&self.cursors)
                            .into_iter()
                            .map(|s| selection_ops::expand_to_word(&self.buffer, s))
                            .collect();
                        self.cursors = MultiCursor::from_cursors(selections);
                    } else {
                        self.cursors =
                            selection_ops::select_next_occurrence(&self.buffer, &self.cursors);
                    }
                    self.reset_goal_columns();
                }
                _ => {}
            }
            return; // Unbound command chords must never type their text.
        }
        if let Key::Named(key) = event.key {
            if self.move_cursors(key) {
                return;
            }
            if self.modifiers.control || self.modifiers.meta {
                return;
            }
            match key {
                NamedKey::Backspace => self.delete_at_cursors(true),
                NamedKey::Delete => self.delete_at_cursors(false),
                NamedKey::Enter => {
                    let eol = self.buffer.eol().as_str().to_owned();
                    self.insert_at_cursor(&eol);
                }
                NamedKey::Tab => self.insert_at_cursor("\t"),
                NamedKey::Space => self.insert_at_cursor(" "),
                NamedKey::Escape => {
                    self.cursors = MultiCursor::new(self.cursors.primary().collapse_to_head());
                    self.chrome.focus.escape_to_editor();
                    self.reset_goal_columns();
                }
                _ => {}
            }
            return;
        }
        if !self.modifiers.control
            && !self.modifiers.meta
            && let Some(text) = &event.text
        {
            self.insert_at_cursor(text);
        }
    }

    fn move_cursors(&mut self, key: NamedKey) -> bool {
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
        let command = if cfg!(target_os = "macos") {
            self.modifiers.meta
        } else {
            self.modifiers.control
        };
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

#[cfg(test)]
mod tests;
