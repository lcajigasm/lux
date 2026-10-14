//! Auto-pairing, auto-closing of delimiters, smart step-over, and bracket matching.

use lux_core::edit_ops;
use lux_core::movement::matching_bracket;
use lux_core::multicursor::MultiCursor;
use lux_core::selection::Selection;

use crate::app::App;

const PAIR_DELIMITERS: [(char, char); 6] = [
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
];

/// Returns the closing counterpart for an opening delimiter, if recognized.
pub(crate) fn closing_for_open(open: char) -> Option<char> {
    PAIR_DELIMITERS
        .iter()
        .find(|(o, _)| *o == open)
        .map(|(_, c)| *c)
}

/// Returns true if `ch` is a recognized closing delimiter.
pub(crate) fn is_closing(ch: char) -> bool {
    PAIR_DELIMITERS.iter().any(|(_, c)| *c == ch)
}

/// Attempts to handle delimiter typing:
/// 1. If selections are non-empty and `ch` is an opening delimiter (or quote), wrap selections.
/// 2. If selection is empty and char immediately after cursor matches `ch` (closing delimiter or quote), step over.
/// 3. If selection is empty and `ch` is an opening delimiter, insert `open + close` and keep cursor inside.
///
/// Returns `true` if handled, `false` to proceed with regular character insertion.
pub(crate) fn handle_char_typed(app: &mut App, ch: char) -> bool {
    if app.editing_blocked() || app.buffer.is_huge_file_mode() {
        return false;
    }

    let has_selection = app.cursors.cursors().iter().any(|s| !s.is_empty());

    // 1. Wrap selection with delimiter pair
    if has_selection && let Some(close) = closing_for_open(ch) {
        let mut close_buf = [0u8; 4];
        let mut open_buf = [0u8; 4];
        let open_str = ch.encode_utf8(&mut open_buf);
        let close_str = close.encode_utf8(&mut close_buf);

        app.apply_edit_operation(app.cursors.clone(), |buffer, selection| {
            let range = selection.char_range();
            let selected_text = buffer.text_range(range);
            let wrapped = format!("{open_str}{selected_text}{close_str}");
            edit_ops::insert_text(buffer, selection, &wrapped)
        });
        return true;
    }

    // 2. Smart Step-Over: if cursor is sitting right before the matching closer
    if !has_selection && is_closing(ch) {
        let all_match_next = app.cursors.cursors().iter().all(|s| {
            if !s.is_empty() {
                return false;
            }
            app.buffer.char_at(s.head.char) == Some(ch)
        });

        if all_match_next && app.cursors.count() > 0 {
            // Step over: advance each cursor one character to the right
            let mut new_cursors = Vec::with_capacity(app.cursors.count());
            for &s in app.cursors.cursors() {
                let next = s.head.next_grapheme(&app.buffer);
                new_cursors.push(Selection::cursor(next));
            }
            app.cursors = MultiCursor::from_cursors(new_cursors);
            app.reset_goal_columns();
            return true;
        }
    }

    // 3. Auto-Close: insert open + close, place cursor in between
    if !has_selection && let Some(close) = closing_for_open(ch) {
        // Special guard for single-quote: if preceded by an alphanumeric char (e.g. "don't"),
        // don't auto-close, treat as an apostrophe.
        if ch == '\'' {
            let preceded_by_alphanumeric = app.cursors.cursors().iter().any(|s| {
                if s.head.char > 0 {
                    let prev_char = app.buffer.char_at(s.head.char - 1);
                    prev_char.is_some_and(|c| c.is_alphanumeric())
                } else {
                    false
                }
            });
            if preceded_by_alphanumeric {
                return false;
            }
        }

        let mut pair_str = String::with_capacity(8);
        pair_str.push(ch);
        pair_str.push(close);

        app.apply_edit_operation(app.cursors.clone(), |buffer, selection| {
            let (edits, after_sel) = edit_ops::insert_text(buffer, selection, &pair_str);
            // Move cursor back by 1 character so it rests between the pair
            let inside_pos = after_sel.head.prev_grapheme(buffer);
            (edits, Selection::cursor(inside_pos))
        });
        return true;
    }

    false
}

/// Smart Backspace: if every cursor is positioned between an empty pair (e.g. `(|)` or `{|}`),
/// delete both characters simultaneously.
///
/// Returns `true` if handled, `false` to proceed with regular backspace.
pub(crate) fn handle_backspace(app: &mut App) -> bool {
    if app.editing_blocked() || app.buffer.is_huge_file_mode() {
        return false;
    }

    let all_between_pair = app.cursors.cursors().iter().all(|s| {
        if !s.is_empty() {
            return false;
        }
        let pos = s.head.char;
        if pos > 0 && pos < app.buffer.len_chars() {
            let prev = app.buffer.char_at(pos - 1);
            let next = app.buffer.char_at(pos);
            match (prev, next) {
                (Some(p), Some(n)) => PAIR_DELIMITERS.iter().any(|&(o, c)| o == p && c == n),
                _ => false,
            }
        } else {
            false
        }
    });

    if all_between_pair && app.cursors.count() > 0 {
        // Expand each cursor's deletion range to cover [pos - 1, pos + 1]
        let mut del_ranges = Vec::with_capacity(app.cursors.count());
        for &s in app.cursors.cursors() {
            let left = s.head.prev_grapheme(&app.buffer);
            let right = s.head.next_grapheme(&app.buffer);
            del_ranges.push(Selection::new(left, right));
        }
        app.replace_selections(MultiCursor::from_cursors(del_ranges), "");
        return true;
    }

    false
}

/// Jumps cursor to the matching bracket if the cursor touches one.
pub(crate) fn jump_to_matching_bracket(app: &mut App) -> bool {
    let mut moved_any = false;
    let mut new_cursors = Vec::with_capacity(app.cursors.count());

    for &sel in app.cursors.cursors() {
        if let Some(target) = matching_bracket(&app.buffer, sel.head) {
            new_cursors.push(Selection::cursor(target));
            moved_any = true;
        } else {
            new_cursors.push(sel);
        }
    }

    if moved_any {
        app.cursors = MultiCursor::from_cursors(new_cursors);
        app.reset_goal_columns();
        app.reveal_primary_cursor();
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
        true
    } else {
        false
    }
}
