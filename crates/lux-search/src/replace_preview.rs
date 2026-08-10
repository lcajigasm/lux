//! Replace in project, with a diff preview and per-file/per-hunk
//! confirmation (Phase 4 deliverable 5). Builds directly on `find_in_files`
//! (deliverable 4): `build_preview` takes the matches an already-run
//! search found and computes, for each, the before/after line — nothing
//! is written to disk at that point. Every hunk starts *unaccepted*; only
//! `accept`/`accept_all_in_file` opts one in, and `apply` only ever
//! touches accepted hunks, re-checking each one's original line against
//! the file's *current* content right before writing it. That's what
//! "never a blind mass replace" means concretely here: no hidden default
//! that replaces everything, and no silently clobbering a file that
//! changed since the preview was built.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::find_in_files::{SearchError, SearchMatch};

/// Opaque handle to one hunk, stable for the preview's lifetime (hunks are
/// never removed, only accepted/rejected, so a plain index suffices —
/// unlike `lux_workspace::RootId` or `lux_ui::file_tree::NodeId`, which
/// need to survive real removals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HunkId(usize);

/// One line-level change: `before` is the line exactly as it reads on
/// disk right now (including its line terminator, matching
/// `SearchMatch::line`'s own convention); `after` is what it would become.
/// `accepted` starts `false` — see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementHunk {
    pub id: HunkId,
    pub path: PathBuf,
    pub line_number: u64,
    pub before: String,
    pub after: String,
    pub accepted: bool,
}

/// A computed but unwritten set of replacements, grouped implicitly by
/// `path` across `hunks`.
#[derive(Debug, Clone, Default)]
pub struct ReplacementPreview {
    hunks: Vec<ReplacementHunk>,
}

/// What happened when accepted hunks for one file were written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileApplyOutcome {
    /// The file was read successfully; `applied` hunks matched the
    /// file's current content and were written, `conflicted` hunks
    /// didn't (something else changed that line since the preview was
    /// built) and were left untouched. A file is only actually rewritten
    /// if `applied` is non-empty.
    Applied {
        applied: Vec<HunkId>,
        conflicted: Vec<HunkId>,
    },
    /// The file couldn't be read or written (deleted, permissions, ...).
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileApplyResult {
    pub path: PathBuf,
    pub outcome: FileApplyOutcome,
}

/// Computes a preview from `matches` (typically `SearchStream::matches()`
/// after a `find_in_files` search): one [`ReplacementHunk`] per match,
/// `after` computed by expanding `replacement` against that match's line
/// with the same pattern (`$1`/`${name}` capture references work exactly
/// as `lux_core::find_replace`'s regex mode already does for in-buffer
/// replace — this is the same idea one layer up, across files instead of
/// within one buffer).
pub fn build_preview(
    matches: &[SearchMatch],
    pattern: &str,
    replacement: &str,
    case_insensitive: bool,
) -> Result<ReplacementPreview, SearchError> {
    let flagged = if case_insensitive {
        format!("(?i){pattern}")
    } else {
        pattern.to_owned()
    };
    let regex = Regex::new(&flagged).map_err(|e| SearchError::InvalidPattern(e.to_string()))?;

    let hunks = matches
        .iter()
        .enumerate()
        .map(|(i, m)| ReplacementHunk {
            id: HunkId(i),
            path: m.path.clone(),
            line_number: m.line_number,
            before: m.line.clone(),
            after: regex.replace_all(&m.line, replacement).into_owned(),
            accepted: false,
        })
        .collect();

    Ok(ReplacementPreview { hunks })
}

impl ReplacementPreview {
    pub fn hunks(&self) -> &[ReplacementHunk] {
        &self.hunks
    }

    pub fn hunk(&self, id: HunkId) -> Option<&ReplacementHunk> {
        self.hunks.get(id.0)
    }

