//! Diff rendering: side-by-side (split) and inline (unified) views of a
//! text diff, both built on `gutter::diff_hunks` — one diff engine in
//! `lux-git`, not a second one per view. Pure text transforms, no
//! repository access.

use crate::gutter::diff_hunks;

/// A single row of a side-by-side diff: `before`/`after` are `None` where
/// that side has nothing to show (padding, so the other side's changed
/// lines still line up visually).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideRow {
    pub before: Option<SideBySideLine>,
    pub after: Option<SideBySideLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideBySideLineKind {
    Context,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideLine {
    /// 1-based line number in its own file.
    pub number: u32,
    pub text: String,
    pub kind: SideBySideLineKind,
}

/// Builds a side-by-side (split) diff of `before` against `after`: every
/// line of both files, paired up so unchanged lines align and changed
/// hunks sit next to each other, with blank padding on whichever side has
/// fewer lines in a given hunk.
pub fn side_by_side(before: &str, after: &str) -> Vec<SideBySideRow> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let hunks = diff_hunks(before, after);

    let mut rows = Vec::new();
    let mut before_pos = 0usize;
    let mut after_pos = 0usize;

    for hunk in &hunks {
        let hunk_before_start = (hunk.before_start - 1) as usize;
        let hunk_after_start = (hunk.after_start - 1) as usize;
        push_context(
            &mut rows,
            &before_lines,
            &after_lines,
            &mut before_pos,
            &mut after_pos,
            hunk_before_start,
            hunk_after_start,
        );

        let before_len = hunk.before_len as usize;
        let after_len = hunk.after_len as usize;
        for i in 0..before_len.max(after_len) {
            rows.push(SideBySideRow {
                before: (i < before_len)
                    .then(|| line(&before_lines, before_pos + i, SideBySideLineKind::Changed)),
                after: (i < after_len)
                    .then(|| line(&after_lines, after_pos + i, SideBySideLineKind::Changed)),
            });
        }
        before_pos += before_len;
        after_pos += after_len;
    }

    push_context(
        &mut rows,
        &before_lines,
        &after_lines,
        &mut before_pos,
        &mut after_pos,
        before_lines.len(),
        after_lines.len(),
    );
    rows
}

/// Appends unchanged (context) rows until either side reaches its target
/// position, advancing both cursors in lockstep.
#[allow(clippy::too_many_arguments)]
fn push_context(
    rows: &mut Vec<SideBySideRow>,
    before_lines: &[&str],
    after_lines: &[&str],
    before_pos: &mut usize,
    after_pos: &mut usize,
    before_end: usize,
    after_end: usize,
) {
    while *before_pos < before_end && *after_pos < after_end {
        rows.push(SideBySideRow {
            before: Some(line(before_lines, *before_pos, SideBySideLineKind::Context)),
            after: Some(line(after_lines, *after_pos, SideBySideLineKind::Context)),
        });
        *before_pos += 1;
        *after_pos += 1;
    }
}

fn line(lines: &[&str], index: usize, kind: SideBySideLineKind) -> SideBySideLine {
    SideBySideLine {
        number: index as u32 + 1,
        text: lines[index].to_string(),
        kind,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDiffLineKind {
    Context,
    Removed,
    Added,
}

/// One line of an inline (unified) diff. `before_number` is `None` for an
/// added line, `after_number` is `None` for a removed line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineDiffLine {
    pub before_number: Option<u32>,
    pub after_number: Option<u32>,
    pub text: String,
    pub kind: InlineDiffLineKind,
}

/// Builds an inline (unified) diff of `before` against `after`: every
/// line in file order, in a single column — unchanged lines once, and
/// within a hunk its removed lines followed by its added lines, the
/// convention `git diff`/unified diff output uses.
pub fn inline_diff(before: &str, after: &str) -> Vec<InlineDiffLine> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let hunks = diff_hunks(before, after);

    let mut lines = Vec::new();
    let mut before_pos = 0usize;
    let mut after_pos = 0usize;

    for hunk in &hunks {
        let hunk_before_start = (hunk.before_start - 1) as usize;
        let hunk_after_start = (hunk.after_start - 1) as usize;
        push_inline_context(
            &mut lines,
            &before_lines,
            &mut before_pos,
            &mut after_pos,
            hunk_before_start,
            hunk_after_start,
        );

        let before_len = hunk.before_len as usize;
        for (offset, text) in before_lines[before_pos..before_pos + before_len]
            .iter()
            .enumerate()
        {
            lines.push(InlineDiffLine {
                before_number: Some((before_pos + offset) as u32 + 1),
                after_number: None,
                text: text.to_string(),
                kind: InlineDiffLineKind::Removed,
            });
        }
        before_pos += before_len;

        let after_len = hunk.after_len as usize;
        for (offset, text) in after_lines[after_pos..after_pos + after_len]
            .iter()
            .enumerate()
        {
            lines.push(InlineDiffLine {
                before_number: None,
                after_number: Some((after_pos + offset) as u32 + 1),
                text: text.to_string(),
                kind: InlineDiffLineKind::Added,
            });
        }
        after_pos += after_len;
    }

    push_inline_context(
        &mut lines,
        &before_lines,
        &mut before_pos,
        &mut after_pos,
        before_lines.len(),
        after_lines.len(),
    );
    lines
}

