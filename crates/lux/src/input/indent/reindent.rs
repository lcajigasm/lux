//! Reindent: sets each selected line's leading whitespace to the level the
//! syntax tree gives it, in the buffer's indent unit. Blank lines are left
//! as they are.

use std::collections::HashMap;

use lux_core::buffer::Buffer;
use lux_core::edit::Edit;
use lux_core::position::Position;
use lux_core::selection::Selection;

use crate::app::App;

impl App {
    /// The `reindent` command.
    pub(crate) fn reindent(&mut self) {
        reindent_selection(self);
    }
}

/// Reindents the lines every selection touches (the caret's line for an
/// empty one) as one undo step. Languages without tree indent rules are
/// left unchanged with a status message saying so.
fn reindent_selection(app: &mut App) {
    if app.buffer.is_huge_file_mode() || app.editing_blocked() {
        return;
    }
    let Some(lang) = app
        .documents
        .language()
        .filter(|&lang| lux_syntax::indent_rules::has_body_kinds(lang))
    else {
        app.documents.message = Some("Reindent: no indentation rules for this syntax".into());
        return;
    };
    super::sync_edit_indent(app);
    let unit = super::indent_unit(&app.buffer.indent());
    let levels = tree_levels(&app.buffer, &app.cursors, lang);
    app.apply_edit_operation(app.cursors.clone(), move |buffer, selection| {
        reindent_lines(buffer, selection, &levels, &unit)
    });
}

/// The lines `selection` reindents: every line it touches, except the
/// line a multi-line selection ends at column 0 of.
fn selected_lines(selection: Selection) -> std::ops::RangeInclusive<usize> {
    let (start, end) = selection.ordered();
    let last = if end.line > start.line && end.column == 0 {
        end.line - 1
    } else {
        end.line
    };
    start.line..=last
}

/// The tree's indent level for every line any selection reindents.
/// Reindent exists for misindented code, so no column-0 line can be
/// trusted to start a top-level item: the levels come from one parse of
/// the whole rope, streamed in chunks rather than copied, shared by every
/// selection. This is an explicit command, not a keystroke. Reindenting
/// only rewrites leading whitespace, so line numbers stay valid while the
/// edits apply.
fn tree_levels(
    buffer: &Buffer,
    cursors: &lux_core::multicursor::MultiCursor,
    lang: lux_syntax::LanguageId,
) -> HashMap<usize, usize> {
    let mut doc = lux_syntax::Document::new(lang);
    doc.set_rope(buffer.rope());
    let mut levels = HashMap::new();
    for &selection in cursors.cursors() {
        for line in selected_lines(selection) {
            levels.entry(line).or_insert_with(|| {
                let content = buffer.line_content(line).unwrap_or_default();
                let column = content.len() - content.trim_start().len();
                lux_syntax::indent_rules::indent_level_at(&doc, line, column)
            });
        }
    }
    levels
}

fn reindent_lines(
    buffer: &mut Buffer,
    selection: Selection,
    levels: &HashMap<usize, usize>,
    unit: &str,
) -> (Vec<Edit>, Selection) {
    let mut edits = Vec::new();
    let mut shifts = Vec::new();
    for line in selected_lines(selection).rev() {
        let level = levels.get(&line).copied().unwrap_or_default();
        let content = buffer.line_content(line).unwrap_or_default();
        let old: usize = content
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .count();
        if content.chars().count() == old {
            shifts.push((line, old, old));
            continue;
        }
        let indent = unit.repeat(level);
        let new = indent.chars().count();
        shifts.push((line, old, new));
        if content.chars().take(old).eq(indent.chars()) {
            continue;
        }
        let line_start = buffer.rope().line_to_char(line);
        let delete = Edit::delete(buffer, line_start, old);
        delete.apply(buffer);
        edits.push(delete);
        if !indent.is_empty() {
            let insert = Edit::insert(line_start, indent);
            insert.apply(buffer);
            edits.push(insert);
        }
    }
    let moved = |pos: Position| {
        let column = match shifts.iter().find(|(line, _, _)| *line == pos.line) {
            Some(&(_, old, new)) if pos.column <= old => new,
            Some(&(_, old, new)) => pos.column - old + new,
            None => pos.column,
        };
        lux_core::selection_ops::position_at_column(buffer, pos.line, column)
    };
    let after = Selection::new(moved(selection.anchor), moved(selection.head));
    (edits, after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust(text: &str, first: usize, last: usize) -> String {
        let mut buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let from = Position::from_char(&buffer, buffer.rope().line_to_char(first));
        let to = Position::from_char(&buffer, buffer.rope().line_to_char(last));
        let selection = Selection::new(from, to);
        let cursors = lux_core::multicursor::MultiCursor::new(selection);
        let levels = tree_levels(&buffer, &cursors, lux_syntax::LanguageId::Rust);
        reindent_lines(&mut buffer, selection, &levels, "    ");
        buffer.text()
    }

    #[test]
    fn reindent_fixes_every_selected_line_from_the_tree() {
        let text = "fn f() {\nif x {\n        y();\n  }\n}\n";
        assert_eq!(
            rust(text, 0, 4),
            "fn f() {\n    if x {\n        y();\n    }\n}\n"
        );
    }

    #[test]
    fn reindent_leaves_lines_outside_the_selection() {
        let text = "fn f() {\nif x {\n  y();\n  }\n}\n";
        assert_eq!(rust(text, 2, 3), "fn f() {\nif x {\n        y();\n  }\n}\n");
    }
}