    pub fn accepted_hunks(&self) -> impl Iterator<Item = &ReplacementHunk> {
        self.hunks.iter().filter(|h| h.accepted)
    }

    /// `false` if `id` doesn't belong to this preview.
    pub fn accept(&mut self, id: HunkId) -> bool {
        self.set_accepted(id, true)
    }

    /// `false` if `id` doesn't belong to this preview.
    pub fn reject(&mut self, id: HunkId) -> bool {
        self.set_accepted(id, false)
    }

    fn set_accepted(&mut self, id: HunkId, accepted: bool) -> bool {
        match self.hunks.get_mut(id.0) {
            Some(h) => {
                h.accepted = accepted;
                true
            }
            None => false,
        }
    }

    /// Accepts every hunk in `path` — the "confirm by file" half of the
    /// deliverable. Returns how many hunks changed state.
    pub fn accept_all_in_file(&mut self, path: &Path) -> usize {
        self.set_all_in_file(path, true)
    }

    pub fn reject_all_in_file(&mut self, path: &Path) -> usize {
        self.set_all_in_file(path, false)
    }

    fn set_all_in_file(&mut self, path: &Path, accepted: bool) -> usize {
        self.hunks
            .iter_mut()
            .filter(|h| h.path == path)
            .map(|h| h.accepted = accepted)
            .count()
    }

    /// Writes every accepted hunk to disk, one write per file (even when a
    /// file has several accepted hunks) — never blind: each hunk's
    /// `before` is re-checked against that line's *current* content right
    /// before writing, so a file edited by something else since the
    /// preview was built comes back as `conflicted`, not silently
    /// overwritten. A file with zero accepted hunks doesn't appear in the
    /// result at all.
    pub fn apply(&self) -> Vec<FileApplyResult> {
        let mut by_file: HashMap<&Path, Vec<&ReplacementHunk>> = HashMap::new();
        for hunk in self.accepted_hunks() {
            by_file.entry(hunk.path.as_path()).or_default().push(hunk);
        }

        by_file
            .into_iter()
            .map(|(path, hunks)| FileApplyResult {
                path: path.to_path_buf(),
                outcome: apply_hunks_to_file(path, &hunks),
            })
            .collect()
    }
}

fn apply_hunks_to_file(path: &Path, hunks: &[&ReplacementHunk]) -> FileApplyOutcome {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return FileApplyOutcome::Failed(e.to_string()),
    };
    let mut lines = split_keepends(&content);

    let mut applied = Vec::new();
    let mut conflicted = Vec::new();
    for hunk in hunks {
        let index = (hunk.line_number as usize).checked_sub(1);
        match index.and_then(|i| lines.get(i)) {
            Some(current) if *current == hunk.before => {
                lines[index.unwrap()] = hunk.after.clone();
                applied.push(hunk.id);
            }
            _ => conflicted.push(hunk.id),
        }
    }

    if !applied.is_empty() {
        let new_content: String = lines.concat();
        if let Err(e) = fs::write(path, new_content) {
            return FileApplyOutcome::Failed(e.to_string());
        }
    }

    FileApplyOutcome::Applied {
        applied,
        conflicted,
    }
}

