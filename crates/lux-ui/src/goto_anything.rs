//! Goto Anything (`Cmd+P`, `LUX_PROMPT_MAESTRO.md` Phase 3 deliverable 6):
//! fuzzy matching over project files, with Sublime-style `file:line`
//! suffix parsing, recent-file history, and preview. The fuzzy engine
//! itself lives in `lux-search::fuzzy`
//! (`docs/adr/0009-search-and-fuzzy-matching.md`); this module owns the
//! query syntax and wires it to that engine.
//!
//! `@symbol` and `#text` suffixes are **not** here — they're deferred to
//! Phase 7 (`lux-lsp`) and Phase 4 (`lux-search`'s Find in Files)
//! respectively, since building either now would mean duplicating
//! infrastructure those phases already own; see
//! `docs/adr/0014-goto-anything-symbol-text-suffixes-deferred.md`.
//!
//! This module accepts a caller-supplied candidate list rather than
//! walking the filesystem itself (real workspace indexing is Phase 4's
//! job), and `preview` takes an already-loaded `Buffer` rather than a
//! path — reading a file is I/O, which belongs off the UI thread (golden
//! rule #6), not in this module.

use crate::editor_view::line_index::LineIndex;
use lux_core::buffer::Buffer;
use lux_search::fuzzy::rank_paths;
use std::collections::HashMap;

/// A parsed Goto Anything query: the fuzzy text plus optional
/// Sublime-style `:line`, `:line:col`, `@symbol`, or `#term` suffix/prefix.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    pub text: String,
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub symbol: Option<String>,
    pub term: Option<String>,
}

impl Query {
    pub fn parse(raw: &str) -> Self {
        // 1. Symbol mode: "@symbol" or "file@symbol"
        if let Some((file_part, sym_part)) = raw.split_once('@') {
            return Query {
                text: file_part.to_string(),
                line: None,
                col: None,
                symbol: Some(sym_part.to_string()),
                term: None,
            };
        }

        // 2. Term search mode: "#term" or "file#term"
        if let Some((file_part, term_part)) = raw.split_once('#') {
            return Query {
                text: file_part.to_string(),
                line: None,
                col: None,
                symbol: None,
                term: Some(term_part.to_string()),
            };
        }

        // 3. Line mode: ":line", ":line:col", "file:line", "file:line:col"
        if let Some(colon_idx) = raw.find(':') {
            let file_part = &raw[..colon_idx];
            let after_colon = &raw[colon_idx + 1..];

            if let Some((line_part, col_part)) = after_colon.split_once(':')
                && let Ok(line @ 1..) = line_part.parse::<u32>()
                && let Ok(col @ 1..) = col_part.parse::<u32>()
            {
                return Query {
                    text: file_part.to_string(),
                    line: Some(line),
                    col: Some(col),
                    symbol: None,
                    term: None,
                };
            }

            if let Ok(line @ 1..) = after_colon.parse::<u32>() {
                return Query {
                    text: file_part.to_string(),
                    line: Some(line),
                    col: None,
                    symbol: None,
                    term: None,
                };
            }

            if colon_idx == 0 && after_colon.is_empty() {
                return Query {
                    text: String::new(),
                    line: None,
                    col: None,
                    symbol: None,
                    term: None,
                };
            }
        }

        Query {
            text: raw.to_string(),
            line: None,
            col: None,
            symbol: None,
            term: None,
        }
    }
}

/// One ranked Goto Anything result: a candidate path, its fuzzy score, and
/// the line/col to jump to (if the query had a `:line` or `:line:col` suffix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match<'a> {
    pub path: &'a str,
    pub score: u32,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

/// Parses `raw_query`'s suffix, then fuzzy-ranks `candidates` against the remaining text.
pub fn search<'a>(
    raw_query: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<Match<'a>> {
    let query = Query::parse(raw_query);
    rank_paths(&query.text, candidates)
        .into_iter()
        .map(|(path, score)| Match {
            path,
            score,
            line: query.line,
            col: query.col,
        })
        .collect()
}

/// Tracks recently opened files with a logical clock (not a wall-clock
/// timestamp — only relative order is ever needed, and it keeps this
/// deterministic and testable), the same technique
/// `command_palette::CommandRegistry` uses for command recency. The
/// "historial de recientes" this deliverable names.
#[derive(Debug, Clone, Default)]
pub struct RecentFiles {
    last_opened: HashMap<String, u64>,
    clock: u64,
}

