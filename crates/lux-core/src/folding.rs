//! Code folding data model, range discovery, and visual line mapping.

use crate::buffer::Buffer;
use std::collections::BTreeMap;

/// A foldable range in a document: from `start_line` to `end_line` inclusive.
///
/// When folded, `start_line` remains visible as the header line,
/// and lines `(start_line + 1)..=end_line` are collapsed (hidden).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoldRange {
    pub start_line: usize,
    pub end_line: usize,
}

impl FoldRange {
    #[must_use]
    pub const fn new(start_line: usize, end_line: usize) -> Self {
        Self {
            start_line,
            end_line,
        }
    }

    /// Whether this range contains the given line (including `start_line`).
    #[must_use]
    pub const fn contains_line(&self, line: usize) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// Whether this range hides the given line when folded
    /// (`start_line` is kept visible, lines after it are hidden).
    #[must_use]
    pub const fn hides_line(&self, line: usize) -> bool {
        line > self.start_line && line <= self.end_line
    }
}

/// Computes indentation-based folding ranges for documents without an AST.
#[must_use]
pub fn indent_folding_ranges(buffer: &Buffer) -> Vec<FoldRange> {
    let total_lines = buffer.len_lines();
    if total_lines <= 1 {
        return Vec::new();
    }

    let mut indents: Vec<Option<usize>> = Vec::with_capacity(total_lines);
    for i in 0..total_lines {
        let text = buffer.line_content(i).unwrap_or_default();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            indents.push(None);
        } else {
            let leading = text.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            indents.push(Some(leading));
        }
    }

    let mut ranges = Vec::new();
    for i in 0..total_lines {
        let Some(base_indent) = indents[i] else {
            continue;
        };

        let mut end_line = None;
        for (j, &indent) in indents.iter().enumerate().skip(i + 1) {
            if let Some(indent) = indent {
                if indent > base_indent {
                    end_line = Some(j);
                } else {
                    break;
                }
            }
        }

        if let Some(end) = end_line
            && end > i
        {
            ranges.push(FoldRange::new(i, end));
        }
    }

    ranges.sort_by_key(|r| (r.start_line, std::cmp::Reverse(r.end_line)));
    ranges.dedup_by_key(|r| r.start_line);
    ranges
}

/// Tracks foldable and actively folded ranges for an open document,
/// and computes the mapping between visual lines and buffer lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoldState {
    foldable_ranges: Vec<FoldRange>,
    folded: BTreeMap<usize, FoldRange>,
}

