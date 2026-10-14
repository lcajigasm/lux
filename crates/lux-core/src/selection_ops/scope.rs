//! Expand selection to scope (`Cmd+Shift+Space`) and expand selection to tag (`Cmd+Shift+A`).
//!
//! Expands outwards through delimiters:
//! 1. Interior of nearest enclosing quotes or brackets (excluding delimiters).
//! 2. Exterior of nearest enclosing quotes or brackets (including delimiters).
//! 3. Parent enclosing scopes iteratively.

use std::ops::Range;

use crate::buffer::Buffer;
use crate::position::Position;
use crate::selection::Selection;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScopeCandidate {
    inner: Range<usize>,
    outer: Range<usize>,
}

/// Expands `selection` to the enclosing scope: interior first, then exterior with delimiters.
pub fn expand_to_scope(buffer: &Buffer, selection: Selection) -> Selection {
    let range = selection.char_range();
    let candidates = find_all_enclosing_scopes(buffer, &range);

    for cand in candidates {
        if cand.inner != range && cand.inner.start <= range.start && range.end <= cand.inner.end {
            return Selection::new(
                Position::from_char(buffer, cand.inner.start),
                Position::from_char(buffer, cand.inner.end),
            );
        }
        if cand.inner == range && cand.outer != range {
            return Selection::new(
                Position::from_char(buffer, cand.outer.start),
                Position::from_char(buffer, cand.outer.end),
            );
        }
        if cand.outer != range && cand.outer.start <= range.start && range.end <= cand.outer.end {
            return Selection::new(
                Position::from_char(buffer, cand.outer.start),
                Position::from_char(buffer, cand.outer.end),
            );
        }
    }

    selection
}

/// Expands `selection` to the enclosing HTML/XML/JSX tag: interior first, then exterior with tags.
pub fn expand_to_tag(buffer: &Buffer, selection: Selection) -> Selection {
    let range = selection.char_range();
    let mut tags = find_enclosing_tags(buffer, &range);
    tags.sort_by_key(|c| c.outer.len());

    for tag in tags {
        if tag.inner != range && tag.inner.start <= range.start && range.end <= tag.inner.end {
            return Selection::new(
                Position::from_char(buffer, tag.inner.start),
                Position::from_char(buffer, tag.inner.end),
            );
        }
        if tag.inner == range && tag.outer != range {
            return Selection::new(
                Position::from_char(buffer, tag.outer.start),
                Position::from_char(buffer, tag.outer.end),
            );
        }
        if tag.outer != range && tag.outer.start <= range.start && range.end <= tag.outer.end {
            return Selection::new(
                Position::from_char(buffer, tag.outer.start),
                Position::from_char(buffer, tag.outer.end),
            );
        }
    }

    selection
}

fn find_all_enclosing_scopes(buffer: &Buffer, range: &Range<usize>) -> Vec<ScopeCandidate> {
    let mut candidates = Vec::new();

    // 1. Check enclosing quotes on the active line(s)
    candidates.extend(find_enclosing_quotes(buffer, range));

    // 2. Check enclosing bracket pairs
    candidates.extend(find_enclosing_brackets(buffer, range));

    // Sort candidates by size of outer range (smallest enclosing scope first)
    candidates.sort_by_key(|c| c.outer.len());
    candidates
}