/// Splits `text` into lines, each keeping its own terminator (`\n` or
/// `\r\n`) — so concatenating the (possibly edited) result reconstructs
/// the file byte-for-byte apart from the lines actually replaced, and so
/// each line compares equal to `SearchMatch::line`, which keeps its
/// terminator too.
fn split_keepends(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'\n' {
            lines.push(text[start..=i].to_string());
            start = i + 1;
        }
    }
    if start < text.len() {
        lines.push(text[start..].to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-search-replace-preview-test-{:?}-{}",
            std::thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(&dir).unwrap()
    }

    fn search_match(path: &Path, line_number: u64, line: &str) -> SearchMatch {
        SearchMatch {
            path: path.to_path_buf(),
            line_number,
            line: line.to_string(),
        }
    }

    #[test]
    fn build_preview_creates_one_hunk_per_match() {
        let path = PathBuf::from("a.rs");
        let matches = vec![
            search_match(&path, 1, "let foo = 1;\n"),
            search_match(&path, 3, "let foo = 2;\n"),
        ];
        let preview = build_preview(&matches, "foo", "bar", false).unwrap();
        assert_eq!(preview.hunks().len(), 2);
        assert_eq!(preview.hunks()[0].before, "let foo = 1;\n");
        assert_eq!(preview.hunks()[0].after, "let bar = 1;\n");
        assert_eq!(preview.hunks()[1].line_number, 3);
        assert!(preview.hunks().iter().all(|h| !h.accepted));
    }

    #[test]
    fn build_preview_expands_capture_groups() {
        let path = PathBuf::from("a.rs");
        let matches = vec![search_match(&path, 1, "name: alice, id: 7\n")];
        let preview = build_preview(&matches, r"(\w+): (\w+)", "$2=$1", false).unwrap();
        // `$2=$1` swaps each "key: value" pair to "value=key".
        assert_eq!(preview.hunks()[0].after, "alice=name, 7=id\n");
    }

    #[test]
    fn build_preview_is_case_insensitive_when_asked() {
        let path = PathBuf::from("a.rs");
        let matches = vec![search_match(&path, 1, "FOO bar\n")];
        let preview = build_preview(&matches, "foo", "baz", true).unwrap();
        assert_eq!(preview.hunks()[0].after, "baz bar\n");
    }

    #[test]
    fn build_preview_with_invalid_pattern_errors() {
        let matches = vec![search_match(&PathBuf::from("a.rs"), 1, "x\n")];
        let err = build_preview(&matches, "(unclosed", "y", false).unwrap_err();
        assert!(matches!(err, SearchError::InvalidPattern(_)));
    }

    #[test]
    fn accept_and_reject_control_what_apply_includes() {
        let path = PathBuf::from("a.rs");
        let matches = vec![
            search_match(&path, 1, "foo\n"),
            search_match(&path, 2, "foo\n"),
        ];
        let mut preview = build_preview(&matches, "foo", "bar", false).unwrap();
        let ids: Vec<HunkId> = preview.hunks().iter().map(|h| h.id).collect();

        assert!(preview.accept(ids[0]));
        assert_eq!(preview.accepted_hunks().count(), 1);

        assert!(preview.accept(ids[1]));
        assert_eq!(preview.accepted_hunks().count(), 2);

        assert!(preview.reject(ids[0]));
        assert_eq!(preview.accepted_hunks().count(), 1);
    }

    #[test]
    fn accept_reject_unknown_id_returns_false() {
        let mut preview = build_preview(&[], "foo", "bar", false).unwrap();
        assert!(!preview.accept(HunkId(0)));
        assert!(!preview.reject(HunkId(0)));
    }

    #[test]
    fn accept_all_in_file_only_affects_that_file() {
        let a = PathBuf::from("a.rs");
        let b = PathBuf::from("b.rs");
        let matches = vec![
            search_match(&a, 1, "foo\n"),
            search_match(&a, 2, "foo\n"),
            search_match(&b, 1, "foo\n"),
        ];
        let mut preview = build_preview(&matches, "foo", "bar", false).unwrap();

        let changed = preview.accept_all_in_file(&a);
        assert_eq!(changed, 2);
        assert_eq!(preview.accepted_hunks().count(), 2);
        assert!(preview.accepted_hunks().all(|h| h.path == a));
    }

    #[test]
    fn apply_writes_only_accepted_hunks_and_leaves_rejected_lines_untouched() {
        let dir = temp_dir();
        let path = dir.join("a.rs");
        fs::write(&path, "foo one\nfoo two\nfoo three\n").unwrap();

        let matches = vec![
            search_match(&path, 1, "foo one\n"),
            search_match(&path, 2, "foo two\n"),
            search_match(&path, 3, "foo three\n"),
        ];
        let mut preview = build_preview(&matches, "foo", "bar", false).unwrap();
        let ids: Vec<HunkId> = preview.hunks().iter().map(|h| h.id).collect();
        preview.accept(ids[0]);
        preview.accept(ids[2]);
        // ids[1] (line 2) stays rejected.

        let results = preview.apply();
        assert_eq!(results.len(), 1);
        match &results[0].outcome {
            FileApplyOutcome::Applied {
                applied,
                conflicted,
            } => {
                assert_eq!(applied.len(), 2);
                assert!(conflicted.is_empty());
            }
            other => panic!("expected Applied, got {other:?}"),
        }

        let on_disk = fs::read_to_string(&path).unwrap();
        assert_eq!(on_disk, "bar one\nfoo two\nbar three\n");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_groups_multiple_accepted_hunks_in_the_same_file_into_one_write() {
        let dir = temp_dir();
        let path = dir.join("a.rs");
        fs::write(&path, "foo\nfoo\n").unwrap();

        let matches = vec![
            search_match(&path, 1, "foo\n"),
            search_match(&path, 2, "foo\n"),
        ];
        let mut preview = build_preview(&matches, "foo", "bar", false).unwrap();
        preview.accept_all_in_file(&path);

        let results = preview.apply();
        // One file touched, even though it had two accepted hunks.
        assert_eq!(results.len(), 1);
        assert_eq!(fs::read_to_string(&path).unwrap(), "bar\nbar\n");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_detects_a_conflict_when_the_file_changed_since_the_preview() {
        let dir = temp_dir();
        let path = dir.join("a.rs");
        fs::write(&path, "foo\n").unwrap();

        let matches = vec![search_match(&path, 1, "foo\n")];
        let mut preview = build_preview(&matches, "foo", "bar", false).unwrap();
        let id = preview.hunks()[0].id;
        preview.accept(id);

        // Something else edits the file between preview and apply.
        fs::write(&path, "already changed\n").unwrap();

        let results = preview.apply();
        match &results[0].outcome {
            FileApplyOutcome::Applied {
                applied,
                conflicted,
            } => {
                assert!(applied.is_empty());
                assert_eq!(conflicted, &vec![id]);
            }
            other => panic!("expected Applied, got {other:?}"),
        }
        // The file was never rewritten — conflicted-only means no write.
        assert_eq!(fs::read_to_string(&path).unwrap(), "already changed\n");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_reports_failure_for_a_file_that_no_longer_exists() {
        let dir = temp_dir();
        let path = dir.join("gone.rs");
        // Never actually written to disk.
        let matches = vec![search_match(&path, 1, "foo\n")];
        let mut preview = build_preview(&matches, "foo", "bar", false).unwrap();
        preview.accept_all_in_file(&path);

        let results = preview.apply();
        assert!(matches!(results[0].outcome, FileApplyOutcome::Failed(_)));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_with_no_accepted_hunks_touches_nothing() {
        let dir = temp_dir();
        let path = dir.join("a.rs");
        fs::write(&path, "foo\n").unwrap();

        let matches = vec![search_match(&path, 1, "foo\n")];
        let preview = build_preview(&matches, "foo", "bar", false).unwrap();
        assert!(preview.apply().is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "foo\n");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let dir = temp_dir();
        let path = dir.join("a.rs");
        fs::write(&path, "foo\r\nbar\r\n").unwrap();

        let matches = vec![search_match(&path, 1, "foo\r\n")];
        let mut preview = build_preview(&matches, "foo", "baz", false).unwrap();
        preview.accept_all_in_file(&path);
        preview.apply();

        assert_eq!(fs::read_to_string(&path).unwrap(), "baz\r\nbar\r\n");

        fs::remove_dir_all(&dir).ok();
    }
}
