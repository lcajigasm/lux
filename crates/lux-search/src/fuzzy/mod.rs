//! Fuzzy string matching backed by `nucleo` (`docs/adr/0009-search-and-
//! fuzzy-matching.md`). The ADR flags `nucleo` as a maintenance risk and
//! asks for characterization tests around its ranking behavior so a fork
//! or hand-written replacement is cheap if it's ever needed — that's what
//! the tests below are for, not just coverage.

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use std::ops::Range;

mod align;

/// One ranked result: the item, its score, and the character (not byte)
/// indices the matcher aligned the query to, ascending. A list highlights
/// exactly `indices`; [`char_spans`] turns them into byte ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked<T> {
    pub item: T,
    pub score: u32,
    pub indices: Vec<u32>,
}

pub use align::{GAP_STEP, MAX_GAP};

/// Ranks `candidates` against `query`, highest score first. A candidate
/// that doesn't match `query` at all is dropped (`nucleo`'s own
/// `Pattern::match_list` semantics); every candidate that matches is kept.
/// Tuned for file paths, in strict tiers: a contiguous run beats a
/// word-initial (acronym) match, which beats a scattered one; inside each,
/// a match starting a word beats one that doesn't, and a match wholly
/// inside the file name beats one reaching into the directory; then fewer
/// skipped characters win; `nucleo`'s own score only breaks ties. Each
/// skipped character costs [`GAP_STEP`], so a caller can add a bonus worth
/// a known number of skipped characters without crossing a tier.
pub fn rank_paths<'a>(
    query: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<Ranked<&'a str>> {
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let matched = pattern.match_list(candidates, &mut matcher);
    let Some(needle) = align::Needle::new(query) else {
        return matched
            .into_iter()
            .map(|(item, score)| Ranked {
                item,
                score,
                indices: Vec::new(),
            })
            .collect();
    };
    let mut buf = Vec::new();
    let mut rows: Vec<Scored<'_>> = matched
        .into_iter()
        .enumerate()
        .map(|(index, (path, score))| match align::best(path, &needle) {
            Some(alignment) => Scored {
                index,
                path,
                score: alignment.adjust(score),
                indices: alignment.indices,
            },
            // `nucleo` matched through normalization or out-of-order
            // words the plain alignment can't follow; keep its own view.
            None => Scored {
                index,
                path,
                score: score.min(align::NUCLEO_MAX),
                indices: nucleo_indices(&pattern, &mut matcher, path, &mut buf),
            },
        })
        .collect();
    // Ranking only, never filtering: the shape tier of the score already
    // puts every contiguous or word-initial match above every scattered
    // one, so "ctrl" lists ctrl.rs first and still shows controller.rs.
    rows.sort_by_key(|row| (std::cmp::Reverse(row.score), row.index));
    rows.into_iter()
        .map(|row| Ranked {
            item: row.path,
            score: row.score,
            indices: row.indices,
        })
        .collect()
}

struct Scored<'a> {
    index: usize,
    path: &'a str,
    score: u32,
    indices: Vec<u32>,
}

/// Ranks `items` against `query` by the text `key` extracts from each item,
/// highest score first, dropping non-matches. Like `rank_paths` but for
/// items that aren't bare strings themselves (e.g. commands, where two
/// items can share a display title) and without the path-boundary tuning —
/// plain default scoring for short, non-path labels. `indices` are the
/// positions `nucleo` itself aligned.
pub fn rank_by<'a, T>(
    query: &str,
    items: impl IntoIterator<Item = &'a T>,
    key: impl Fn(&T) -> &str,
) -> Vec<Ranked<&'a T>> {
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = Matcher::default();
    let mut buf = Vec::new();
    let mut scored: Vec<Ranked<&'a T>> = items
        .into_iter()
        .filter_map(|item| {
            let text = key(item);
            let score = pattern.score(Utf32Str::new(text, &mut buf), &mut matcher)?;
            let indices = nucleo_indices(&pattern, &mut matcher, text, &mut buf);
            Some(Ranked {
                item,
                score,
                indices,
            })
        })
        .collect();
    scored.sort_by_key(|ranked| std::cmp::Reverse(ranked.score));
    scored
}

/// `nucleo`'s matched positions as character indices. It counts grapheme
/// clusters, so when `text` has combining characters its indices are not
/// character indices; rather than light the wrong letters, return none.
fn nucleo_indices(
    pattern: &Pattern,
    matcher: &mut Matcher,
    text: &str,
    buf: &mut Vec<char>,
) -> Vec<u32> {
    let haystack = Utf32Str::new(text, buf);
    if haystack.len() != text.chars().count() {
        return Vec::new();
    }
    let mut indices = Vec::new();
    if pattern.indices(haystack, matcher, &mut indices).is_none() {
        return Vec::new();
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Byte ranges in `text` of the characters at `indices` (character indices,
/// ascending, as [`Ranked::indices`] holds them). Out-of-range indices are
/// ignored.
pub fn char_spans(text: &str, indices: &[u32]) -> Vec<Range<usize>> {
    let mut spans = Vec::with_capacity(indices.len());
    let mut wanted = indices.iter().copied().peekable();
    for (index, (byte, ch)) in text.char_indices().enumerate() {
        let Some(&next) = wanted.peek() else { break };
        if next as usize == index {
            spans.push(byte..byte + ch.len_utf8());
            wanted.next();
        }
    }
    spans
}

/// [`char_spans`] limited to the byte range `within` of `text` and shifted
/// so `within.start` is 0. A list that ranks a longer key than the title
/// it shows (`name command args`) lights only the title's part of the
/// match.
pub fn char_spans_in(text: &str, indices: &[u32], within: Range<usize>) -> Vec<Range<usize>> {
    char_spans(text, indices)
        .into_iter()
        .filter(|span| within.start <= span.start && span.end <= within.end)
        .map(|span| span.start - within.start..span.end - within.start)
        .collect()
}

/// Byte range of the first case-insensitive occurrence of `needle` in
/// `text`, for lists that filter by substring rather than fuzzily. Compares
/// character by character so the range is valid even when lowercasing
/// changes a character's byte length.
pub fn substring_span(text: &str, needle: &str) -> Option<Range<usize>> {
    let needle: Vec<char> = needle.chars().map(fold).collect();
    if needle.is_empty() {
        return None;
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    chars.windows(needle.len()).find_map(|window| {
        window
            .iter()
            .zip(&needle)
            .all(|((_, ch), want)| fold(*ch) == *want)
            .then(|| {
                let (first, _) = window[0];
                let (last, ch) = window[window.len() - 1];
                first..last + ch.len_utf8()
            })
    })
}

fn fold(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

#[cfg(test)]
mod tests;
