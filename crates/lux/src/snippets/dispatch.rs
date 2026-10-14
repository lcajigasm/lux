//! Dispatching snippet expansion on Tab, interactive editing, and scope resolution.

use super::picker::SnippetPickerState;
use super::registry::Snippet;
use crate::app::App;
use lux_core::edit::Edit;
use lux_core::history::Transaction;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_syntax::LanguageId;

/// Detects the TextMate scope selector for the currently active document.
pub(crate) fn current_document_scope(app: &App) -> Option<&'static str> {
    if let Some(syntax) = &app.syntax
        && let Some(doc) = &syntax.document
    {
        return current_document_scope_for_lang(doc.language());
    }

    if let Some(path) = &app.documents.path
        && let Some(ext) = path.extension().and_then(|e| e.to_str())
        && let Some(lang) = LanguageId::from_extension(ext)
    {
        return current_document_scope_for_lang(lang);
    }

    None
}

fn current_document_scope_for_lang(lang: LanguageId) -> Option<&'static str> {
    Some(match lang {
        LanguageId::Rust => "source.rust",
        LanguageId::TypeScript => "source.ts",
        LanguageId::Tsx => "source.tsx",
        LanguageId::JavaScript => "source.js",
        LanguageId::Python => "source.python",
        LanguageId::Go => "source.go",
        LanguageId::C => "source.c",
        LanguageId::Cpp => "source.cpp",
        LanguageId::Java => "source.java",
        LanguageId::Kotlin => "source.kotlin",
        LanguageId::Swift => "source.swift",
        LanguageId::Html => "text.html",
        LanguageId::Css => "source.css",
        LanguageId::Json => "source.json",
        LanguageId::Yaml => "source.yaml",
        LanguageId::Markdown => "text.markdown",
        LanguageId::Bash => "source.shell",
        LanguageId::Toml => "source.toml",
    })
}

/// Attempts to expand a snippet trigger preceding the cursor when Tab is pressed.
/// Returns true if a snippet was found, the trigger deleted, and the snippet expanded.
pub(crate) fn try_expand_snippet_trigger(app: &mut App) -> bool {
    let primary = app.cursors.primary();
    if !primary.is_empty() {
        return false;
    }

    let cursor_char = primary.head.char;
    let cursor_line = primary.head.line;
    let line_text = app.buffer.line_content(cursor_line).unwrap_or_default();
    let col = primary.head.column;

    let chars_before: Vec<char> = line_text.chars().take(col).collect();
    if chars_before.is_empty() {
        return false;
    }

    // Scan backwards from cursor for word/trigger characters
    let mut trigger_chars = Vec::new();
    for &ch in chars_before.iter().rev() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '!' {
            trigger_chars.push(ch);
        } else {
            break;
        }
    }

    if trigger_chars.is_empty() {
        return false;
    }
    trigger_chars.reverse();
    let trigger_word: String = trigger_chars.into_iter().collect();

    let scope = current_document_scope(app);
    let snippet_content = {
        let Some(snippet) = app
            .ui
            .snippets
            .registry
            .find_by_trigger(&trigger_word, scope)
        else {
            return false;
        };
        snippet.content.clone()
    };

    // Delete trigger word from buffer
    let trigger_len = trigger_word.chars().count();
    let trigger_start = cursor_char.saturating_sub(trigger_len);
    let delete_edit = Edit::delete(&app.buffer, trigger_start, trigger_len);
    delete_edit.apply(&mut app.buffer);

    let before_sel = primary;
    let after_trigger_delete = Position::from_char(&app.buffer, trigger_start);
    app.cursors = MultiCursor::new(Selection::cursor(after_trigger_delete));

    // Now expand the snippet template at the trigger start
    let (expand_edit, session) =
        lux_core::snippets::insert(&mut app.buffer, trigger_start, &snippet_content);
    let initial_sel = session.current_selection(&app.buffer);

    let combined_edits = vec![delete_edit, expand_edit];
    let transaction = Transaction::new(combined_edits, vec![before_sel], vec![initial_sel]);
    app.history.record(transaction);

    app.cursors = MultiCursor::new(initial_sel);
    app.snippet_session = Some(session);
    app.reset_goal_columns();
    app.sync_syntax_on_edit();

    true
}

/// Handles character typing inside an active snippet session.
pub(crate) fn handle_snippet_text_input(app: &mut App, text: &str) -> bool {
    let Some(session) = app.snippet_session.as_mut() else {
        return false;
    };

    let before_sel = app.cursors.primary();
    let cursor_char = before_sel.head.char;
    let (edits, after_sel) = session.handle_text_input(&mut app.buffer, cursor_char, text);

    let transaction = Transaction::new(edits, vec![before_sel], vec![after_sel]);
    app.history.record(transaction);
    app.cursors = MultiCursor::new(after_sel);
    app.reset_goal_columns();
    app.sync_syntax_on_edit();

    true
}

/// Handles Backspace inside an active snippet session.
pub(crate) fn handle_snippet_backspace(app: &mut App) -> bool {
    let Some(session) = app.snippet_session.as_mut() else {
        return false;
    };

    let before_sel = app.cursors.primary();
    let cursor_char = before_sel.head.char;
    let Some((edits, after_sel)) = session.handle_backspace(&mut app.buffer, cursor_char) else {
        return false;
    };

    let transaction = Transaction::new(edits, vec![before_sel], vec![after_sel]);
    app.history.record(transaction);
    app.cursors = MultiCursor::new(after_sel);
    app.reset_goal_columns();
    app.sync_syntax_on_edit();

    true
}

/// Opens the interactive snippet picker modal dialog.
pub(crate) fn open_snippet_picker(app: &mut App) {
    let scope = current_document_scope(app);
    let available: Vec<Snippet> = app
        .ui
        .snippets
        .registry
        .for_scope(scope)
        .into_iter()
        .cloned()
        .collect();
    app.ui.snippets.picker = Some(SnippetPickerState::new(available));
    app.scheduler.invalidate_all();
}