fn push_inline_context(
    lines: &mut Vec<InlineDiffLine>,
    before_lines: &[&str],
    before_pos: &mut usize,
    after_pos: &mut usize,
    before_end: usize,
    after_end: usize,
) {
    while *before_pos < before_end && *after_pos < after_end {
        lines.push(InlineDiffLine {
            before_number: Some(*before_pos as u32 + 1),
            after_number: Some(*after_pos as u32 + 1),
            text: before_lines[*before_pos].to_string(),
            kind: InlineDiffLineKind::Context,
        });
        *before_pos += 1;
        *after_pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_by_side_of_identical_text_is_all_context() {
        let text = "one\ntwo\nthree\n";
        let rows = side_by_side(text, text);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| {
            r.before.as_ref().unwrap().kind == SideBySideLineKind::Context
                && r.after.as_ref().unwrap().kind == SideBySideLineKind::Context
        }));
    }

    #[test]
    fn side_by_side_pairs_a_modified_line() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nTWO\nthree\n";
        let rows = side_by_side(before, after);
        assert_eq!(rows.len(), 3);
        let changed_row = &rows[1];
        assert_eq!(changed_row.before.as_ref().unwrap().text, "two");
        assert_eq!(changed_row.after.as_ref().unwrap().text, "TWO");
        assert_eq!(
            changed_row.before.as_ref().unwrap().kind,
            SideBySideLineKind::Changed
        );
    }

    #[test]
    fn side_by_side_pads_the_shorter_side_on_pure_insertion() {
        let before = "one\ntwo\n";
        let after = "one\nnew1\nnew2\ntwo\n";
        let rows = side_by_side(before, after);
        // context "one", two blank-before/added rows, context "two".
        assert_eq!(rows.len(), 4);
        assert!(rows[1].before.is_none());
        assert_eq!(rows[1].after.as_ref().unwrap().text, "new1");
        assert!(rows[2].before.is_none());
        assert_eq!(rows[2].after.as_ref().unwrap().text, "new2");
        assert_eq!(rows[3].before.as_ref().unwrap().text, "two");
    }

    #[test]
    fn side_by_side_pads_the_shorter_side_on_pure_deletion() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nthree\n";
        let rows = side_by_side(before, after);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].before.as_ref().unwrap().text, "two");
        assert!(rows[1].after.is_none());
    }

    #[test]
    fn inline_diff_of_identical_text_is_all_context() {
        let text = "one\ntwo\nthree\n";
        let lines = inline_diff(text, text);
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.kind == InlineDiffLineKind::Context));
        assert_eq!(lines[1].before_number, Some(2));
        assert_eq!(lines[1].after_number, Some(2));
    }

    #[test]
    fn inline_diff_shows_removed_lines_before_added_lines() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nTWO\nthree\n";
        let lines = inline_diff(before, after);
        assert_eq!(
            lines.len(),
            4,
            "context(one) + removed(two) + added(TWO) + context(three)"
        );
        assert_eq!(lines[0].kind, InlineDiffLineKind::Context);
        assert_eq!(lines[1].kind, InlineDiffLineKind::Removed);
        assert_eq!(lines[1].text, "two");
        assert_eq!(lines[1].before_number, Some(2));
        assert_eq!(lines[1].after_number, None);
        assert_eq!(lines[2].kind, InlineDiffLineKind::Added);
        assert_eq!(lines[2].text, "TWO");
        assert_eq!(lines[2].before_number, None);
        assert_eq!(lines[2].after_number, Some(2));
        assert_eq!(lines[3].kind, InlineDiffLineKind::Context);
    }

    #[test]
    fn inline_diff_of_pure_insertion_has_no_removed_lines() {
        let before = "one\ntwo\n";
        let after = "one\nnew\ntwo\n";
        let lines = inline_diff(before, after);
        assert!(!lines.iter().any(|l| l.kind == InlineDiffLineKind::Removed));
        let added: Vec<_> = lines
            .iter()
            .filter(|l| l.kind == InlineDiffLineKind::Added)
            .collect();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].text, "new");
    }

    #[test]
    fn inline_diff_of_pure_deletion_has_no_added_lines() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nthree\n";
        let lines = inline_diff(before, after);
        assert!(!lines.iter().any(|l| l.kind == InlineDiffLineKind::Added));
        let removed: Vec<_> = lines
            .iter()
            .filter(|l| l.kind == InlineDiffLineKind::Removed)
            .collect();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].text, "two");
    }
}