impl RecentFiles {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `path` as opened just now.
    pub fn record_open(&mut self, path: &str) {
        self.clock += 1;
        self.last_opened.insert(path.to_string(), self.clock);
    }

    fn recency_of(&self, path: &str) -> u64 {
        self.last_opened.get(path).copied().unwrap_or(0)
    }
}

/// Like [`search`], but recency-aware. An empty query (no fuzzy text —
/// the palette just opened, nothing typed yet) lists every candidate
/// most-recently-opened first, never-opened candidates last in their
/// original order — the "recent files" view a query-less Goto Anything
/// shows. A non-empty query still ranks by fuzzy score first, breaking
/// ties between equally good matches by recency, same as
/// `command_palette::CommandRegistry::search`.
pub fn search_with_recency<'a>(
    raw_query: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    recent: &RecentFiles,
) -> Vec<Match<'a>> {
    let query = Query::parse(raw_query);
    if query.text.is_empty() {
        let mut candidates: Vec<&str> = candidates.into_iter().collect();
        candidates.sort_by_key(|path| std::cmp::Reverse(recent.recency_of(path)));
        return candidates
            .into_iter()
            .map(|path| Match {
                path,
                score: 0,
                line: query.line,
                col: query.col,
            })
            .collect();
    }

    let mut results = search(raw_query, candidates);
    results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| recent.recency_of(b.path).cmp(&recent.recency_of(a.path)))
    });
    results
}

