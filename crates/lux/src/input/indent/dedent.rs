//! Dedent on typing: a closer typed as the first non-blank char of a line
//! moves to its opener's indentation, and a Python `else:`, `elif ...:`,
//! `except ...:` or `finally:` moves to the header it continues. The dedent
//! and the typed char are one edit, so one undo removes both.

use lux_core::buffer::Buffer;
use lux_core::edit::Edit;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_syntax::LanguageId;

use crate::app::App;

/// How far back a closer looks for its opener. Past this the line keeps
/// the indentation it has.
const MAX_SCAN_CHARS: usize = 256 * 1024;
/// How many lines back a Python clause looks for its header.
const MAX_SCAN_LINES: usize = 2_000;

/// Types `ch` with a dedent where one applies. Returns false, having done
/// nothing, when no cursor's line dedents, so the caller types `ch` as usual.
pub(crate) fn handle_char_typed(app: &mut App, ch: char) -> bool {
    if app.editing_blocked() || app.buffer.is_huge_file_mode() || app.snippet_session.is_some() {
        return false;
    }
    let python = app.documents.language() == Some(LanguageId::Python);
    let rule = match ch {
        '}' | ']' | ')' => Rule::Closer(ch),
        ':' if python => Rule::PythonClause,
        _ => return false,
    };
    if app.cursors.cursors().iter().any(|s| !s.is_empty()) {
        return false;
    }
    super::sync_edit_indent(app);
    let tab = tab_width(&app.buffer);
    let any = app
        .cursors
        .cursors()
        .iter()
        .any(|s| target(&app.buffer, s.head, rule, tab).is_some());
    if !any {
        return false;
    }
    let typed = ch.to_string();
    crate::macros::dispatch::record_text_input(app, &typed);
    app.apply_edit_operation(app.cursors.clone(), move |buffer, selection| {
        let head = selection.head;
        let mut edits = Vec::new();
        let mut at = head.char;
        if let Some(indent) = target(buffer, head, rule, tab) {
            let line_start = buffer.rope().line_to_char(head.line);
            let line = buffer.line_content(head.line).unwrap_or_default();
            let old_len = leading(&line).chars().count().min(head.column);
            let old = Edit::delete(buffer, line_start, old_len);
            old.apply(buffer);
            edits.push(old);
            if !indent.is_empty() {
                let new = Edit::insert(line_start, indent.as_str());
                new.apply(buffer);
                edits.push(new);
            }
            at = head.char - old_len + indent.chars().count();
        }
        let insert = Edit::insert(at, typed.as_str());
        insert.apply(buffer);
        edits.push(insert);
        (
            edits,
            Selection::cursor(Position::from_char(buffer, at + 1)),
        )
    });
    true
}

#[derive(Clone, Copy)]
enum Rule {
    Closer(char),
    PythonClause,
}

/// The indentation `head`'s line takes when `rule`'s char is typed there,
/// or `None` when the line keeps its own. Only ever narrows.
fn target(buffer: &Buffer, head: Position, rule: Rule, tab: usize) -> Option<String> {
    let line = buffer.line_content(head.line)?;
    let prefix: String = line.chars().take(head.column).collect();
    let indent = match rule {
        Rule::Closer(close) => {
            if !prefix.chars().all(|c| c == ' ' || c == '\t') {
                return None;
            }
            opener_indent(buffer, head.char, close)?
        }
        Rule::PythonClause => {
            let suffix: String = line.chars().skip(head.column).collect();
            if !suffix.trim().is_empty() {
                return None;
            }
            let clause = clause_keyword(prefix.trim())?;
            header_indent(buffer, head.line, &leading(&prefix), clause, tab)?
        }
    };
    (width(&indent, tab) < width(&leading(&prefix), tab)).then_some(indent)
}

/// The leading whitespace of the line holding the unmatched opener of
/// `close` before `before`.
fn opener_indent(buffer: &Buffer, before: usize, close: char) -> Option<String> {
    let open = match close {
        '}' => '{',
        ']' => '[',
        _ => '(',
    };
    let rope = buffer.rope();
    let mut depth = 0usize;
    let mut at = before;
    for c in rope.chars_at(before).reversed().take(MAX_SCAN_CHARS) {
        at -= 1;
        if c == close {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                let line = rope.char_to_line(at);
                return Some(leading(&buffer.line_content(line)?));
            }
            depth -= 1;
        }
    }
    None
}

/// The clause keyword `text` starts, if `text` plus a typed `:` would
/// complete a Python `else:`, `elif ...:`, `except ...:` or `finally:`.
fn clause_keyword(text: &str) -> Option<&'static str> {
    let word: String = text
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let rest = &text[word.len()..];
    match word.as_str() {
        "else" if rest.trim().is_empty() => Some("else"),
        "finally" if rest.trim().is_empty() => Some("finally"),
        "elif" if !rest.trim().is_empty() => Some("elif"),
        "except" => Some("except"),
        _ => None,
    }
}

/// The indentation of the nearest header above `line` that `clause`
/// continues: a line no deeper than `line`, shallower than every line
/// between them, that opens with a compatible keyword.
fn header_indent(
    buffer: &Buffer,
    line: usize,
    current: &str,
    clause: &str,
    tab: usize,
) -> Option<String> {
    let headers: &[&str] = match clause {
        "elif" => &["if", "elif"],
        "except" => &["try", "except"],
        "finally" => &["try", "except", "else"],
        _ => &["if", "elif", "for", "while", "try", "except"],
    };
    let limit = width(current, tab);
    let mut shallowest = usize::MAX;
    for row in (line.saturating_sub(MAX_SCAN_LINES)..line).rev() {
        let text = buffer.line_content(row)?;
        let body = text.trim_start();
        if body.is_empty() || body.starts_with('#') {
            continue;
        }
        let indent = leading(&text);
        let depth = width(&indent, tab);
        if depth < shallowest && depth <= limit && opens_with(body, headers) {
            return Some(indent);
        }
        shallowest = shallowest.min(depth);
        if shallowest == 0 {
            return None;
        }
    }
    None
}

fn opens_with(body: &str, keywords: &[&str]) -> bool {
    let body = body.strip_prefix("async ").unwrap_or(body);
    keywords.iter().any(|k| {
        body.strip_prefix(k)
            .is_some_and(|rest| rest.starts_with([' ', ':', '(', '\t']))
    })
}

fn leading(line: &str) -> String {
    line.chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .collect()
}

fn width(indent: &str, tab: usize) -> usize {
    indent
        .chars()
        .fold(0, |w, c| if c == '\t' { w - w % tab + tab } else { w + 1 })
}

fn tab_width(buffer: &Buffer) -> usize {
    (buffer.indent().width as usize).max(1)
}
