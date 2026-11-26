//! Sublime Text's Transpose (`Ctrl+T`).
//!
//! - An empty caret swaps the grapheme before it with the one after it and
//!   moves past both. At the end of a line it swaps the two graphemes
//!   before it instead. It never reaches across a line break.
//! - Several selections, at least one non-empty: the selected texts rotate
//!   one place forward (the last one moves to the first selection). Empty
//!   carets in the mix stay put.
//! - One selection over two lines swaps the content of those lines.
//! - One selection inside a line: if it covers two or more words, the first
//!   and last swap; otherwise it trades places with the next word on the
//!   line, or the previous one when there is no next word. A selection that
//!   runs on through the line break (a whole line, as `Cmd+L` selects it)
//!   with fewer than two words is left alone.

use std::collections::HashMap;

use super::touched_lines;
use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::multicursor::MultiCursor;
use crate::position::Position;
use crate::selection::Selection;

/// Builds the per-selection operation that transposes `cursors`, for use
/// with `apply_to_each`. Every caret is handled in one transaction, so the
/// whole command is a single undo step.
pub fn transpose_op(
    buffer: &Buffer,
    cursors: &MultiCursor,
) -> impl FnMut(&mut Buffer, Selection) -> (Vec<Edit>, Selection) + use<> {
    let selected: Vec<Selection> = cursors
        .cursors()
        .iter()
        .copied()
        .filter(|s| !s.is_empty())
        .collect();
    // Keyed by start offset: `apply_to_each` hands each selection over
    // unchanged, and selections never overlap.
    let mut rotated: HashMap<usize, String> = HashMap::new();
    if selected.len() > 1 {
        let texts: Vec<String> = selected
            .iter()
            .map(|s| buffer.text_range(s.char_range()))
            .collect();
        for (index, selection) in selected.iter().enumerate() {
            let from = (index + texts.len() - 1) % texts.len();
            rotated.insert(selection.char_range().start, texts[from].clone());
        }
    }
    let any_selected = !selected.is_empty();
    move |buffer, selection| {
        if selection.is_empty() {
            return if any_selected {
                (Vec::new(), selection)
            } else {
                transpose(buffer, selection)
            };
        }
        if let Some(text) = rotated.get(&selection.char_range().start) {
            let length = text.chars().count();
            return replace(buffer, selection, selection.char_range(), text, 0, length);
        }
        transpose_selection(buffer, selection)
    }
}

/// Transposes around one empty caret (see the module docs). A no-op on a
/// non-empty selection, at the start of a line, or on a line with fewer
/// than two graphemes before an end-of-line caret.
pub fn transpose(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    if !selection.is_empty() {
        return (Vec::new(), selection);
    }
    // An earlier caret in the same command may already have rewritten the
    // text around this one, so its cached byte offset can sit inside a
    // multi-byte char. The char offset is still right; rebuild from it.
    let cursor = Position::from_char(buffer, selection.head.char);
    let (line_start, content_end) = line_bounds(buffer, cursor.line);
    if cursor.char <= line_start {
        return (Vec::new(), selection);
    }
    let (left, middle, right) = if cursor.char >= content_end {
        let middle = cursor.prev_grapheme(buffer);
        let left = middle.prev_grapheme(buffer);
        if left == middle || left.char < line_start {
            return (Vec::new(), selection);
        }
        (left.char, middle.char, cursor.char)
    } else {
        (
            cursor.prev_grapheme(buffer).char,
            cursor.char,
            cursor.next_grapheme(buffer).char,
        )
    };
    let swapped = format!(
        "{}{}",
        buffer.text_range(middle..right),
        buffer.text_range(left..middle)
    );
    let edits = splice(buffer, left..right, &swapped);
    let caret = Position::from_char(buffer, right);
    (edits, Selection::cursor(caret))
}