fn find_enclosing_quotes(buffer: &Buffer, range: &Range<usize>) -> Vec<ScopeCandidate> {
    let mut candidates = Vec::new();
    let rope = buffer.rope();
    let len = rope.len_chars();
    if len == 0 {
        return candidates;
    }

    let start_pos = Position::from_char(buffer, range.start);
    let line_start = rope.line_to_char(start_pos.line);
    let line_end = if start_pos.line + 1 < rope.len_lines() {
        rope.line_to_char(start_pos.line + 1)
    } else {
        len
    };

    let quote_chars = ['"', '\'', '`'];
    for quote in quote_chars {
        let mut idx = line_start;
        while idx < line_end {
            if rope.char(idx) == quote && (idx == 0 || rope.char(idx - 1) != '\\') {
                let open_idx = idx;
                let mut close_idx = None;
                let mut search = open_idx + 1;
                while search < line_end {
                    if rope.char(search) == quote && rope.char(search - 1) != '\\' {
                        close_idx = Some(search);
                        break;
                    }
                    search += 1;
                }

                if let Some(close) = close_idx {
                    let outer = open_idx..close + 1;
                    let inner = (open_idx + 1)..close;
                    if outer.start <= range.start && range.end <= outer.end {
                        candidates.push(ScopeCandidate { inner, outer });
                    }
                    idx = close + 1;
                } else {
                    idx += 1;
                }
            } else {
                idx += 1;
            }
        }
    }

    candidates
}

fn find_enclosing_brackets(buffer: &Buffer, range: &Range<usize>) -> Vec<ScopeCandidate> {
    let mut candidates = Vec::new();
    let rope = buffer.rope();
    let len = rope.len_chars();
    if len == 0 {
        return candidates;
    }

    let bracket_pairs = [('(', ')'), ('[', ']'), ('{', '}')];

    for &(open, close) in &bracket_pairs {
        let mut search_pos = range.start;
        while let Some(open_idx) =
            scan_backward_for_unmatched_opener(buffer, search_pos, open, close)
        {
            if let Some(close_idx) = scan_forward_for_matching_closer(buffer, open_idx, open, close)
            {
                let outer = open_idx..close_idx + 1;
                let inner = (open_idx + 1)..close_idx;
                if outer.start <= range.start && range.end <= outer.end {
                    candidates.push(ScopeCandidate { inner, outer });
                }
            }

            if open_idx == 0 {
                break;
            }
            search_pos = open_idx - 1;
        }
    }

    candidates
}

fn scan_backward_for_unmatched_opener(
    buffer: &Buffer,
    from: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let rope = buffer.rope();
    let mut depth: i32 = 0;
    let mut idx = from;

    loop {
        let c = rope.char(idx);
        if c == close {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                return Some(idx);
            }
            depth -= 1;
        }

        if idx == 0 {
            break;
        }
        idx -= 1;
    }
    None
}

