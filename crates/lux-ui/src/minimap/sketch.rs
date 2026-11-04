//! Column sketch of one source line for the minimap.
//!
//! A minimap that shaped every glyph would walk the whole file through the
//! text shaper on each frame. This sketch walks the bytes once and keeps
//! runs: a comment, a string, a keyword from a fixed word list, other
//! words, and punctuation. The colors come from the syntax theme at draw
//! time. It is not the tree-sitter highlighter.

use super::{MinimapInk, MinimapRow, MinimapRun};

const SKETCH_COLUMNS: usize = 160;
const MAX_RUNS: usize = 48;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Code,
    String(char),
    Block,
}

struct OpenRun {
    ink: MinimapInk,
    column: usize,
    width: usize,
    start_byte: usize,
    end_byte: usize,
}

pub(super) struct LineSketch {
    pub indent: usize,
    pub content: usize,
    pub runs: Vec<MinimapRun>,
    pub in_block: bool,
}

pub(super) fn sketch_line(line: &str, in_block: bool) -> LineSketch {
    let mut indent = 0usize;
    let mut total = 0usize;
    let mut past_indent = false;
    let mut column = 0usize;
    let mut mode = if in_block { Mode::Block } else { Mode::Code };
    let mut escape = false;
    let mut runs = Vec::new();
    let mut open: Option<OpenRun> = None;
    let bytes = line.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        let start = i;
        let ch = line[i..].chars().next().unwrap();
        i += ch.len_utf8();
        if ch == '\n' || ch == '\r' {
            continue;
        }
        total += 1;
        if !past_indent && (ch == ' ' || ch == '\t') {
            indent += 1;
            column += 1;
            continue;
        }
        past_indent = true;
        if column >= SKETCH_COLUMNS {
            column += 1;
            continue;
        }

        match mode {
            Mode::Block => {
                push(
                    &mut open,
                    &mut runs,
                    line,
                    MinimapInk::Comment,
                    column,
                    start,
                    i,
                );
                if ch == '*' && bytes.get(i) == Some(&b'/') {
                    let slash = i;
                    i += 1;
                    total += 1;
                    column += 1;
                    if column < SKETCH_COLUMNS {
                        push(
                            &mut open,
                            &mut runs,
                            line,
                            MinimapInk::Comment,
                            column,
                            slash,
                            i,
                        );
                    }
                    close_run(line, &mut runs, &mut open);
                    mode = Mode::Code;
                }
            }
            Mode::String(quote) => {
                push(
                    &mut open,
                    &mut runs,
                    line,
                    MinimapInk::String,
                    column,
                    start,
                    i,
                );
                if escape {
                    escape = false;
                } else if ch == '\\' {
                    escape = true;
                } else if ch == quote {
                    close_run(line, &mut runs, &mut open);
                    mode = Mode::Code;
                }
            }
            Mode::Code => {
                if ch == '/' && bytes.get(i) == Some(&b'/') {
                    close_run(line, &mut runs, &mut open);
                    // A `//` comment ends with the line. It does not set
                    // `mode`, so it cannot leak into the next line.
                    push(
                        &mut open,
                        &mut runs,
                        line,
                        MinimapInk::Comment,
                        column,
                        start,
                        i,
                    );
                    let slash = i;
                    i += 1;
                    total += 1;
                    column += 1;
                    if column < SKETCH_COLUMNS {
                        push(
                            &mut open,
                            &mut runs,
                            line,
                            MinimapInk::Comment,
                            column,
                            slash,
                            i,
                        );
                    }
                    while i < bytes.len() {
                        let rest = i;
                        let next = line[i..].chars().next().unwrap();
                        i += next.len_utf8();
                        if next == '\n' || next == '\r' {
                            break;
                        }
                        total += 1;
                        column += 1;
                        if column < SKETCH_COLUMNS {
                            push(
                                &mut open,
                                &mut runs,
                                line,
                                MinimapInk::Comment,
                                column,
                                rest,
                                i,
                            );
                        }
                    }
                    close_run(line, &mut runs, &mut open);
                    break;
                } else if ch == '/' && bytes.get(i) == Some(&b'*') {
                    close_run(line, &mut runs, &mut open);
                    mode = Mode::Block;
                    push(
                        &mut open,
                        &mut runs,
                        line,
                        MinimapInk::Comment,
                        column,
                        start,
                        i,
                    );
                    let star = i;
                    i += 1;
                    total += 1;
                    column += 1;
                    if column < SKETCH_COLUMNS {
                        push(
                            &mut open,
                            &mut runs,
                            line,
                            MinimapInk::Comment,
                            column,
                            star,
                            i,
                        );
                    }
                } else if ch == '"' || ch == '\'' {
                    close_run(line, &mut runs, &mut open);
                    mode = Mode::String(ch);
                    escape = false;
                    push(
                        &mut open,
                        &mut runs,
                        line,
                        MinimapInk::String,
                        column,
                        start,
                        i,
                    );
                } else if ch == ' ' || ch == '\t' {
                    close_run(line, &mut runs, &mut open);
                } else if ch.is_alphanumeric() || ch == '_' {
                    push(
                        &mut open,
                        &mut runs,
                        line,
                        MinimapInk::Text,
                        column,
                        start,
                        i,
                    );
                } else {
                    push(
                        &mut open,
                        &mut runs,
                        line,
                        MinimapInk::Punct,
                        column,
                        start,
                        i,
                    );
                }
            }
        }
        column += 1;
    }
    close_run(line, &mut runs, &mut open);
    LineSketch {
        indent,
        content: total - indent,
        runs,
        in_block: mode == Mode::Block,
    }
}

