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
    let matched = pattern.match_list(candidates, &mut matcher);
    if query.trim().is_empty() {
        return matched;
    }
    // Smart case: an uppercase letter in the query is significant.
    let sensitive = query.chars().any(|ch| ch.is_uppercase());
    let mut rows: Vec<Scored<'_>> = matched
        .into_iter()
        .enumerate()
        .map(|(index, (path, score))| {
            let quality = alignment(path, query, sensitive);
            Scored {
                index,
                path,
                score: quality
                    .as_ref()
                    .map(|quality| quality.adjust(score))
                    .unwrap_or(score),
                tight: quality.is_some_and(|quality| quality.tight()),
            }
        })
        .collect();
    // A contiguous run or a word-initial match makes a gappy hit noise.
    // "mon" then keeps monokai.rs and drops sample-utf8-bom.bin, whose
    // m, o, and n are the letters of "sample", "bom", and ".bin".
    if rows.iter().any(|row| row.tight) {
        rows.retain(|row| row.tight);
    }
    rows.sort_by_key(|row| (std::cmp::Reverse(row.score), row.index));
    rows.into_iter().map(|row| (row.path, row.score)).collect()
}

struct Scored<'a> {
    index: usize,
    path: &'a str,
    score: u32,
    tight: bool,
}

struct Quality {
    /// Unmatched characters inside the matched span.
    gap: u32,
    contiguous: bool,
    /// Every matched character sits on a word boundary (`foo-bar`, `/`, camel).
    acronym: bool,
}

impl Quality {
    fn tight(&self) -> bool {
        self.contiguous || self.acronym
    }

    fn adjust(&self, nucleo: u32) -> u32 {
        // Bands stay apart, and inside a band each skipped character costs
        // more than nucleo's whole score. A long gap cannot outrank a short one.
        let band: u32 = if self.contiguous {
            2
        } else if self.acronym {
            1
        } else {
            0
        };
        let gap_rank = 20_000u32.saturating_sub(self.gap.saturating_mul(100));
        band.saturating_mul(100_000)
            .saturating_add(gap_rank)
            .saturating_add(nucleo.min(9_999))
    }
}

fn alignment(haystack: &str, query: &str, sensitive: bool) -> Option<Quality> {
    let h: Vec<char> = haystack.chars().collect();
    let q: Vec<char> = query
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .map(|ch| {
            if sensitive {
                ch
            } else {
                ch.to_lowercase().next().unwrap_or(ch)
            }
        })
        .collect();
    if q.is_empty() || h.is_empty() {
        return None;
    }
    let eq = |left: char, right: char| {
        if sensitive {
            left == right
        } else {
            left.to_lowercase().next().unwrap_or(left) == right
        }
    };
    let n = h.len();
    const NONE: u32 = u32::MAX / 4;
    let mut gap = vec![NONE; n];
    let mut acronym = vec![false; n];
    for (j, ch) in h.iter().copied().enumerate() {
        if eq(ch, q[0]) {
            gap[j] = 0;
            acronym[j] = on_boundary(&h, j);
        }
    }
    for &needle in q.iter().skip(1) {
        let mut best: Option<(i64, bool)> = None;
        let mut next_gap = vec![NONE; n];
        let mut next_acronym = vec![false; n];
        for j in 0..n {
            if let Some((key, acro)) = best
                && eq(h[j], needle)
            {
                next_gap[j] = (key + j as i64 - 1) as u32;
                next_acronym[j] = acro && on_boundary(&h, j);
            }
            if gap[j] < NONE {
                let key = gap[j] as i64 - j as i64;
                let take = match best {
                    None => true,
                    Some((prev, prev_acro)) => {
                        key < prev || (key == prev && acronym[j] && !prev_acro)
                    }
                };
                if take {
                    best = Some((key, acronym[j]));
                }
            }
        }
        gap = next_gap;
        acronym = next_acronym;
    }
    let (j, span) = gap
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, span)| *span < NONE)
        .min_by_key(|(_, span)| *span)?;
    Some(Quality {
        gap: span,
        contiguous: span == 0,
        acronym: acronym[j],
    })
}

fn on_boundary(chars: &[char], index: usize) -> bool {
    let Some(prev) = index.checked_sub(1).map(|prev| chars[prev]) else {
        return true;
    };
    let cur = chars[index];
    matches!(prev, '/' | '\\' | '-' | '_' | '.' | ' ' | ':')
        || (prev.is_lowercase() && cur.is_uppercase())
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
    use nucleo::pattern::{CaseMatching, Normalization, Pattern};
    use nucleo::{Config, Matcher};

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
    fn query_mon_on_the_compare_fixtures_keeps_only_monokai() {
        let files = [
            "characters.txt",
            "monokai.rs",
            "sample-latin1.bin",
            "sample-utf16-be-bom.bin",
            "sample-utf16-be-nobom.bin",
            "sample-utf16-le-bom.bin",
            "sample-utf16-le-nobom.bin",
            "sample-utf8-bom.bin",
            "sample-windows-1252.bin",
        ];
        let pattern = Pattern::parse("mon", CaseMatching::Smart, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let raw: Vec<&str> = pattern
            .match_list(files, &mut matcher)
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        assert!(
            raw.iter().any(|path| path.starts_with("sample-")),
            "the unfiltered matcher must still see the scattered bin names, got {raw:?}"
        );
        let results = rank_paths("mon", files);
        assert_eq!(
            results.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
            vec!["monokai.rs"]
        );
    }

    #[test]
    fn word_initials_stay_when_a_contiguous_match_drops_scattered_names() {
        let results = rank_paths(
            "mon",
            ["sample-utf8-bom.bin", "my-other-notes.rs", "monokai.rs"],
        );
        let names: Vec<&str> = results.iter().map(|(path, _)| *path).collect();
        assert_eq!(names[0], "monokai.rs");
        assert!(names.contains(&"my-other-notes.rs"));
        assert!(!names.iter().any(|path| path.ends_with(".bin")));
    }

    #[test]
    fn gaps_are_penalized_when_every_match_is_scattered() {
        let results = rank_paths("gan", ["gxxxxxxxxaxxxxxxxn.rs", "goto_anything.rs"]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "goto_anything.rs");
        assert!(results[0].1 > results[1].1);
    }

    #[test]
    fn rank_by_drops_non_matching_items() {
        let items = [String::from("Save File"), String::from("Close Window")];
        let results = rank_by("save", &items, |s| s.as_str());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.as_str(), "Save File");
    }
}