fn scan_forward_for_matching_closer(
    buffer: &Buffer,
    open_idx: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let rope = buffer.rope();
    let len = rope.len_chars();
    let mut depth: i32 = 0;
    let mut idx = open_idx;

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

fn find_enclosing_tags(buffer: &Buffer, range: &Range<usize>) -> Vec<ScopeCandidate> {
    let mut candidates = Vec::new();
    let rope = buffer.rope();
    let len = rope.len_chars();
    if len < 4 {
        return candidates;
    }

    let mut search_from = range.start.min(len.saturating_sub(1));
    while let Some(lt_idx) = scan_backward_for_char(buffer, search_from, '<') {
        if let Some((tag_name, tag_end)) = parse_opening_tag(buffer, lt_idx) {
            // Find matching closing tag </tag_name>
            if let Some((close_start, close_end)) =
                find_matching_closing_tag(buffer, tag_end, &tag_name)
            {
                let outer = lt_idx..close_end;
                let inner = tag_end..close_start;
                if outer.start <= range.start && range.end <= outer.end {
                    candidates.push(ScopeCandidate { inner, outer });
                }
            }
        }
        if lt_idx == 0 {
            break;
        }
        search_from = lt_idx - 1;
    }

    candidates
}

fn scan_backward_for_char(buffer: &Buffer, from: usize, target: char) -> Option<usize> {
    let rope = buffer.rope();
    let mut idx = from;
    loop {
        if rope.char(idx) == target {
            return Some(idx);
        }
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
    None
}

fn parse_opening_tag(buffer: &Buffer, lt_idx: usize) -> Option<(String, usize)> {
    let rope = buffer.rope();
    let len = rope.len_chars();
    if lt_idx + 1 >= len || rope.char(lt_idx + 1) == '/' || rope.char(lt_idx + 1) == '!' {
        return None;
    }

    let mut tag_name = String::new();
    let mut idx = lt_idx + 1;
    while idx < len {
        let c = rope.char(idx);
        if c.is_alphanumeric() || c == '-' || c == '_' {
            tag_name.push(c);
            idx += 1;
        } else {
            break;
        }
    }

    if tag_name.is_empty() {
        return None;
    }

    while idx < len {
        let c = rope.char(idx);
        if c == '>' {
            if idx > 0 && rope.char(idx - 1) == '/' {
                return None; // Self-closing tag
            }
            return Some((tag_name, idx + 1));
        }
        idx += 1;
    }
    None
}

fn find_matching_closing_tag(
    buffer: &Buffer,
    from: usize,
    tag_name: &str,
) -> Option<(usize, usize)> {
    let text = buffer.text();
    let target_close = format!("</{tag_name}>");
    let target_open = format!("<{tag_name}");

    let mut depth = 1;
    let mut search = from;

    while search < text.len() {
        let next_close = text[search..].find(&target_close);
        let next_open = text[search..].find(&target_open);

        match (next_open, next_close) {
            (Some(o_pos), Some(c_pos)) if o_pos < c_pos => {
                let open_idx = search + o_pos;
                // Verify it's actually an open tag and not </tag or tag prefix
                if let Some((parsed, end_pos)) = parse_opening_tag(buffer, open_idx)
                    && parsed == tag_name
                {
                    depth += 1;
                    search = end_pos;
                } else {
                    search = open_idx + 1;
                }
            }
            (_, Some(c_pos)) => {
                depth -= 1;
                let close_start = search + c_pos;
                let close_end = close_start + target_close.chars().count();
                if depth == 0 {
                    return Some((close_start, close_end));
                }
                search = close_end;
            }
            _ => break,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos_at(buf: &Buffer, c: usize) -> Position {
        Position::from_char(buf, c)
    }

    #[test]
    fn expand_to_scope_quotes() {
        let buf = Buffer::from_bytes(b"var x = \"hello world\";").unwrap();
        let sel = Selection::cursor(pos_at(&buf, 11)); // inside "hello world"

        let sel1 = expand_to_scope(&buf, sel);
        assert_eq!(buf.text_range(sel1.char_range()), "hello world");

        let sel2 = expand_to_scope(&buf, sel1);
        assert_eq!(buf.text_range(sel2.char_range()), "\"hello world\"");
    }

    #[test]
    fn expand_to_scope_brackets_and_nested() {
        let buf = Buffer::from_bytes(b"[1, (2 + 3), 4]").unwrap();
        let sel = Selection::cursor(pos_at(&buf, 7)); // at '2'

        let sel1 = expand_to_scope(&buf, sel);
        assert_eq!(buf.text_range(sel1.char_range()), "2 + 3");

        let sel2 = expand_to_scope(&buf, sel1);
        assert_eq!(buf.text_range(sel2.char_range()), "(2 + 3)");

        let sel3 = expand_to_scope(&buf, sel2);
        assert_eq!(buf.text_range(sel3.char_range()), "1, (2 + 3), 4");

        let sel4 = expand_to_scope(&buf, sel3);
        assert_eq!(buf.text_range(sel4.char_range()), "[1, (2 + 3), 4]");
    }

    #[test]
    fn expand_to_tag_html() {
        let buf = Buffer::from_bytes(b"<div class=\"container\"><span>Text</span></div>").unwrap();
        let sel = Selection::cursor(pos_at(&buf, 31)); // inside "Text"

        let sel1 = expand_to_tag(&buf, sel);
        assert_eq!(buf.text_range(sel1.char_range()), "Text");

        let sel2 = expand_to_tag(&buf, sel1);
        assert_eq!(buf.text_range(sel2.char_range()), "<span>Text</span>");

        let sel3 = expand_to_tag(&buf, sel2);
        assert_eq!(
            buf.text_range(sel3.char_range()),
            "<div class=\"container\"><span>Text</span></div>"
        );
    }
}
