//! Text stored in a panel field, edited in place.
//!
//! Command+C/V/X/A/Z (Control on other platforms) apply to this text while
//! the panel is open. The document keymap does not see those chords.

use lux_platform::event::KeyEvent;
use lux_platform::{Clipboard, ElementState, Ime, Key, PhysicalKey};

use crate::app::App;

#[derive(Clone, Debug)]
pub(crate) struct QueryField {
    text: String,
    caret: usize,
    anchor: usize,
    undo: Vec<String>,
    redo: Vec<String>,
    composing: bool,
}

impl Default for QueryField {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryField {
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
            caret: 0,
            anchor: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            composing: false,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.chars().count();
        Self {
            text,
            caret,
            anchor: caret,
            undo: Vec::new(),
            redo: Vec::new(),
            composing: false,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    pub(crate) fn push_str(&mut self, extra: &str) {
        if extra.is_empty() {
            return;
        }
        self.snapshot();
        self.delete_selection();
        let byte = self.byte_at(self.caret);
        self.text.insert_str(byte, extra);
        self.caret += extra.chars().count();
        self.anchor = self.caret;
    }

    pub(crate) fn pop(&mut self) -> Option<char> {
        if self.caret == 0 && self.anchor == 0 {
            return None;
        }
        self.snapshot();
        if self.caret != self.anchor {
            self.delete_selection();
            return None;
        }
        self.caret -= 1;
        let start = self.byte_at(self.caret);
        let ch = self.text[start..].chars().next().unwrap();
        self.text.remove(start);
        self.anchor = self.caret;
        Some(ch)
    }

    pub(crate) fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.chars().count();
    }

    /// Copies the selection, or the whole field when nothing is selected,
    /// then deletes that text.
    pub(crate) fn cut(&mut self) -> String {
        if self.caret == self.anchor {
            self.select_all();
        }
        self.snapshot();
        let text = self.selected_text();
        self.delete_selection();
        text
    }

