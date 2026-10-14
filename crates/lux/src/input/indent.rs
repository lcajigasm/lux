//! Indentation, tab stops, and newline auto-indent logic for Lux.

use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_core::{buffer::IndentStyle, edit_ops};

use crate::app::App;

/// Inserts a tab stop or soft spaces according to document indentation settings.
pub(crate) fn insert_tab(app: &mut App) {
    if app.buffer.is_huge_file_mode() || app.editing_blocked() {
        return;
    }
    let indent = app.buffer.indent();
    if indent.style == IndentStyle::Tabs {
        app.apply_edit_operation(app.cursors.clone(), |buffer, selection| {
            edit_ops::insert_text(buffer, selection, "\t")
        });
    } else {
        let width = (indent.width as usize).max(1);
        app.apply_edit_operation(app.cursors.clone(), move |buffer, selection| {
            let col = selection.head.column;
            let spaces = width - (col % width);
            let s = " ".repeat(spaces);
            edit_ops::insert_text(buffer, selection, &s)
        });
    }
}

/// Inserts a newline with smart indentation preserving leading whitespace and
/// incrementing indentation if a block opening delimiter was typed.
pub(crate) fn insert_newline(app: &mut App) {
    if app.buffer.is_huge_file_mode() || app.editing_blocked() {
        return;
    }
    let eol = app.buffer.eol().as_str().to_string();
    let indent = app.buffer.indent();
    let indent_unit = match indent.style {
        IndentStyle::Tabs => "\t".to_string(),
        IndentStyle::Spaces => " ".repeat((indent.width as usize).max(1)),
    };

    app.apply_edit_operation(app.cursors.clone(), move |buffer, selection| {
        let pos = selection.head;
        let line_content = buffer.line_content(pos.line).unwrap_or_default();
        let leading_ws: String = line_content
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .collect();

        let chars: Vec<char> = line_content.chars().collect();
        let col = pos.column.min(chars.len());
        let prefix: String = chars[..col].iter().collect();
        let suffix: String = chars[col..].iter().collect();

        let trimmed_prefix = prefix.trim_end();
        let trimmed_suffix = suffix.trim_start();

        let is_between_braces = (trimmed_prefix.ends_with('{') && trimmed_suffix.starts_with('}'))
            || (trimmed_prefix.ends_with('(') && trimmed_suffix.starts_with(')'))
            || (trimmed_prefix.ends_with('[') && trimmed_suffix.starts_with(']'));

        let is_only_ws = chars.iter().all(|&c| c == ' ' || c == '\t');
        let leading_ws_count = leading_ws.chars().count();

        if is_between_braces {
            let text = format!("{eol}{leading_ws}{indent_unit}{eol}{leading_ws}");
            let (edits, _) = edit_ops::insert_text(buffer, selection, &text);
            let middle_col = leading_ws.chars().count() + indent_unit.chars().count();
            let new_pos = Position::from_char(
                buffer,
                selection.ordered().0.char + eol.chars().count() + middle_col,
            );
            (edits, Selection::cursor(new_pos))
        } else if pos.column <= leading_ws_count && !is_only_ws {
            edit_ops::insert_text(buffer, selection, &eol)
        } else if is_only_ws {
            if pos.column == 0 {
                edit_ops::insert_text(buffer, selection, &eol)
            } else {
                let text = format!("{eol}{leading_ws}");
                edit_ops::insert_text(buffer, selection, &text)
            }
        } else {
            let opens_block = trimmed_prefix.ends_with('{')
                || trimmed_prefix.ends_with('(')
                || trimmed_prefix.ends_with('[')
                || trimmed_prefix.ends_with(':');

            let indent_to_add = if opens_block {
                format!("{leading_ws}{indent_unit}")
            } else {
                leading_ws
            };
            let text = format!("{eol}{indent_to_add}");
            edit_ops::insert_text(buffer, selection, &text)
        }
    });
}
