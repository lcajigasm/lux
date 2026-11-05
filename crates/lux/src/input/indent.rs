//! Indentation, tab stops, and newline auto-indent logic for Lux.

use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_core::{buffer::IndentStyle, edit_ops};

use crate::app::App;

/// A file with leading whitespace keeps its detected indent. A flat file
/// takes `editor.indentStyle` and `editor.tabWidth`. An explicit choice
/// (status bar, or a project `tab_size` on a flat buffer) wins over both.
pub(crate) fn sync_edit_indent(app: &mut App) {
    if app.buffer.indent_is_explicit() || app.buffer.indent_from_content() {
        return;
    }
    let Ok(config) = app.ui.settings.config.lock() else {
        return;
    };
    let resolved = config.resolved();
    let style = resolved
        .get("editor.indentStyle")
        .and_then(|value| value.as_str())
        .unwrap_or("space");
    let width = resolved
        .get("editor.tabWidth")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_f64().map(|number| number as u64))
        })
        .unwrap_or(4)
        .clamp(1, 16) as u8;
    let indent = if style.eq_ignore_ascii_case("tab") {
        lux_core::buffer::Indent {
            style: IndentStyle::Tabs,
            width: 1,
        }
    } else {
        lux_core::buffer::Indent {
            style: IndentStyle::Spaces,
            width,
        }
    };
    drop(config);
    app.buffer.set_fallback_indent(indent);
}

pub(crate) fn indent_selection(app: &mut App) {
    sync_edit_indent(app);
    app.apply_edit_operation(app.cursors.clone(), edit_ops::indent);
}

pub(crate) fn dedent_selection(app: &mut App) {
    sync_edit_indent(app);
    app.apply_edit_operation(app.cursors.clone(), edit_ops::dedent);
}

/// Inserts a tab or spaces according to the file indent and the settings.
pub(crate) fn insert_tab(app: &mut App) {
    if app.buffer.is_huge_file_mode() || app.editing_blocked() {
        return;
    }
    sync_edit_indent(app);
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
/// Languages with a checked body kind take the extra level from the parse
/// tree, so a brace inside a string does not indent and Python does.
pub(crate) fn insert_newline(app: &mut App) {
    if app.buffer.is_huge_file_mode() || app.editing_blocked() {
        return;
    }
    sync_edit_indent(app);
    if let Some(lang) = app.documents.language()
        && lux_syntax::indent_rules::has_body_kinds(lang)
    {
        insert_newline_from_tree(app, lang);
        return;
    }
    insert_newline_heuristic(app);
}

fn insert_newline_from_tree(app: &mut App, lang: lux_syntax::LanguageId) {
    let eol = app.buffer.eol().as_str().to_string();
    let unit = indent_unit(&app.buffer.indent());
    app.apply_edit_operation(app.cursors.clone(), move |buffer, selection| {
        let (start, end) = selection.ordered();
        let line = buffer.line_content(start.line).unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let col = start.column.min(chars.len());
        let prefix: String = chars[..col].iter().collect();
        let suffix: String = chars[col..].iter().collect();
        let between = (prefix.trim_end().ends_with('{') && suffix.trim_start().starts_with('}'))
            || (prefix.trim_end().ends_with('(') && suffix.trim_start().starts_with(')'))
            || (prefix.trim_end().ends_with('[') && suffix.trim_start().starts_with(']'));
        let leading: String = line
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .collect();
        // A column-0 token after `if True:` is not inside the suite. A
        // second probe, one indent wider, is. The wider level is used only
        // when the tree actually nests it. Neither probe is inserted.
        let suffix_blank = suffix.trim().is_empty();
        let (same_probe, deep_probe) = if between {
            (
                format!("{eol}{leading}x{eol}"),
                format!("{eol}{leading}{unit}x{eol}"),
            )
        } else if suffix_blank {
            (format!("{eol}{leading}x"), format!("{eol}{leading}{unit}x"))
        } else {
            (eol.clone(), eol.clone())
        };
        let same_text = splice_range(&buffer.text(), start.char, end.char, &same_probe);
        let deep_text = splice_range(&buffer.text(), start.char, end.char, &deep_probe);
        let open_level = tree_level(lang, &same_text, start.line);
        let new_level = tree_level(lang, &same_text, start.line + 1).max(tree_level(
            lang,
            &deep_text,
            start.line + 1,
        ));
        let mid = shift_indent(&leading, new_level - open_level, &unit);
        let inserted = if between {
            let closer_level = tree_level(lang, &same_text, start.line + 2);
            let closer = shift_indent(&leading, closer_level - open_level, &unit);
            format!("{eol}{mid}{eol}{closer}")
        } else {
            format!("{eol}{mid}")
        };
        let cursor_chars = if between {
            eol.chars().count() + mid.chars().count()
        } else {
            inserted.chars().count()
        };
        let (edits, _) = edit_ops::insert_text(buffer, selection, &inserted);
        let new_pos = Position::from_char(buffer, start.char + cursor_chars);
        (edits, Selection::cursor(new_pos))
    });
}

fn tree_level(lang: lux_syntax::LanguageId, source: &str, row: usize) -> i32 {
    let mut doc = lux_syntax::Document::new(lang);
    doc.set_text(source);
    lux_syntax::indent_rules::indent_level(&doc, source, row) as i32
}

fn indent_unit(indent: &lux_core::buffer::Indent) -> String {
    match indent.style {
        IndentStyle::Tabs => "\t".to_string(),
        IndentStyle::Spaces => " ".repeat((indent.width as usize).max(1)),
    }
}

fn shift_indent(leading: &str, delta: i32, unit: &str) -> String {
    if delta >= 0 {
        let mut out = leading.to_string();
        for _ in 0..delta {
            out.push_str(unit);
        }
        return out;
    }
    let mut out = leading.to_string();
    for _ in 0..(-delta) {
        if let Some(stripped) = out.strip_suffix(unit) {
            out = stripped.to_string();
        } else {
            break;
        }
    }
    out
}

fn splice_range(text: &str, start: usize, end: usize, insert: &str) -> String {
    let mut out = String::with_capacity(text.len() + insert.len());
    let mut chars = text.chars();
    for _ in 0..start {
        if let Some(ch) = chars.next() {
            out.push(ch);
        }
    }
    out.push_str(insert);
    for _ in start..end {
        chars.next();
    }
    out.extend(chars);
    out
}

fn insert_newline_heuristic(app: &mut App) {
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