fn push(
    open: &mut Option<OpenRun>,
    runs: &mut Vec<MinimapRun>,
    line: &str,
    ink: MinimapInk,
    column: usize,
    start_byte: usize,
    end_byte: usize,
) {
    if let Some(run) = open.as_mut()
        && run.ink == ink
        && run.column + run.width == column
    {
        run.width += 1;
        run.end_byte = end_byte;
        return;
    }
    close_run(line, runs, open);
    *open = Some(OpenRun {
        ink,
        column,
        width: 1,
        start_byte,
        end_byte,
    });
}

fn close_run(line: &str, runs: &mut Vec<MinimapRun>, open: &mut Option<OpenRun>) {
    let Some(run) = open.take() else {
        return;
    };
    if run.width == 0 {
        return;
    }
    let mut ink = run.ink;
    if ink == MinimapInk::Text && is_keyword(&line[run.start_byte..run.end_byte]) {
        ink = MinimapInk::Keyword;
    }
    let at_cap = runs.len() >= MAX_RUNS;
    if let Some(last) = runs.last_mut() {
        let adjacent = last.ink == ink && last.column as usize + last.width as usize == run.column;
        if at_cap || adjacent {
            let end = run.column + run.width;
            if end > last.column as usize {
                last.width = (end - last.column as usize).min(u16::MAX as usize) as u16;
            }
            return;
        }
    }
    if at_cap {
        return;
    }
    runs.push(MinimapRun {
        column: run.column.min(u16::MAX as usize) as u16,
        width: run.width.min(u16::MAX as usize) as u16,
        ink,
    });
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "fn" | "let"
            | "mut"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "return"
            | "pub"
            | "struct"
            | "enum"
            | "impl"
            | "use"
            | "mod"
            | "match"
            | "const"
            | "static"
            | "type"
            | "where"
            | "async"
            | "await"
            | "move"
            | "ref"
            | "in"
            | "as"
            | "true"
            | "false"
            | "self"
            | "super"
            | "crate"
            | "trait"
            | "dyn"
            | "unsafe"
    )
}

pub(super) fn build_rows(
    index: &crate::editor_view::line_index::LineIndex,
    text: &str,
    total_visual: usize,
    total_lines: usize,
    available_rows: usize,
    folds: Option<&lux_core::folding::FoldState>,
) -> Vec<MinimapRow> {
    let per_row = super::lines_per_row(total_visual, available_rows);
    let mut rows = Vec::with_capacity(super::row_count(total_visual, available_rows).max(1));
    let mut in_block = false;
    let mut best: Option<MinimapRow> = None;
    let mut filled = 0usize;
    for visual in 0..total_visual {
        let line = folds.map_or(visual, |f| f.visual_to_buffer_line(visual, total_lines));
        let line_text = &text[index.line_byte_range(text, line)];
        let sketch = sketch_line(line_text, in_block);
        in_block = sketch.in_block;
        let candidate = MinimapRow {
            line,
            indent_columns: sketch.indent,
            content_columns: sketch.content,
            runs: sketch.runs,
        };
        best = Some(match best {
            Some(prev) if prev.content_columns >= candidate.content_columns => prev,
            _ => candidate,
        });
        filled += 1;
        if filled == per_row {
            rows.push(best.take().unwrap());
            filled = 0;
        }
    }
    if let Some(last) = best.take() {
        rows.push(last);
    }
    if rows.is_empty() {
        rows.push(MinimapRow {
            line: 0,
            indent_columns: 0,
            content_columns: 0,
            runs: Vec::new(),
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_comment_is_one_comment_run() {
        let sketch = sketch_line("// note", false);
        assert_eq!(sketch.runs.len(), 1);
        assert_eq!(sketch.runs[0].ink, MinimapInk::Comment);
        assert!(!sketch.in_block);
    }

    #[test]
    fn a_string_and_a_keyword_are_separate_runs() {
        let sketch = sketch_line("let s = \"hi\";", false);
        assert!(sketch.runs.iter().any(|run| run.ink == MinimapInk::Keyword));
        assert!(sketch.runs.iter().any(|run| run.ink == MinimapInk::String));
        let string = sketch
            .runs
            .iter()
            .find(|run| run.ink == MinimapInk::String)
            .unwrap();
        assert!(string.width >= 4);
    }

    #[test]
    fn indentation_shifts_the_first_run() {
        let sketch = sketch_line("    code", false);
        assert_eq!(sketch.indent, 4);
        assert_eq!(sketch.runs[0].column, 4);
        assert_eq!(sketch.runs[0].ink, MinimapInk::Text);
    }

    #[test]
    fn a_block_comment_continues_onto_the_next_line() {
        let open = sketch_line("/* hide", false);
        assert!(open.in_block);
        let next = sketch_line("still", true);
        assert!(next.in_block);
        assert_eq!(next.runs[0].ink, MinimapInk::Comment);
        let closed = sketch_line("done */ let x", true);
        assert!(!closed.in_block);
        assert!(closed.runs.iter().any(|run| run.ink == MinimapInk::Keyword));
    }

    #[test]
    fn a_blank_line_has_no_runs() {
        let sketch = sketch_line("   ", false);
        assert!(sketch.runs.is_empty());
        assert_eq!(sketch.content, 0);
    }
}