    pub(crate) fn selected_text(&self) -> String {
        let (start, end) = self.selection_range();
        if start == end {
            return self.text.clone();
        }
        self.text.chars().skip(start).take(end - start).collect()
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.text.clone());
        self.text = prev;
        self.caret = self.text.chars().count();
        self.anchor = self.caret;
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.text.clone());
        self.text = next;
        self.caret = self.text.chars().count();
        self.anchor = self.caret;
        true
    }

    pub(crate) fn set_composing(&mut self, composing: bool) {
        self.composing = composing;
    }

    pub(crate) fn is_composing(&self) -> bool {
        self.composing
    }

    fn snapshot(&mut self) {
        if self.undo.last().is_some_and(|s| s == &self.text) {
            return;
        }
        self.undo.push(self.text.clone());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn selection_range(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    fn delete_selection(&mut self) {
        let (start, end) = self.selection_range();
        if start == end {
            return;
        }
        let byte_start = self.byte_at(start);
        let byte_end = self.byte_at(end);
        self.text.replace_range(byte_start..byte_end, "");
        self.caret = start;
        self.anchor = start;
    }

    fn byte_at(&self, char_index: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(self.text.len())
    }
}

impl std::ops::Deref for QueryField {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

impl PartialEq<str> for QueryField {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for QueryField {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<String> for QueryField {
    fn eq(&self, other: &String) -> bool {
        &self.text == other
    }
}

impl std::fmt::Display for QueryField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

pub(crate) fn shortcut_letter(event: &KeyEvent) -> Option<char> {
    if let Some(PhysicalKey::Letter(c)) = event.physical {
        return Some(c.to_ascii_lowercase());
    }
    let Key::Character(text) = &event.key else {
        return None;
    };
    let mut chars = text.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || !ch.is_ascii_alphabetic() {
        return None;
    }
    Some(ch.to_ascii_lowercase())
}

/// Command or Control. Those chords are shortcuts, not panel text.
/// Alt alone still types (an Option glyph). Ctrl+Alt is AltGr and is left
/// for the keymap rather than inserted as a panel character.
pub(crate) fn is_shortcut_chord(app: &App) -> bool {
    app.modifiers.meta || app.modifiers.control
}

fn command_held(app: &App) -> bool {
    if cfg!(target_os = "macos") {
        app.modifiers.meta && !app.modifiers.control
    } else {
        app.modifiers.control && !app.modifiers.meta
    }
}

fn text_field(app: &mut App) -> Option<&mut QueryField> {
    if let Some(palette) = app.ui.command_palette.as_mut() {
        return Some(&mut palette.query);
    }
    if let Some(picker) = app.ui.language_picker.as_mut() {
        return Some(&mut picker.query);
    }
    if let Some(picker) = app.ui.build.picker.as_mut() {
        return Some(&mut picker.query);
    }
    if let Some(picker) = app.ui.project.picker.as_mut() {
        return Some(&mut picker.query);
    }
    if let Some(picker) = app.ui.theme_picker.as_mut() {
        return Some(&mut picker.query);
    }
    if let Some(snippets) = app.ui.snippets.picker.as_mut() {
        return Some(&mut snippets.query);
    }
    if let Some(goto) = app.ui.goto_anything.as_mut() {
        return Some(&mut goto.query);
    }
    if let Some(search) = app.ui.search_bar.as_mut() {
        return Some(match search.focused_field {
            crate::search::SearchField::Find => &mut search.find_query,
            crate::search::SearchField::Replace => &mut search.replace_text,
        });
    }
    if let Some(search) = app.ui.project_search.as_mut() {
        return Some(match search.focused_field {
            crate::project_search::SearchField::Find => &mut search.find_query,
            crate::project_search::SearchField::Where => &mut search.where_query,
            crate::project_search::SearchField::Replace => &mut search.replace_query,
        });
    }
    None
}

pub(crate) fn field_is_composing(app: &App) -> bool {
    composing_flag(app)
}

fn composing_flag(app: &App) -> bool {
    if app
        .ui
        .command_palette
        .as_ref()
        .is_some_and(|p| p.query.is_composing())
    {
        return true;
    }
    if app
        .ui
        .language_picker
        .as_ref()
        .is_some_and(|p| p.query.is_composing())
    {
        return true;
    }
    if app
        .ui
        .build
        .picker
        .as_ref()
        .is_some_and(|p| p.query.is_composing())
    {
        return true;
    }
    if app
        .ui
        .project
        .picker
        .as_ref()
        .is_some_and(|p| p.query.is_composing())
    {
        return true;
    }
    if app
        .ui
        .theme_picker
        .as_ref()
        .is_some_and(|p| p.query.is_composing())
    {
        return true;
    }
    if app
        .ui
        .snippets
        .picker
        .as_ref()
        .is_some_and(|p| p.query.is_composing())
    {
        return true;
    }
    if app
        .ui
        .goto_anything
        .as_ref()
        .is_some_and(|p| p.query.is_composing())
    {
        return true;
    }
    if app
        .ui
        .search_bar
        .as_ref()
        .is_some_and(|s| s.find_query.is_composing() || s.replace_text.is_composing())
    {
        return true;
    }
    app.ui.project_search.as_ref().is_some_and(|s| {
        s.find_query.is_composing()
            || s.where_query.is_composing()
            || s.replace_query.is_composing()
    })
}

pub(crate) fn panel_blocks_document_ime(app: &App) -> bool {
    app.ui.command_palette.is_some()
        || app.ui.language_picker.is_some()
        || app.ui.build.picker.is_some()
        || app.ui.project.picker.is_some()
        || app.ui.theme_picker.is_some()
        || app.ui.snippets.picker.is_some()
        || app.ui.goto_anything.is_some()
        || app.ui.search_bar.is_some()
        || app.ui.project_search.is_some()
        || app.ui.dialogs.conflict_prompt.is_some()
        || app.ui.lsp.completion_popup.is_some()
}

pub(crate) fn route_ime(app: &mut App, event: &Ime) -> bool {
    if !panel_blocks_document_ime(app) {
        return false;
    }
    match event {
        Ime::Enabled => {}
        Ime::Disabled => {
            if let Some(field) = text_field(app) {
                field.set_composing(false);
            }
        }
        Ime::Preedit { text, .. } => {
            if let Some(field) = text_field(app) {
                field.set_composing(!text.is_empty());
            }
        }
        Ime::Commit(text) => {
            if let Some(field) = text_field(app) {
                field.set_composing(false);
                field.push_str(text);
            }
            sync_search(app);
        }
    }
    app.scheduler.invalidate_all();
    true
}

pub(crate) fn edit_focused_field(
    app: &mut App,
    event: &KeyEvent,
    clipboard: &dyn Clipboard,
) -> bool {
    if event.state != ElementState::Pressed || app.modifiers.alt || !command_held(app) {
        return false;
    }
    if app.modifiers.shift && shortcut_letter(event) != Some('z') {
        return false;
    }
    let Some(letter) = shortcut_letter(event) else {
        return false;
    };
    if !matches!(letter, 'a' | 'c' | 'x' | 'v' | 'z') || !has_text_field(app) {
        return false;
    }
    match letter {
        'c' => {
            let Some(field) = text_field(app) else {
                return false;
            };
            let _ = clipboard.write_text(&field.selected_text());
        }
        'x' => {
            let Some(field) = text_field(app) else {
                return false;
            };
            let text = field.cut();
            let _ = clipboard.write_text(&text);
        }
        'v' => {
            let Ok(pasted) = clipboard.read_text() else {
                return false;
            };
            let Some(field) = text_field(app) else {
                return false;
            };
            field.push_str(&pasted);
        }
        'a' => {
            let Some(field) = text_field(app) else {
                return false;
            };
            field.select_all();
        }
        'z' if app.modifiers.shift => {
            let Some(field) = text_field(app) else {
                return false;
            };
            field.redo();
        }
        'z' => {
            let Some(field) = text_field(app) else {
                return false;
            };
            field.undo();
        }
        _ => return false,
    }
    sync_search(app);
    app.scheduler.invalidate_all();
    true
}

fn has_text_field(app: &App) -> bool {
    app.ui.command_palette.is_some()
        || app.ui.language_picker.is_some()
        || app.ui.build.picker.is_some()
        || app.ui.project.picker.is_some()
        || app.ui.theme_picker.is_some()
        || app.ui.snippets.picker.is_some()
        || app.ui.goto_anything.is_some()
        || app.ui.search_bar.is_some()
        || app.ui.project_search.is_some()
}

fn sync_search(app: &mut App) {
    if app.ui.search_bar.is_some() {
        let buffer = app.buffer.clone();
        if let Some(search) = app.ui.search_bar.as_mut() {
            search.sync_query(&buffer);
        }
    }
}

/// Regex / case / word toggles. Only Option+Command (Alt+Ctrl elsewhere)
/// plus the physical letter. A Spanish `ç` is text, not the case toggle.
pub(crate) fn search_option_toggle(app: &mut App, event: &KeyEvent) -> bool {
    if app.ui.search_bar.is_none() || !app.modifiers.alt || !command_held(app) {
        return false;
    }
    let Some(letter) = shortcut_letter(event) else {
        return false;
    };
    let buffer = app.buffer.clone();
    let Some(search) = app.ui.search_bar.as_mut() else {
        return false;
    };
    let handled = match letter {
        'r' => {
            search.toggle_regex(&buffer);
            true
        }
        'c' => {
            search.toggle_case(&buffer);
            true
        }
        'w' => {
            search.toggle_word(&buffer);
            true
        }
        _ => false,
    };
    if handled {
        app.scheduler.invalidate_all();
    }
    handled
}
