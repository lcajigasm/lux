//! Word movement with subword rules: stops at `camelCase`/`PascalCase`
//! transitions and `_`/`-` separators, not just whitespace/punctuation —
//! `fooBarBaz` is three stops, not one, and `foo_bar` stops at the
//! underscore too. Deliberately a fresh classifier rather than reusing
//! `edit_ops::word`'s (coarser, and tuned for "delete the previous word",
//! which skips trailing whitespace first — a different operation).

use crate::buffer::Buffer;
use crate::position::Position;

/// How far a single word-movement scan looks, in chars. Bounds the cost of
/// one movement to "a very long identifier" rather than "the rest of the
/// document" if no boundary is ever found.
const SCAN_CAP: usize = 256;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Class {
    Upper,
    Lower,
    Digit,
    Separator,
    Space,
    Other,
}

fn classify(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c == '_' || c == '-' {
        Class::Separator
    } else if c.is_uppercase() {
        Class::Upper
    } else if c.is_lowercase() {
        Class::Lower
    } else if c.is_ascii_digit() {
        Class::Digit
    } else {
        Class::Other
    }
}

fn is_word_content(class: Class) -> bool {
    matches!(class, Class::Upper | Class::Lower | Class::Digit)
}

/// True when there's a subword boundary right between `prev` and `cur`.
/// `next` (the char after `cur`, if any) is only needed to resolve
/// `"HTTPRequest"`-style runs of capitals: the boundary sits right before
/// the *last* capital of the run, found by checking whether the one after
/// it drops to lowercase.
fn is_subword_boundary(prev: char, cur: char, next: Option<char>) -> bool {
    let (pc, cc) = (classify(prev), classify(cur));
    if is_word_content(pc) != is_word_content(cc) {
        return true;
    }
    if !is_word_content(pc) {
        return false;
    }
    match (pc, cc) {
        (Class::Lower, Class::Upper) | (Class::Digit, Class::Upper) => true,
        (Class::Upper, Class::Upper) => matches!(next.map(classify), Some(Class::Lower)),
        _ => false,
    }
}

fn window_after(buffer: &Buffer, start: usize) -> Vec<char> {
    let rope = buffer.rope();
    let end = (start + SCAN_CAP).min(rope.len_chars());
    rope.slice(start..end).chars().collect()
}

fn window_before(buffer: &Buffer, start: usize) -> (usize, Vec<char>) {
    let rope = buffer.rope();
    let begin = start.saturating_sub(SCAN_CAP);
    (begin, rope.slice(begin..start).chars().collect())
}

fn scan_forward(buffer: &Buffer, char_idx: usize) -> usize {
    let start = char_idx.min(buffer.rope().len_chars());
    let window = window_after(buffer, start);
    if window.is_empty() {
        return start;
    }

    let mut i = 0;
    while i < window.len() && classify(window[i]) == Class::Space {
        i += 1;
    }
    if i >= window.len() {
        return start + i;
    }
    i += 1;
    while i < window.len() {
        let next = window.get(i + 1).copied();
        if is_subword_boundary(window[i - 1], window[i], next) {
            break;
        }
        i += 1;
    }
    start + i
}

fn scan_backward(buffer: &Buffer, char_idx: usize) -> usize {
    let start = char_idx.min(buffer.rope().len_chars());
    let (begin, window) = window_before(buffer, start);
    if window.is_empty() {
        return begin;
    }

    let mut i = window.len();
    while i > 0 && classify(window[i - 1]) == Class::Space {
        i -= 1;
    }
    if i == 0 {
        return begin;
    }
    i -= 1;
    while i > 0 {
        let next = window.get(i + 1).copied();
        if is_subword_boundary(window[i - 1], window[i], next) {
            break;
        }
        i -= 1;
    }
    begin + i
}

/// Moves right to the next subword boundary: skips any whitespace right
/// after `pos` first, then advances through one run of content, stopping
/// at a subword transition, a separator, or the next run of whitespace.
pub fn move_word_right(buffer: &Buffer, pos: Position) -> Position {
    Position::from_char(buffer, scan_forward(buffer, pos.char))
}

/// The mirror of `move_word_right`, scanning backward.
pub fn move_word_left(buffer: &Buffer, pos: Position) -> Position {
    Position::from_char(buffer, scan_backward(buffer, pos.char))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    #[test]
    fn word_right_stops_on_camel_case_transition() {
        let buffer = Buffer::from_bytes(b"fooBar baz").unwrap();
        let next = move_word_right(&buffer, pos_at(&buffer, 0));
        assert_eq!(next.char, 3); // right before "Bar"
    }

    #[test]
    fn word_right_treats_a_run_of_capitals_specially() {
        let buffer = Buffer::from_bytes(b"HTTPRequest").unwrap();
        let next = move_word_right(&buffer, pos_at(&buffer, 0));
        assert_eq!(next.char, 4); // "HTTP" | "Request"
    }

    #[test]
    fn word_right_stops_at_underscore_separators() {
        let buffer = Buffer::from_bytes(b"foo_bar").unwrap();
        let first = move_word_right(&buffer, pos_at(&buffer, 0));
        assert_eq!(first.char, 3); // right at "_"
        let second = move_word_right(&buffer, first);
        assert_eq!(second.char, 4); // right after "_", at "bar"
    }

    #[test]
    fn word_right_skips_whitespace_then_stops_at_the_next_boundary() {
        let buffer = Buffer::from_bytes(b"foo   Bar").unwrap();
        let next = move_word_right(&buffer, pos_at(&buffer, 3)); // right after "foo"
        assert_eq!(next.char, 9); // "Bar" is itself a single subword run
    }

    #[test]
    fn word_right_at_document_end_stays_put() {
        let buffer = Buffer::from_bytes(b"foo").unwrap();
        let next = move_word_right(&buffer, pos_at(&buffer, 3));
        assert_eq!(next.char, 3);
    }

    #[test]
    fn word_left_is_the_inverse_of_word_right() {
        let buffer = Buffer::from_bytes(b"fooBar baz").unwrap();
        let forward = move_word_right(&buffer, pos_at(&buffer, 0));
        let back = move_word_left(&buffer, forward);
        assert_eq!(back.char, 0);
    }

    #[test]
    fn word_left_from_the_middle_of_a_run_goes_to_its_start() {
        let buffer = Buffer::from_bytes(b"fooBar baz").unwrap();
        let back = move_word_left(&buffer, pos_at(&buffer, 9)); // inside "baz"
        assert_eq!(back.char, 7); // start of "baz"
    }

    #[test]
    fn word_left_at_document_start_stays_put() {
        let buffer = Buffer::from_bytes(b"foo").unwrap();
        let back = move_word_left(&buffer, pos_at(&buffer, 0));
        assert_eq!(back.char, 0);
    }
}
