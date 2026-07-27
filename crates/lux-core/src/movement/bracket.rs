//! Bracket matching and jump-to-match. Syntax-unaware — plain nesting-depth
//! counting over `()`/`[]`/`{}`, with no idea that a bracket inside a
//! string literal or comment isn't a "real" one. `lux-core` has no
//! tokenizer yet (tree-sitter arrives in a later phase); this is the
//! honest, correctly-scoped version until then, not a stand-in pretending
//! to be smarter.

use crate::buffer::Buffer;
use crate::position::Position;

const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

/// The position of the bracket matching whichever bracket touches `pos` —
/// checking the char right at `pos` first (cursor sitting just before a
/// bracket), then the char right before `pos` (cursor sitting just after
/// one). `None` if neither side is a recognized bracket, or the bracket is
/// unbalanced.
pub fn matching_bracket(buffer: &Buffer, pos: Position) -> Option<Position> {
    let rope = buffer.rope();
    let len = rope.len_chars();

    if pos.char < len
        && let Some(idx) = find_match_index(buffer, pos.char, rope.char(pos.char))
    {
        return Some(Position::from_char(buffer, idx));
    }
    if pos.char > 0
        && let Some(idx) = find_match_index(buffer, pos.char - 1, rope.char(pos.char - 1))
    {
        return Some(Position::from_char(buffer, idx));
    }
    None
}

fn find_match_index(buffer: &Buffer, idx: usize, c: char) -> Option<usize> {
    for &(open, close) in &PAIRS {
        if c == open {
            return scan_forward_for_match(buffer, idx, open, close);
        }
        if c == close {
            return scan_backward_for_match(buffer, idx, open, close);
        }
    }
    None
}

fn scan_forward_for_match(buffer: &Buffer, from: usize, open: char, close: char) -> Option<usize> {
    let rope = buffer.rope();
    let len = rope.len_chars();
    let mut depth: i32 = 0;
    let mut idx = from;
    while idx < len {
        let c = rope.char(idx);
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
        idx += 1;
    }
    None
}

fn scan_backward_for_match(buffer: &Buffer, from: usize, open: char, close: char) -> Option<usize> {
    let rope = buffer.rope();
    let mut depth: i32 = 0;
    let mut idx = from;
    loop {
        let c = rope.char(idx);
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buffer: &Buffer, char_idx: usize) -> Position {
        Position::from_char(buffer, char_idx)
    }

    #[test]
    fn matches_from_the_opening_bracket() {
        let buffer = Buffer::from_bytes(b"foo(bar)baz").unwrap();
        let m = matching_bracket(&buffer, pos_at(&buffer, 3)).unwrap();
        assert_eq!(m.char, 7);
    }

    #[test]
    fn matches_from_right_after_the_closing_bracket() {
        let buffer = Buffer::from_bytes(b"foo(bar)baz").unwrap();
        let m = matching_bracket(&buffer, pos_at(&buffer, 8)).unwrap();
        assert_eq!(m.char, 3);
    }

    #[test]
    fn matches_from_right_before_the_closing_bracket() {
        let buffer = Buffer::from_bytes(b"foo(bar)baz").unwrap();
        let m = matching_bracket(&buffer, pos_at(&buffer, 7)).unwrap();
        assert_eq!(m.char, 3);
    }

    #[test]
    fn handles_nested_pairs_of_the_same_type() {
        let buffer = Buffer::from_bytes(b"(a(b)c)").unwrap();
        let outer = matching_bracket(&buffer, pos_at(&buffer, 0)).unwrap();
        assert_eq!(outer.char, 6);
        let inner = matching_bracket(&buffer, pos_at(&buffer, 2)).unwrap();
        assert_eq!(inner.char, 4);
    }

    #[test]
    fn handles_different_bracket_types_independently() {
        let buffer = Buffer::from_bytes(b"[a(b)c]").unwrap();
        let square = matching_bracket(&buffer, pos_at(&buffer, 0)).unwrap();
        assert_eq!(square.char, 6);
        let paren = matching_bracket(&buffer, pos_at(&buffer, 2)).unwrap();
        assert_eq!(paren.char, 4);
    }

    #[test]
    fn unbalanced_bracket_returns_none() {
        let buffer = Buffer::from_bytes(b"(a, b").unwrap();
        assert_eq!(matching_bracket(&buffer, pos_at(&buffer, 0)), None);
    }

    #[test]
    fn non_bracket_position_returns_none() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        assert_eq!(matching_bracket(&buffer, pos_at(&buffer, 2)), None);
    }
}
