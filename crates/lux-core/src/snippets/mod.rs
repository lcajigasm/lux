//! Snippets with real, navigable placeholders: `Tab`/`Shift+Tab` move
//! between tab stops, mirrored placeholders (the same index appearing more
//! than once) update together, and defaults start out selected so typing
//! replaces them. This is what v0 never had — it stripped `$1`-style
//! syntax with a plain string `strip()` and inserted whatever was left, no
//! placeholder model at all (see ADR 0001).

mod parse;

use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::position::Position;
use crate::selection::Selection;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TabstopGroup {
    ranges: Vec<Range<usize>>,
}

/// An active snippet: the buffer now holds its expanded text, and this
/// tracks where each tab stop (and its mirrors) currently sit so `next`,
/// `prev`, and `update_current` can navigate and edit them as the
/// surrounding text keeps changing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetSession {
    tabstops: Vec<TabstopGroup>,
    current: usize,
}

/// Inserts `template` (parsed for `$1`/`${1:default}`/`$0`/mirror syntax)
/// at `at`, and returns the session positioned on the first tab stop.
pub fn insert(buffer: &mut Buffer, at: usize, template: &str) -> (Edit, SnippetSession) {
    let parsed = parse::parse(template);
    let edit = Edit::insert(at, parsed.text);
    edit.apply(buffer);

    let tabstops = parsed
        .tabstops
        .into_iter()
        .map(|t| TabstopGroup {
            ranges: t
                .ranges
                .into_iter()
                .map(|r| (at + r.start)..(at + r.end))
                .collect(),
        })
        .collect();
    (
        edit,
        SnippetSession {
            tabstops,
            current: 0,
        },
    )
}

impl SnippetSession {
    /// The selection for the tab stop the session is currently on — a
    /// default's range if it has one (so typing replaces it), otherwise a
    /// collapsed cursor.
    pub fn current_selection(&self, buffer: &Buffer) -> Selection {
        self.selection_for(buffer, self.current)
    }

    fn selection_for(&self, buffer: &Buffer, group: usize) -> Selection {
        let range = &self.tabstops[group].ranges[0];
        Selection::new(
            Position::from_char(buffer, range.start),
            Position::from_char(buffer, range.end),
        )
    }

    /// `true` once the session is on its last tab stop (`$0`, or the
    /// implicit end-of-snippet position) — `next` returns `None` from here,
    /// which is the caller's signal to exit snippet mode.
    pub fn is_on_last(&self) -> bool {
        self.current + 1 >= self.tabstops.len()
    }

    /// Moves to the next tab stop (`Tab`). `None` if already on the last
    /// one — the caller should exit snippet mode rather than call this
    /// again.
    pub fn next(&mut self, buffer: &Buffer) -> Option<Selection> {
        if self.is_on_last() {
            return None;
        }
        self.current += 1;
        Some(self.current_selection(buffer))
    }

    /// Moves to the previous tab stop (`Shift+Tab`). `None` if already on
    /// the first.
    pub fn prev(&mut self, buffer: &Buffer) -> Option<Selection> {
        if self.current == 0 {
            return None;
        }
        self.current -= 1;
        Some(self.current_selection(buffer))
    }