/// A text snippet from `buffer` around `line` (or the file's start if
/// `line` is `None`), `radius_lines` of context on each side — the
/// preview shown next to a Goto Anything result.
#[must_use]
pub fn preview(buffer: &Buffer, line: Option<u32>, radius_lines: usize) -> String {
    let text = buffer.text();
    let index = LineIndex::build(&text);
    let total_lines = index.len();
    let target_line = line
        .map(|l| ((l as usize).saturating_sub(1)).min(total_lines.saturating_sub(1)))
        .unwrap_or(0);
    let start_line = target_line.saturating_sub(radius_lines);
    let end_line = (target_line + radius_lines + 1).min(total_lines);
    let start_byte = index.line_byte_range(&text, start_line).start;
    let end_byte = index.line_byte_range(&text, end_line.saturating_sub(1)).end;
    text[start_byte..end_byte].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text_has_no_line() {
        assert_eq!(
            Query::parse("main.rs"),
            Query {
                text: "main.rs".to_string(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_file_line_suffix() {
        assert_eq!(
            Query::parse("src/main.rs:42"),
            Query {
                text: "src/main.rs".to_string(),
                line: Some(42),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_file_line_col_suffix() {
        assert_eq!(
            Query::parse("src/main.rs:42:15"),
            Query {
                text: "src/main.rs".to_string(),
                line: Some(42),
                col: Some(15),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_symbol_mode() {
        assert_eq!(
            Query::parse("@"),
            Query {
                text: String::new(),
                symbol: Some(String::new()),
                ..Default::default()
            }
        );
        assert_eq!(
            Query::parse("@build_frame"),
            Query {
                text: String::new(),
                symbol: Some("build_frame".to_string()),
                ..Default::default()
            }
        );
        assert_eq!(
            Query::parse("main.rs@build_frame"),
            Query {
                text: "main.rs".to_string(),
                symbol: Some("build_frame".to_string()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_term_mode() {
        assert_eq!(
            Query::parse("#render"),
            Query {
                text: String::new(),
                term: Some("render".to_string()),
                ..Default::default()
            }
        );
        assert_eq!(
            Query::parse("main.rs#render"),
            Query {
                text: "main.rs".to_string(),
                term: Some("render".to_string()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_rejects_zero_as_a_line_number() {
        // Sublime's suffix is 1-based; ":0" isn't a valid target line, so
        // it's kept as literal query text instead of being silently
        // clamped or dropped.
        assert_eq!(
            Query::parse("main.rs:0"),
            Query {
                text: "main.rs:0".to_string(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_rejects_non_numeric_suffix() {
        assert_eq!(
            Query::parse("main.rs:bar"),
            Query {
                text: "main.rs:bar".to_string(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_rejects_trailing_colon_with_no_digits() {
        // Covers the transient state while the user is still typing the
        // line number after the colon.
        assert_eq!(
            Query::parse("main.rs:"),
            Query {
                text: "main.rs:".to_string(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn parse_line_only_suffix_leaves_empty_text() {
        assert_eq!(
            Query::parse(":42"),
            Query {
                text: String::new(),
                line: Some(42),
                ..Default::default()
            }
        );
        assert_eq!(
            Query::parse(":42:15"),
            Query {
                text: String::new(),
                line: Some(42),
                col: Some(15),
                ..Default::default()
            }
        );
    }

    #[test]
    fn search_ranks_matches_and_attaches_parsed_line() {
        let results = search("main:10", ["src/main.rs", "src/lib.rs", "src/mainframe.rs"]);
        assert_eq!(results.len(), 2);
        for m in &results {
            assert_eq!(m.line, Some(10));
        }
        assert!(results.iter().any(|m| m.path == "src/main.rs"));
        assert!(results.iter().any(|m| m.path == "src/mainframe.rs"));
        assert!(!results.iter().any(|m| m.path == "src/lib.rs"));
    }

    #[test]
    fn search_without_suffix_has_no_line() {
        let results = search("lib", ["src/lib.rs"]);
        assert_eq!(
            results,
            vec![Match {
                path: "src/lib.rs",
                score: results[0].score,
                line: None,
                col: None,
            }]
        );
    }

    #[test]
    fn search_with_empty_text_returns_all_candidates_for_jump_to_line() {
        let results = search(":7", ["b.rs", "a.rs"]);
        assert_eq!(
            results,
            vec![
                Match {
                    path: "b.rs",
                    score: 0,
                    line: Some(7),
                    col: None,
                },
                Match {
                    path: "a.rs",
                    score: 0,
                    line: Some(7),
                    col: None,
                },
            ]
        );
    }

    #[test]
    fn search_excludes_non_matching_candidates() {
        let results = search("goto", ["src/goto_anything.rs", "src/theme.rs"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "src/goto_anything.rs");
    }

    #[test]
    fn search_with_recency_empty_query_lists_most_recently_opened_first() {
        let mut recent = RecentFiles::new();
        recent.record_open("b.rs");
        recent.record_open("a.rs");

        let results: Vec<&str> = search_with_recency("", ["a.rs", "b.rs", "c.rs"], &recent)
            .into_iter()
            .map(|m| m.path)
            .collect();
        // "a.rs" opened most recently, then "b.rs", then never-opened
        // "c.rs" last (in its original position among the never-opened).
        assert_eq!(results, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn search_with_recency_never_opened_candidates_are_never_dropped() {
        let recent = RecentFiles::new();
        let results = search_with_recency("", ["x.rs", "y.rs"], &recent);
        assert_eq!(results.len(), 2);
        for m in &results {
            assert_eq!(m.score, 0);
        }
    }

    #[test]
    fn search_with_recency_breaks_fuzzy_score_ties_by_recency() {
        let mut recent = RecentFiles::new();
        recent.record_open("b/main.rs");

        let results: Vec<&str> = search_with_recency("main", ["a/main.rs", "b/main.rs"], &recent)
            .into_iter()
            .map(|m| m.path)
            .collect();
        assert_eq!(results, vec!["b/main.rs", "a/main.rs"]);
    }

    #[test]
    fn search_with_recency_still_ranks_by_fuzzy_quality_first() {
        // "a/main.rs" matches "main" as a tight, contiguous run right after
        // a path boundary; "b/manual_index.rs" only matches it with wide
        // gaps (m-a-...-i-...-n). Recency alone should never let the
        // clearly worse fuzzy match win.
        let mut recent = RecentFiles::new();
        recent.record_open("b/manual_index.rs");

        let results = search_with_recency("main", ["a/main.rs", "b/manual_index.rs"], &recent);
        assert_eq!(results[0].path, "a/main.rs");
    }

    #[test]
    fn preview_returns_context_around_the_target_line() {
        let buffer = Buffer::from_bytes(b"one\ntwo\nthree\nfour\nfive\n").unwrap();
        let snippet = preview(&buffer, Some(3), 1);
        assert_eq!(snippet, "two\nthree\nfour");
    }

    #[test]
    fn preview_without_a_line_starts_at_the_top_of_the_file() {
        let buffer = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        let snippet = preview(&buffer, None, 1);
        assert_eq!(snippet, "one\ntwo");
    }

    #[test]
    fn preview_clamps_a_line_past_the_end_of_the_file() {
        let buffer = Buffer::from_bytes(b"one\ntwo\n").unwrap();
        let snippet = preview(&buffer, Some(999), 1);
        assert!(snippet.contains("two"));
    }
}
