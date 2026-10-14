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
pub(super) fn with_primary_last(cursors: &MultiCursor) -> Vec<Selection> {
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
    fn command_modifier(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.modifiers.meta
        } else {
            // Ctrl+Alt commonly represents AltGr on Windows/Linux. Treating
            // it as a command chord drops printable international input.
            self.modifiers.control && !self.modifiers.alt
        }
    }

    pub(crate) fn line_comment_token(&self) -> Option<&'static str> {
        if let Some(lang) = self.documents.language() {
            use lux_syntax::LanguageId::*;
            return match lang {
                Rust | TypeScript | Tsx | JavaScript | C | Cpp | Java | Kotlin | Swift | Go => {
                    Some("//")
                }
                Python | Bash | Toml | Yaml => Some("#"),
                _ => None,
            };
        }
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

    pub(crate) fn position_at_mouse(&mut self) -> Position {
        let local_x = (self.mouse.position.x as f32 - self.sidebar_width()).max(0.0);
        let local_y = (self.mouse.position.y as f32 - self.tab_bar_height()).max(0.0);
        let digits = self.buffer.len_lines().max(1).to_string().len();
        let gutter_w = self.editor_view.reference_width(&"0".repeat(digits));
        let left_padding = self.text_left() - self.sidebar_width() - gutter_w;
        self.editor_view.hit_test(
            &self.buffer,
            &self.viewport,
            local_x,
            local_y,
            left_padding,
            Some(&self.fold_state),
        )
    }

    pub(crate) fn begin_mouse_selection(&mut self) {
        let position = self.position_at_mouse();
        let local_x = (self.mouse.position.x as f32 - self.sidebar_width()).max(0.0);
        if !self.is_distraction_free() && local_x <= 16.0 * self.scale_factor {
            self.toggle_bookmark_at_line(position.line);
            return;
        }
        if self.fold_state.is_line_folded(position.line) {
            let digits = self.buffer.len_lines().max(1).to_string().len();
            let gutter_w = self.editor_view.reference_width(&"0".repeat(digits));
            let text_left = gutter_w + 22.0 * self.scale_factor;
            let line_content = self.buffer.line_content(position.line).unwrap_or_default();
            let trimmed = line_content.trim_end_matches(['\r', '\n']);
            let text_w = self.editor_view.reference_width(trimmed);
            let pill_x = text_left + text_w - self.viewport.scroll_x + 6.0 * self.scale_factor;
            let pill_w = 26.0 * self.scale_factor;
            if local_x >= pill_x - 6.0 * self.scale_factor
                && local_x <= pill_x + pill_w + 6.0 * self.scale_factor
            {
                self.unfold_at_line(position.line);
                return;
            }
        }
        let anchor = if self.modifiers.shift {
            self.cursors.primary().anchor
        } else {
            position
        };
        self.mouse.selection_anchor = Some(anchor);
        if self.modifiers.alt {
            let mut cursors = self.cursors.cursors().to_vec();
            cursors.push(Selection::new(anchor, position));
            self.cursors = MultiCursor::from_cursors(cursors);
        } else {
            self.cursors = MultiCursor::new(Selection::new(anchor, position));
        }
        self.snippet_session = None;
        self.reset_goal_columns();
    }

    pub(crate) fn update_mouse_selection(&mut self) {
        let Some(anchor) = self.mouse.selection_anchor else {
            return;
        };
        let position = self.position_at_mouse();
        if self.modifiers.alt {
            self.cursors = selection_ops::box_select(&self.buffer, anchor, position);
        } else {
            self.cursors = MultiCursor::new(Selection::new(anchor, position));
        }
        self.snippet_session = None;
        self.reset_goal_columns();
    }

    pub(crate) fn reveal_primary_cursor(&mut self) {
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
        if crate::snippets::dispatch::handle_snippet_text_input(self, text) {
            return;
        }
        crate::macros::dispatch::record_text_input(self, text);

        // Parallel paste: when multi-cursor count matches lines count, paste 1 line per cursor.
        // Note: apply_to_each processes cursors back-to-front (highest line/offset first),
        // so we count down line_idx from lines.len() - 1 to distribute lines in document order.
        let lines: Vec<&str> = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect();
        if self.cursors.count() > 1 && lines.len() == self.cursors.count() {
            let mut line_idx = lines.len();
            self.apply_edit_operation(self.cursors.clone(), |buffer, selection| {
                line_idx = line_idx.saturating_sub(1);
                let piece = lines.get(line_idx).copied().unwrap_or("");
                edit_ops::insert_text(buffer, selection, piece)
            });
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

    pub(crate) fn replace_selections(&mut self, ranges: MultiCursor, text: &str) {
        self.apply_edit_operation(ranges, |buffer, selection| {
            edit_ops::insert_text(buffer, selection, text)
        });
    }

    pub(crate) fn apply_edit_operation(
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

    pub(crate) fn delete_at_cursors(&mut self, backward: bool) {
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

        if dispatch::dispatch_keymap_or_chord(self, event, clipboard) {
            return;
        }

        if self.modifiers.alt
            && !self.command_modifier()
            && let Key::Character(key) = &event.key
            && (key.eq_ignore_ascii_case("z") || key == "Ω")
        {
            self.toggle_word_wrap();
            #[cfg(target_os = "macos")]
            crate::menu::macos::update_view_checkmarks(self);
            return;
        }

        if (event.key == Key::Named(NamedKey::F(11)) && self.modifiers.shift)
            || (self.command_modifier()
                && self.modifiers.control
                && self.modifiers.shift
                && matches!(&event.key, Key::Character(k) if k.eq_ignore_ascii_case("f")))
        {
            self.toggle_distraction_free();
            #[cfg(target_os = "macos")]
            crate::menu::macos::update_view_checkmarks(self);
            return;
        }

        let command = self.command_modifier();
        if command
            && self.modifiers.alt
            && let Key::Character(key) = &event.key
        {
            if key == "2" || key == "™" || key == "€" {
                if self.modifiers.shift {
                    self.split_editor_down();
                } else {
                    self.split_editor_right();
                }
                return;
            }
            if (key.eq_ignore_ascii_case("w") || key == "∑") && self.close_active_split() {
                return;
            }
        }
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
                "j" => self.apply_edit_operation(self.cursors.clone(), edit_ops::join_lines),
                "]" => self.apply_edit_operation(self.cursors.clone(), edit_ops::indent),
                "[" => self.apply_edit_operation(self.cursors.clone(), edit_ops::dedent),
                "/" => {
                    if let Some(token) = self.line_comment_token() {
                        self.apply_edit_operation(self.cursors.clone(), |buffer, selection| {
                            edit_ops::toggle_line_comment(buffer, selection, token)
                        });
                    }
                }
                "p" if self.modifiers.shift => self.toggle_command_palette(),
                "p" => self.toggle_goto_anything(),
                "r" if self.modifiers.shift => self.open_goto_project_symbols(),
                "r" => self.open_goto_anything("@"),
                "f" if self.modifiers.shift => self.open_project_find(),
                "f" => self.open_find(),
                "h" if self.modifiers.shift => self.open_project_replace(),
                "h" => self.open_replace(),
                "b" => self.toggle_sidebar(),
                "m" if self.modifiers.shift => self.open_language_picker(),
                _ => {}
            }
            return; // Unbound command chords must never type their text.
        }
        if self.editing_blocked() {
            return;
        }
        if let Key::Named(key) = event.key {
            if (matches!(key, NamedKey::F(6))
                || (command && self.modifiers.alt && key == NamedKey::ArrowRight))
                && self.focus_next_split()
            {
                return;
            }
            let word = if cfg!(target_os = "macos") {
                self.modifiers.alt
            } else {
                self.modifiers.control && !self.modifiers.alt
            };
            if word && matches!(key, NamedKey::Backspace | NamedKey::Delete) {
                let forward = key == NamedKey::Delete;
                crate::macros::dispatch::record_word_delete(self, forward);
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
                crate::macros::dispatch::record_move(
                    self,
                    key,
                    word,
                    self.command_modifier(),
                    self.modifiers.shift,
                );
                self.snippet_session = None;
                return;
            }
            if self.modifiers.control || self.modifiers.meta {
                return;
            }
            match key {
                NamedKey::Backspace => {
                    crate::macros::dispatch::record_backspace(self);
                    if !crate::snippets::dispatch::handle_snippet_backspace(self)
                        && !autopair::handle_backspace(self)
                    {
                        self.delete_at_cursors(true);
                    }
                }
                NamedKey::Delete => {
                    crate::macros::dispatch::record_delete(self);
                    self.delete_at_cursors(false);
                }
                NamedKey::Enter => {
                    crate::macros::dispatch::record_enter(self);
                    self.insert_newline();
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
                NamedKey::Tab => {
                    if !crate::snippets::dispatch::try_expand_snippet_trigger(self) {
                        self.insert_tab();
                    }
                }
                NamedKey::Space => self.insert_at_cursor(" "),
                NamedKey::Escape => {
                    if self.snippet_session.take().is_some() {
                        return;
                    }
                    if self.cursors.cursors().len() > 1 || !self.cursors.primary().is_empty() {
                        self.cursors = MultiCursor::new(self.cursors.primary().collapse_to_head());
                        self.chrome.focus.escape_to_editor();
                        self.reset_goal_columns();
                    } else if self.is_distraction_free() {
                        self.exit_distraction_free();
                        #[cfg(target_os = "macos")]
                        crate::menu::macos::update_view_checkmarks(self);
                    } else {
                        self.cursors = MultiCursor::new(self.cursors.primary().collapse_to_head());
                        self.chrome.focus.escape_to_editor();
                        self.reset_goal_columns();
                    }
                }
                _ => {}
            }
            return;
        }
        if !self.command_modifier()
            && let Some(text) = &event.text
        {
            let handled = text.chars().count() == 1
                && text
                    .chars()
                    .next()
                    .is_some_and(|ch| autopair::handle_char_typed(self, ch));
            if !handled {
                self.insert_at_cursor(text);
            }
        }
    }

    pub(crate) fn insert_tab(&mut self) {
        indent::insert_tab(self);
    }

    pub(crate) fn insert_newline(&mut self) {
        indent::insert_newline(self);
    }

    pub(crate) fn jump_to_matching_bracket(&mut self) {
        autopair::jump_to_matching_bracket(self);
    }
}

mod autopair;
pub(crate) mod dispatch;
mod indent;
mod motion;

#[cfg(test)]
mod autopair_tests;
#[cfg(test)]
mod bookmark_tests;
#[cfg(test)]
mod build_tests;
#[cfg(test)]
mod distraction_free_tests;
#[cfg(test)]
mod find_in_files_tests;
#[cfg(test)]
mod folding_tests;
#[cfg(test)]
mod goto_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod macro_tests;
#[cfg(test)]
mod occurrence_tests;
#[cfg(test)]
mod project_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod snippet_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod theme_tests;