    /// Replaces the current tab stop's content with `new_text` — in every
    /// mirror at once, since they're the same placeholder. Every other tab
    /// stop's ranges (including this one's own, at their new length) are
    /// repositioned to stay correct as the buffer shifts around each edit.
    pub fn update_current(&mut self, buffer: &mut Buffer, new_text: &str) -> Vec<Edit> {
        let current = self.current;
        let new_len = new_text.chars().count();

        let mut flat: Vec<(usize, usize, Range<usize>, bool)> = Vec::new();
        for (group_idx, group) in self.tabstops.iter().enumerate() {
            for (mirror_idx, range) in group.ranges.iter().enumerate() {
                flat.push((group_idx, mirror_idx, range.clone(), group_idx == current));
            }
        }
        flat.sort_by_key(|(_, _, range, _)| range.start);

        let mut edits = Vec::new();
        let mut shift: isize = 0;
        let mut new_ranges: Vec<Option<Range<usize>>> = vec![None; flat.len()];

        for (flat_idx, (_, _, range, is_current)) in flat.iter().enumerate() {
            let adj_start = (range.start as isize + shift) as usize;
            if *is_current {
                let old_len = range.end - range.start;
                let delete_edit = Edit::delete(buffer, adj_start, old_len);
                delete_edit.apply(buffer);
                edits.push(delete_edit);
                if !new_text.is_empty() {
                    let insert_edit = Edit::insert(adj_start, new_text);
                    insert_edit.apply(buffer);
                    edits.push(insert_edit);
                }
                new_ranges[flat_idx] = Some(adj_start..(adj_start + new_len));
                shift += new_len as isize - old_len as isize;
            } else {
                let len = range.end - range.start;
                new_ranges[flat_idx] = Some(adj_start..(adj_start + len));
            }
        }

        for ((group_idx, mirror_idx, _, _), new_range) in flat.into_iter().zip(new_ranges) {
            self.tabstops[group_idx].ranges[mirror_idx] =
                new_range.expect("every flat entry was resolved above");
        }

        edits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_places_expanded_text_and_starts_on_the_first_tabstop() {
        let mut buffer = Buffer::from_bytes(b"before  after").unwrap();
        let (_, session) = insert(&mut buffer, 7, "let ${1:x} = ${2:0};");
        assert_eq!(buffer.text(), "before let x = 0; after");
        let sel = session.current_selection(&buffer);
        assert_eq!(sel.char_range(), 11..12); // "x", selected
    }

    #[test]
    fn next_advances_through_tabstops_in_order() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "${1:a}-${2:b}-${3:c}");
        assert_eq!(session.current_selection(&buffer).char_range(), 0..1);
        assert_eq!(session.next(&buffer).unwrap().char_range(), 2..3);
        assert_eq!(session.next(&buffer).unwrap().char_range(), 4..5);
    }

    #[test]
    fn next_reaches_the_implicit_final_tabstop_then_returns_none() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "${1:a}");
        assert!(session.next(&buffer).is_some()); // to the implicit $0 at the end
        assert!(session.is_on_last());
        assert!(session.next(&buffer).is_none());
    }

    #[test]
    fn explicit_zero_tabstop_is_always_navigated_to_last() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "$0 ${1:a} ${2:b}");
        session.next(&buffer); // -> tabstop 1
        session.next(&buffer); // -> tabstop 2
        let last = session.next(&buffer);
        assert!(last.is_none() || session.is_on_last());
        assert!(session.is_on_last());
        let sel = session.current_selection(&buffer);
        assert_eq!(sel.char_range(), 0..0); // the explicit $0, at the start
    }

    #[test]
    fn prev_moves_back_and_returns_none_at_the_start() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "${1:a}-${2:b}");
        session.next(&buffer);
        assert_eq!(session.prev(&buffer).unwrap().char_range(), 0..1);
        assert!(session.prev(&buffer).is_none());
    }

    #[test]
    fn update_current_replaces_every_mirror_at_once() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "${1:x} + $1 = 2 * $1");
        session.update_current(&mut buffer, "value");
        assert_eq!(buffer.text(), "value + value = 2 * value");
    }

    #[test]
    fn update_current_keeps_later_tabstops_correctly_positioned() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "${1:x} then ${2:y}");
        session.update_current(&mut buffer, "longer_name");
        assert_eq!(buffer.text(), "longer_name then y");
        let next_sel = session.next(&buffer).unwrap();
        assert_eq!(buffer.text_range(next_sel.char_range()), "y");
    }

    #[test]
    fn update_current_with_mirrors_before_and_after_shifts_both_correctly() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "start $2 $1 $2 end ${2:mid}");
        // tabstop 1 is the current (first) one; tabstop 2 has mirrors both
        // before and after it in the document.
        session.update_current(&mut buffer, "ONE");
        assert_eq!(buffer.text(), "start mid ONE mid end mid");
        let sel = session.next(&buffer).unwrap();
        assert_eq!(buffer.text_range(sel.char_range()), "mid");
    }

    #[test]
    fn update_current_is_undoable() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (insert_edit, mut session) = insert(&mut buffer, 0, "${1:x} $1");
        let update_edits = session.update_current(&mut buffer, "y");
        assert_eq!(buffer.text(), "y y");

        for edit in update_edits.iter().rev() {
            edit.invert().apply(&mut buffer);
        }
        insert_edit.invert().apply(&mut buffer);
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn shrinking_the_current_tabstop_still_updates_later_ones() {
        let mut buffer = Buffer::from_bytes(b"").unwrap();
        let (_, mut session) = insert(&mut buffer, 0, "${1:longname} then ${2:y}");
        session.update_current(&mut buffer, "x");
        assert_eq!(buffer.text(), "x then y");
        let sel = session.next(&buffer).unwrap();
        assert_eq!(buffer.text_range(sel.char_range()), "y");
    }
}