impl FoldState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates available foldable ranges while preserving valid active folds.
    pub fn set_foldable_ranges(&mut self, mut ranges: Vec<FoldRange>) {
        ranges.sort_by_key(|r| (r.start_line, std::cmp::Reverse(r.end_line)));
        ranges.dedup_by_key(|r| r.start_line);
        self.foldable_ranges = ranges;

        self.folded.retain(|&start, active_range| {
            if let Some(new_range) = self.foldable_ranges.iter().find(|r| r.start_line == start) {
                *active_range = *new_range;
                true
            } else {
                active_range.end_line > start
            }
        });
    }

    #[must_use]
    pub fn foldable_ranges(&self) -> &[FoldRange] {
        &self.foldable_ranges
    }

    #[must_use]
    pub fn folded_ranges(&self) -> &BTreeMap<usize, FoldRange> {
        &self.folded
    }

    #[must_use]
    pub fn is_line_foldable(&self, line: usize) -> bool {
        self.foldable_ranges.iter().any(|r| r.start_line == line)
    }

    #[must_use]
    pub fn is_line_folded(&self, line: usize) -> bool {
        self.folded.contains_key(&line)
    }

    #[must_use]
    pub fn fold_range_for_line(&self, line: usize) -> Option<FoldRange> {
        self.foldable_ranges
            .iter()
            .find(|r| r.start_line == line)
            .copied()
    }

    #[must_use]
    pub fn is_line_hidden(&self, line: usize) -> bool {
        self.folded.range(..line).any(|(_, r)| r.hides_line(line))
    }

    /// Toggles the fold state of the block starting at `line`.
    pub fn toggle_fold(&mut self, line: usize) -> bool {
        if self.folded.remove(&line).is_some() {
            true
        } else if let Some(r) = self.fold_range_for_line(line) {
            self.folded.insert(line, r);
            true
        } else {
            false
        }
    }

    /// Folds the block starting at `line`. Returns true if newly folded.
    pub fn fold(&mut self, line: usize) -> bool {
        if self.folded.contains_key(&line) {
            return false;
        }
        if let Some(r) = self.fold_range_for_line(line) {
            self.folded.insert(line, r);
            true
        } else {
            false
        }
    }

    /// Unfolds the block starting at `line`. Returns true if was folded.
    pub fn unfold(&mut self, line: usize) -> bool {
        self.folded.remove(&line).is_some()
    }

    /// Unfolds all folded blocks in the file.
    pub fn unfold_all(&mut self) -> bool {
        if self.folded.is_empty() {
            false
        } else {
            self.folded.clear();
            true
        }
    }

    /// Folds all top-level (outermost) blocks in the document.
    pub fn fold_all(&mut self) {
        for range in &self.foldable_ranges {
            let is_enclosed = self.foldable_ranges.iter().any(|other| {
                other != range
                    && other.start_line <= range.start_line
                    && other.end_line >= range.end_line
            });
            if !is_enclosed {
                self.folded.insert(range.start_line, *range);
            }
        }
    }

    /// Folds all blocks at the specified nesting level (1 = top-level, 2 = nested once, etc.).
    pub fn fold_by_level(&mut self, target_level: usize) {
        if target_level == 0 {
            return;
        }
        for range in &self.foldable_ranges {
            let enclosing_count = self
                .foldable_ranges
                .iter()
                .filter(|other| {
                    *other != range
                        && other.start_line <= range.start_line
                        && other.end_line >= range.end_line
                })
                .count();
            let level = enclosing_count + 1;
            if level == target_level {
                self.folded.insert(range.start_line, *range);
            }
        }
    }

    /// Folds the innermost foldable block at or enclosing `cursor_line`.
    pub fn fold_cursor(&mut self, cursor_line: usize) -> bool {
        let mut candidate = None;
        let mut min_span = usize::MAX;
        for range in &self.foldable_ranges {
            if range.contains_line(cursor_line) && !self.folded.contains_key(&range.start_line) {
                let span = range.end_line - range.start_line;
                if span < min_span {
                    min_span = span;
                    candidate = Some(*range);
                }
            }
        }
        if let Some(range) = candidate {
            self.folded.insert(range.start_line, range);
            true
        } else {
            false
        }
    }

    /// Unfolds the innermost active fold at or enclosing `cursor_line`.
    pub fn unfold_cursor(&mut self, cursor_line: usize) -> bool {
        let mut candidate_key = None;
        let mut min_span = usize::MAX;
        for (&start, range) in &self.folded {
            if range.contains_line(cursor_line) {
                let span = range.end_line - range.start_line;
                if span < min_span {
                    min_span = span;
                    candidate_key = Some(start);
                }
            }
        }
        if let Some(key) = candidate_key {
            self.folded.remove(&key);
            true
        } else {
            false
        }
    }

    /// Merged hidden intervals `(start_line + 1)..=end_line` from all active folds.
    #[must_use]
    pub fn hidden_intervals(&self) -> Vec<(usize, usize)> {
        let mut intervals = Vec::new();
        for range in self.folded.values() {
            if range.end_line > range.start_line {
                intervals.push((range.start_line + 1, range.end_line));
            }
        }
        if intervals.is_empty() {
            return Vec::new();
        }
        intervals.sort_unstable();
        let mut merged = Vec::with_capacity(intervals.len());
        let (mut cur_s, mut cur_e) = intervals[0];
        for (s, e) in intervals.into_iter().skip(1) {
            if s <= cur_e + 1 {
                cur_e = cur_e.max(e);
            } else {
                merged.push((cur_s, cur_e));
                cur_s = s;
                cur_e = e;
            }
        }
        merged.push((cur_s, cur_e));
        merged
    }

    /// Total visual lines displayed in the viewport when folding is applied.
    #[must_use]
    pub fn total_visual_lines(&self, total_buffer_lines: usize) -> usize {
        let intervals = self.hidden_intervals();
        let mut hidden_count = 0;
        for (s, e) in intervals {
            let clamped_s = s.min(total_buffer_lines);
            let clamped_e = e.min(total_buffer_lines.saturating_sub(1));
            if clamped_e >= clamped_s {
                hidden_count += clamped_e - clamped_s + 1;
            }
        }
        total_buffer_lines.saturating_sub(hidden_count)
    }

    /// Maps a 0-indexed visual line index to its corresponding buffer line index.
    #[must_use]
    pub fn visual_to_buffer_line(&self, visual_line: usize, total_buffer_lines: usize) -> usize {
        if total_buffer_lines == 0 {
            return 0;
        }
        let intervals = self.hidden_intervals();
        let mut remaining = visual_line;
        let mut cur_buf = 0;

        for (s, e) in intervals {
            if cur_buf < s {
                let visible_chunk = s - cur_buf;
                if remaining < visible_chunk {
                    return cur_buf + remaining;
                }
                remaining -= visible_chunk;
            }
            cur_buf = e + 1;
        }

        (cur_buf + remaining).min(total_buffer_lines.saturating_sub(1))
    }

    /// Maps a buffer line index to its visual line index, or `None` if the line is hidden.
    #[must_use]
    pub fn buffer_to_visual_line(&self, buffer_line: usize) -> Option<usize> {
        let intervals = self.hidden_intervals();
        let mut hidden_before = 0;
        for (s, e) in intervals {
            if buffer_line >= s && buffer_line <= e {
                return None;
            }
            if buffer_line > e {
                hidden_before += e - s + 1;
            } else if buffer_line >= s {
                hidden_before += buffer_line - s;
            }
        }
        Some(buffer_line.saturating_sub(hidden_before))
    }

    /// Returns the next visible buffer line strictly after `current`.
    #[must_use]
    pub fn next_visible_line(&self, current: usize, total_buffer_lines: usize) -> usize {
        let mut next = current + 1;
        while next < total_buffer_lines && self.is_line_hidden(next) {
            next += 1;
        }
        next.min(total_buffer_lines.saturating_sub(1))
    }

    /// Returns the previous visible buffer line strictly before `current`.
    #[must_use]
    pub fn prev_visible_line(&self, current: usize) -> usize {
        if current == 0 {
            return 0;
        }
        let mut prev = current.saturating_sub(1);
        while prev > 0 && self.is_line_hidden(prev) {
            prev -= 1;
        }
        if self.is_line_hidden(prev) { 0 } else { prev }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indent_folding_ranges_detects_indented_blocks() {
        let text = "fn test() {\n    let a = 1;\n    let b = 2;\n}\n";
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let ranges = indent_folding_ranges(&buffer);
        assert_eq!(ranges, vec![FoldRange::new(0, 2)]);
    }

    #[test]
    fn fold_state_hides_lines_correctly() {
        let mut state = FoldState::new();
        state.set_foldable_ranges(vec![FoldRange::new(0, 3)]);
        assert!(state.is_line_foldable(0));
        assert!(!state.is_line_folded(0));

        assert!(state.toggle_fold(0));
        assert!(state.is_line_folded(0));
        assert!(!state.is_line_hidden(0));
        assert!(state.is_line_hidden(1));
        assert!(state.is_line_hidden(2));
        assert!(state.is_line_hidden(3));
        assert!(!state.is_line_hidden(4));

        assert_eq!(state.total_visual_lines(10), 7);
        assert_eq!(state.visual_to_buffer_line(0, 10), 0);
        assert_eq!(state.visual_to_buffer_line(1, 10), 4);
        assert_eq!(state.buffer_to_visual_line(0), Some(0));
        assert_eq!(state.buffer_to_visual_line(1), None);
        assert_eq!(state.buffer_to_visual_line(4), Some(1));
    }

    #[test]
    fn fold_cursor_and_unfold_cursor() {
        let mut state = FoldState::new();
        state.set_foldable_ranges(vec![FoldRange::new(0, 10), FoldRange::new(2, 5)]);

        assert!(state.fold_cursor(3));
        assert!(state.is_line_folded(2));
        assert!(!state.is_line_folded(0));

        assert!(state.fold_cursor(3));
        assert!(state.is_line_folded(0));

        assert!(state.unfold_cursor(3));
        assert!(!state.is_line_folded(2));
        assert!(state.is_line_folded(0));
    }

    #[test]
    fn fold_all_and_unfold_all() {
        let mut state = FoldState::new();
        state.set_foldable_ranges(vec![
            FoldRange::new(0, 5),
            FoldRange::new(1, 3),
            FoldRange::new(6, 10),
        ]);

        state.fold_all();
        assert!(state.is_line_folded(0));
        assert!(!state.is_line_folded(1));
        assert!(state.is_line_folded(6));

        assert!(state.unfold_all());
        assert!(state.folded.is_empty());
    }

    #[test]
    fn fold_by_level() {
        let mut state = FoldState::new();
        state.set_foldable_ranges(vec![
            FoldRange::new(0, 10),
            FoldRange::new(2, 8),
            FoldRange::new(4, 6),
        ]);

        state.fold_by_level(2);
        assert!(!state.is_line_folded(0));
        assert!(state.is_line_folded(2));
        assert!(!state.is_line_folded(4));
    }

    #[test]
    fn visual_navigation_skips_hidden_lines() {
        let mut state = FoldState::new();
        state.set_foldable_ranges(vec![FoldRange::new(2, 5)]);
        state.fold(2);

        assert_eq!(state.next_visible_line(2, 10), 6);
        assert_eq!(state.prev_visible_line(6), 2);
    }
}
