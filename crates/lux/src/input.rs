//! Keyboard editing shared by the native event handler and integration tests.

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_core::{edit_ops, selection_ops};
use lux_platform::event::KeyEvent;
use lux_platform::{Clipboard, ElementState, Key, NamedKey};

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

fn key_event_to_chord(
    event: &KeyEvent,
    modifiers: &lux_platform::Modifiers,
) -> Option<lux_config::Chord> {
    let key = match &event.key {
        Key::Character(s) => s.to_lowercase(),
        Key::Named(named) => match named {
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

impl App {
    fn command_modifier(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.modifiers.meta
        } else {
            // Ctrl+Alt commonly represents AltGr on Windows/Linux. Treating
            // it as a command chord drops printable international input.
            self.modifiers.control && !self.modifiers.alt
        }
    }

    fn line_comment_token(&self) -> Option<&'static str> {
        let extension = self.documents.path.as_ref()?.extension()?.to_str()?;
        match extension.to_ascii_lowercase().as_str() {
            "rs" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "js" | "jsx" | "ts" | "tsx"
            | "java" | "kt" | "kts" | "swift" | "go" => Some("//"),
            "py" | "rb" | "sh" | "bash" | "zsh" | "toml" | "yaml" | "yml" => Some("#"),
            "sql" | "lua" => Some("--"),
            _ => None,
        }
    }

    pub(crate) fn reset_goal_columns(&mut self) {
        self.goal_columns = self
            .cursors
            .cursors()
            .iter()
            .map(|s| s.head.column)
            .collect();
        self.reveal_primary_cursor();
    }

    fn position_at_mouse(&mut self) -> Position {
        self.editor_view.hit_test(
            &self.buffer,
            &self.viewport,
            self.mouse.position.x as f32,
            self.mouse.position.y as f32,
            crate::app::TEXT_LEFT_PADDING * self.scale_factor,
        )
    }

    pub(crate) fn begin_mouse_selection(&mut self) {
        let position = self.position_at_mouse();
        let anchor = if self.modifiers.shift {
            self.cursors.primary().anchor
        } else {
            position
        };
        self.mouse.selection_anchor = Some(anchor);
        self.cursors = MultiCursor::new(Selection::new(anchor, position));
        self.snippet_session = None;
        self.reset_goal_columns();
    }

    pub(crate) fn update_mouse_selection(&mut self) {
        let Some(anchor) = self.mouse.selection_anchor else {
            return;
        };
        let position = self.position_at_mouse();
        self.cursors = MultiCursor::new(Selection::new(anchor, position));
        self.snippet_session = None;
        self.reset_goal_columns();
    }

    fn reveal_primary_cursor(&mut self) {
        let line_height = LINE_HEIGHT * self.scale_factor;
        let primary = self.cursors.primary().head;
        let y = primary.line as f32 * line_height;
        if y < self.viewport.scroll_y {
            self.viewport.scroll_y = y;
        } else if y + line_height > self.viewport.scroll_y + self.viewport.height {
            self.viewport.scroll_y = (y + line_height - self.viewport.height).max(0.0);
        }
        let x = self.editor_view.x_for_position(
            &self.buffer,
            primary,
            crate::app::TEXT_LEFT_PADDING * self.scale_factor,
        );
        let caret_width = 2.0 * self.scale_factor;
        if primary.column == 0 {
            self.viewport.scroll_x = 0.0;
        } else if x < self.viewport.scroll_x {
            self.viewport.scroll_x = x;
        } else if x + caret_width > self.viewport.scroll_x + self.viewport.width {
            self.viewport.scroll_x = (x + caret_width - self.viewport.width).max(0.0);
        }
        self.start = std::time::Instant::now();
        self.scheduler.invalidate_all();
    }

    pub(crate) fn insert_at_cursor(&mut self, text: &str) {
        if text.is_empty() || self.buffer.is_huge_file_mode() || self.editing_blocked() {
            return;
        }
        if let Some(session) = &mut self.snippet_session {
            let edits = session.update_current(&mut self.buffer, text);
            let before = vec![self.cursors.primary()];
            let after_sel = session.current_selection(&self.buffer);
            let transaction = lux_core::history::Transaction::new(edits, before, vec![after_sel]);
            self.history.record(transaction);
            self.cursors = MultiCursor::new(after_sel);
            self.reset_goal_columns();
            self.sync_syntax_on_edit();
            return;
        }
        self.replace_selections(self.cursors.clone(), text);
    }

    #[allow(dead_code)]
    pub(crate) fn expand_snippet(&mut self, template: &str) {
        if self.buffer.is_huge_file_mode() || self.editing_blocked() {
            return;
        }
        let at = self.cursors.primary().head.char;
        let (edit, session) = lux_core::snippets::insert(&mut self.buffer, at, template);
        let before = vec![self.cursors.primary()];
        let initial_sel = session.current_selection(&self.buffer);
        let transaction =
            lux_core::history::Transaction::new(vec![edit], before, vec![initial_sel]);
        self.history.record(transaction);
        self.cursors = MultiCursor::new(initial_sel);
        self.snippet_session = Some(session);
        self.reset_goal_columns();
        self.sync_syntax_on_edit();
    }

    fn replace_selections(&mut self, ranges: MultiCursor, text: &str) {
        self.apply_edit_operation(ranges, |buffer, selection| {
            edit_ops::insert_text(buffer, selection, text)
        });
    }

    fn apply_edit_operation(
        &mut self,
        ranges: MultiCursor,
        op: impl FnMut(
            &mut lux_core::buffer::Buffer,
            Selection,
        ) -> (Vec<lux_core::edit::Edit>, Selection),
    ) {
        if self.buffer.is_huge_file_mode() {
            return;
        }
        let before = with_primary_last(&self.cursors);
        let primary_index = ranges
            .cursors()
            .iter()
            .position(|s| *s == ranges.primary())
            .unwrap();
        let (mut transaction, after) = edit_ops::apply_to_each(&mut self.buffer, &ranges, op);
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
        self.sync_syntax_on_edit();
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

    #[cfg(test)]
    pub(crate) fn handle_key(&mut self, event: &KeyEvent) {
        self.handle_key_with_clipboard(event, None);
    }

    pub(crate) fn handle_key_with_clipboard(
        &mut self,
        event: &KeyEvent,
        clipboard: Option<&dyn Clipboard>,
    ) {
        if event.state != ElementState::Pressed || self.ime.is_composing() {
            return;
        }

        let platform = if cfg!(target_os = "macos") {
            lux_config::Platform::Mac
        } else if cfg!(target_os = "windows") {
            lux_config::Platform::Windows
        } else {
            lux_config::Platform::Linux
        };
        let keymap_cmd = key_event_to_chord(event, &self.modifiers).and_then(|chord| {
            let seq = lux_config::ChordSequence(vec![chord]);
            self.keymap.command_for(&seq, platform).map(str::to_owned)
        });

        if let Some(cmd) = keymap_cmd.as_deref() {
            match cmd {
                "copy" => {
                    self.copy_to_clipboard(clipboard);
                }
                "cut" if !self.editing_blocked() && !self.buffer.is_huge_file_mode() => {
                    if self.copy_to_clipboard(clipboard) {
                        self.replace_selections(self.cursors.clone(), "");
                    }
                }
                "paste" if !self.editing_blocked() && !self.buffer.is_huge_file_mode() => {
                    if let Some(clipboard) = clipboard {
                        match clipboard.read_text() {
                            Ok(text) => self.insert_at_cursor(&text),
                            Err(error) => self.documents.message = Some(error.to_string()),
                        }
                    }
                }
                "undo" if !self.buffer.is_huge_file_mode() => {
                    let restored = self.history.undo(&mut self.buffer);
                    if let Some(selections) = restored {
                        self.cursors = MultiCursor::from_cursors(selections);
                        self.reset_goal_columns();
                        self.sync_syntax_on_edit();
                    }
                }
                "redo" if !self.buffer.is_huge_file_mode() => {
                    let restored = self.history.redo(&mut self.buffer);
                    if let Some(selections) = restored {
                        self.cursors = MultiCursor::from_cursors(selections);
                        self.reset_goal_columns();
                        self.sync_syntax_on_edit();
                    }
                }
                "select_all" => {
                    self.cursors = MultiCursor::new(Selection::new(
                        Position::document_start(),
                        Position::document_end(&self.buffer),
                    ));
                    self.reset_goal_columns();
                }
                "duplicate_line" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::duplicate_line);
                }
                "select_next_occurrence" => {
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
                "delete_line" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::delete_line);
                }
                "join_lines" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::join_lines);
                }
                "indent" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::indent);
                }
                "dedent" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::dedent);
                }
                "toggle_comment" => {
                    if let Some(token) = self.line_comment_token() {
                        self.apply_edit_operation(self.cursors.clone(), |buffer, selection| {
                            edit_ops::toggle_line_comment(buffer, selection, token)
                        });
                    }
                }
                _ => {}
            }
            return;
        }

        let command = self.command_modifier();
        if command && let Key::Character(key) = &event.key {
            match key.to_lowercase().as_str() {
                "c" => {
                    self.copy_to_clipboard(clipboard);
                }
                "x" if !self.editing_blocked() && !self.buffer.is_huge_file_mode() => {
                    if self.copy_to_clipboard(clipboard) {
                        self.replace_selections(self.cursors.clone(), "");
                    }
                }
                "v" if !self.editing_blocked() && !self.buffer.is_huge_file_mode() => {
                    if let Some(clipboard) = clipboard {
                        match clipboard.read_text() {
                            Ok(text) => self.insert_at_cursor(&text),
                            Err(error) => self.documents.message = Some(error.to_string()),
                        }
                    }
                }
                "z" if !self.buffer.is_huge_file_mode() => {
                    let restored = if self.modifiers.shift {
                        self.history.redo(&mut self.buffer)
                    } else {
                        self.history.undo(&mut self.buffer)
                    };
                    if let Some(selections) = restored {
                        self.cursors = MultiCursor::from_cursors(selections);
                        self.reset_goal_columns();
                        self.sync_syntax_on_edit();
                    }
                }
                "a" => {
                    self.cursors = MultiCursor::new(Selection::new(
                        Position::document_start(),
                        Position::document_end(&self.buffer),
                    ));
                    self.reset_goal_columns();
                }
                "d" if self.modifiers.shift => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::duplicate_line);
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
                "k" if self.modifiers.shift => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::delete_line);
                }
                "j" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::join_lines);
                }
                "]" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::indent);
                }
                "[" => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::dedent);
                }
                "/" => {
                    if let Some(token) = self.line_comment_token() {
                        self.apply_edit_operation(self.cursors.clone(), |buffer, selection| {
                            edit_ops::toggle_line_comment(buffer, selection, token)
                        });
                    }
                }
                _ => {}
            }
            return; // Unbound command chords must never type their text.
        }
        if self.editing_blocked() {
            return;
        }
        if let Key::Named(key) = event.key {
            let word = if cfg!(target_os = "macos") {
                self.modifiers.alt
            } else {
                self.modifiers.control && !self.modifiers.alt
            };
            if word && matches!(key, NamedKey::Backspace | NamedKey::Delete) {
                let op = if key == NamedKey::Backspace {
                    edit_ops::delete_word_backward
                } else {
                    edit_ops::delete_word_forward
                };
                self.apply_edit_operation(self.cursors.clone(), op);
                return;
            }
            if self.modifiers.alt && matches!(key, NamedKey::ArrowUp | NamedKey::ArrowDown) {
                let op = if key == NamedKey::ArrowUp {
                    edit_ops::move_line_up
                } else {
                    edit_ops::move_line_down
                };
                self.apply_edit_operation(self.cursors.clone(), op);
                return;
            }
            if self.move_cursors(key) {
                self.snippet_session = None;
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
                NamedKey::Tab if self.modifiers.shift => {
                    if let Some(session) = &mut self.snippet_session
                        && let Some(sel) = session.prev(&self.buffer)
                    {
                        self.cursors = MultiCursor::new(sel);
                        self.reset_goal_columns();
                        return;
                    }
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::dedent);
                }
                NamedKey::Tab if self.snippet_session.is_some() => {
                    if let Some(session) = &mut self.snippet_session {
                        if let Some(sel) = session.next(&self.buffer) {
                            self.cursors = MultiCursor::new(sel);
                            self.reset_goal_columns();
                        } else {
                            self.snippet_session = None;
                        }
                        return;
                    }
                }
                NamedKey::Tab if self.cursors.cursors().iter().any(|s| !s.is_empty()) => {
                    self.apply_edit_operation(self.cursors.clone(), edit_ops::indent);
                }
                NamedKey::Tab => self.insert_at_cursor("\t"),
                NamedKey::Space => self.insert_at_cursor(" "),
                NamedKey::Escape => {
                    if self.snippet_session.take().is_some() {
                        return;
                    }
                    self.cursors = MultiCursor::new(self.cursors.primary().collapse_to_head());
                    self.chrome.focus.escape_to_editor();
                    self.reset_goal_columns();
                }
                _ => {}
            }
            return;
        }
        if !self.command_modifier()
            && let Some(text) = &event.text
        {
            self.insert_at_cursor(text);
        }
    }

    /// Copies every non-empty selection in document order. Multiple
    /// selections use the document's own line ending so pasting into a
    /// single cursor remains lossless and platform-consistent.
    fn copy_to_clipboard(&mut self, clipboard: Option<&dyn Clipboard>) -> bool {
        let Some(clipboard) = clipboard else {
            return false;
        };
        let pieces: Vec<_> = self
            .cursors
            .cursors()
            .iter()
            .filter(|selection| !selection.is_empty())
            .map(|selection| self.buffer.text_range(selection.char_range()))
            .collect();
        if pieces.is_empty() {
            return false;
        }
        let text = pieces.join(self.buffer.eol().as_str());
        match clipboard.write_text(&text) {
            Ok(()) => true,
            Err(error) => {
                self.documents.message = Some(error.to_string());
                false
            }
        }
    }
}

mod motion;

#[cfg(test)]
mod tests;