/// Transposes one non-empty selection on its own: two lines, or words.
fn transpose_selection(buffer: &mut Buffer, selection: Selection) -> (Vec<Edit>, Selection) {
    let lines = touched_lines(selection, buffer.rope());
    if lines.end() - lines.start() == 1 {
        return swap_lines(buffer, *lines.start());
    }
    if lines.end() != lines.start() {
        return (Vec::new(), selection);
    }
    let range = selection.char_range();
    let text = buffer.text_range(range.clone());
    let words = word_runs(&text);
    if words.len() >= 2 {
        let (first, last) = (words[0].clone(), words[words.len() - 1].clone());
        let chars: Vec<char> = text.chars().collect();
        let piece = |r: std::ops::Range<usize>| chars[r].iter().collect::<String>();
        let swapped = format!(
            "{}{}{}",
            piece(last.clone()),
            piece(first.end..last.start),
            piece(first.clone())
        );
        let whole = format!(
            "{}{}{}",
            piece(0..first.start),
            swapped,
            piece(last.end..chars.len())
        );
        return replace(buffer, selection, range.clone(), &whole, 0, range.len());
    }
    // The selection starts on its only touched line; it may run on through
    // that line's break (`touched_lines` drops a last line reached at
    // column 0), which leaves no neighbour word on the line to trade with.
    let (line_start, content_end) = line_bounds(buffer, *lines.start());
    if range.end > content_end {
        return (Vec::new(), selection);
    }
    let line: Vec<char> = buffer.text_range(line_start..content_end).chars().collect();
    let (start, end) = (range.start - line_start, range.end - line_start);
    let after = word_runs(&line[end..].iter().collect::<String>());
    if let Some(next) = after.first() {
        let (gap_end, word_end) = (end + next.start, end + next.end);
        let swapped: String = line[gap_end..word_end]
            .iter()
            .chain(&line[end..gap_end])
            .chain(&line[start..end])
            .collect();
        let span = range.start..line_start + word_end;
        return replace(
            buffer,
            selection,
            span,
            &swapped,
            word_end - end,
            range.len(),
        );
    }
    let before = word_runs(&line[..start].iter().collect::<String>());
    if let Some(prev) = before.last() {
        let swapped: String = line[start..end]
            .iter()
            .chain(&line[prev.end..start])
            .chain(&line[prev.start..prev.end])
            .collect();
        let span = line_start + prev.start..range.end;
        return replace(buffer, selection, span, &swapped, 0, range.len());
    }
    (Vec::new(), selection)
}

/// Swaps the content of `line` and the line after it, keeping the line
/// break between them, and selects both.
fn swap_lines(buffer: &mut Buffer, line: usize) -> (Vec<Edit>, Selection) {
    let (first_start, first_end) = line_bounds(buffer, line);
    let (second_start, second_end) = line_bounds(buffer, line + 1);
    let swapped = format!(
        "{}{}{}",
        buffer.text_range(second_start..second_end),
        buffer.text_range(first_end..second_start),
        buffer.text_range(first_start..first_end)
    );
    let edits = splice(buffer, first_start..second_end, &swapped);
    let selection = Selection::new(
        Position::from_char(buffer, first_start),
        Position::from_char(buffer, second_end),
    );
    (edits, selection)
}

/// Replaces `span` with `text` and selects `length` chars starting
/// `offset` chars into the new text, keeping the selection's direction.
fn replace(
    buffer: &mut Buffer,
    selection: Selection,
    span: std::ops::Range<usize>,
    text: &str,
    offset: usize,
    length: usize,
) -> (Vec<Edit>, Selection) {
    let start = span.start;
    let edits = splice(buffer, span, text);
    let from = Position::from_char(buffer, start + offset);
    let to = Position::from_char(buffer, start + offset + length);
    let reversed = selection.head.char < selection.anchor.char;
    let selection = if reversed {
        Selection::new(to, from)
    } else {
        Selection::new(from, to)
    };
    (edits, selection)
}

fn splice(buffer: &mut Buffer, span: std::ops::Range<usize>, text: &str) -> Vec<Edit> {
    let delete = Edit::delete(buffer, span.start, span.len());
    delete.apply(buffer);
    let insert = Edit::insert(span.start, text);
    insert.apply(buffer);
    vec![delete, insert]
}

/// Char offsets of the start of `line` and the end of its content (before
/// any line terminator).
fn line_bounds(buffer: &Buffer, line: usize) -> (usize, usize) {
    let start = buffer.rope().line_to_char(line);
    let content = buffer.line_content(line).unwrap_or_default();
    (start, start + content.chars().count())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Char ranges of the maximal runs of word characters in `text`.
fn word_runs(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut start = None;
    for (index, c) in text.chars().enumerate() {
        match (is_word_char(c), start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                runs.push(from..index);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        runs.push(from..text.chars().count());
    }
    runs
}

#[cfg(test)]
#[path = "transpose_tests.rs"]
mod tests;
