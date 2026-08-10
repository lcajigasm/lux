//! Fuzzy string matching backed by `nucleo` (`docs/adr/0009-search-and-
//! fuzzy-matching.md`). The ADR flags `nucleo` as a maintenance risk and
//! asks for characterization tests around its ranking behavior so a fork
//! or hand-written replacement is cheap if it's ever needed — that's what
//! the tests below are for, not just coverage.

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};

/// Ranks `candidates` against `query`, highest score first. A candidate
/// that doesn't match `query` at all is dropped (`nucleo`'s own
/// `Pattern::match_list` semantics). Tuned for file paths: `/` is a
/// path-boundary delimiter, so a query matching right after a `/` scores
/// higher than the same letters matched mid-segment.
pub fn rank_paths<'a>(
    query: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<(&'a str, u32)> {
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    pattern.match_list(candidates, &mut matcher)
}

/// Ranks `items` against `query` by the text `key` extracts from each item,
/// highest score first, dropping non-matches. Like `rank_paths` but for
/// items that aren't bare strings themselves (e.g. commands, where two
/// items can share a display title) and without the path-boundary tuning —
/// plain default scoring for short, non-path labels.
pub fn rank_by<'a, T>(
    query: &str,
    items: impl IntoIterator<Item = &'a T>,
    key: impl Fn(&T) -> &str,
) -> Vec<(&'a T, u32)> {
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::default();
    let mut buf = Vec::new();
    let mut scored: Vec<(&'a T, u32)> = items
        .into_iter()
        .filter_map(|item| {
            pattern
                .score(Utf32Str::new(key(item), &mut buf), &mut matcher)
                .map(|score| (item, score))
        })
        .collect();
    scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    scored
}

#[cfg(test)]
mod tests {
    use super::{rank_by, rank_paths};

    #[test]
    fn drops_non_matching_candidates() {
        let results = rank_paths("goto", ["src/goto_anything.rs", "src/theme.rs"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "src/goto_anything.rs");
    }

    #[test]
    fn ranks_path_boundary_match_above_mid_segment_match() {
        // "ui" matches right after a `/` in the first path, and mid-segment
        // (inside "build") in the second — the path-boundary bonus from
        // `Config::match_paths` should rank the former higher.
        let results = rank_paths("ui", ["src/ui/mod.rs", "src/builder.rs"]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "src/ui/mod.rs");
        assert!(results[0].1 > results[1].1);
    }

    #[test]
    fn is_case_insensitive_for_lowercase_queries() {
        let results = rank_paths("readme", ["README.md"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "README.md");
    }

    #[test]
    fn respects_case_for_mixed_case_queries() {
        // `CaseMatching::Smart`: a query with an uppercase letter only
        // matches haystacks with matching case.
        let results = rank_paths("README", ["readme.md", "README.md"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "README.md");
    }

    #[test]
    fn empty_query_returns_all_candidates_unscored_in_order() {
        let results = rank_paths("", ["b.rs", "a.rs"]);
        assert_eq!(results, vec![("b.rs", 0), ("a.rs", 0)]);
    }

    #[test]
    fn allows_gaps_between_matched_characters() {
        // Fuzzy, not substring: "gan" matches "goto_anything.rs" via
        // g-o-t-o-_-a...n... with gaps, not as a contiguous run.
        let results = rank_paths("gan", ["goto_anything.rs"]);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn rank_by_extracts_the_match_key_and_keeps_item_identity() {
        #[derive(Debug, PartialEq, Eq)]
        struct Labeled(u32, &'static str);
        let items = [Labeled(1, "Format Document"), Labeled(2, "Format Document")];
        let results = rank_by("format", &items, |i| i.1);
        // Two items sharing a title are both kept, as themselves (not
        // collapsed or confused) — the reason `rank_by` exists instead of
        // reusing `rank_paths` for non-path candidates.
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.0, 1);
        assert_eq!(results[1].0.0, 2);
    }

    #[test]
    fn rank_by_drops_non_matching_items() {
        let items = [String::from("Save File"), String::from("Close Window")];
        let results = rank_by("save", &items, |s| s.as_str());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.as_str(), "Save File");
    }
}
