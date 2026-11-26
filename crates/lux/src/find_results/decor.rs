//! Paint-time styling of the `Find Results` tab. The tab is an ordinary
//! buffer (F4, Enter and double-click read its text), so nothing here
//! edits it: each visible line is classified on the fly and the painter
//! colors glyph ranges and draws match boxes behind them.

use std::ops::Range;

use regex::Regex;

use super::FindResultsNav;

/// What a run of bytes on a result line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    /// `Searching files for …` and the summary line.
    Meta,
    /// A file block's `path:` line.
    Header,
    /// The `  1234:` gutter of a match line.
    Number,
    /// The matched line's own text.
    Text,
}

/// Styling of one buffer line: colored runs (byte ranges within the
/// line, in order) and the byte ranges of the query hits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LineDecor {
    pub runs: Vec<(Range<usize>, Role)>,
    pub hits: Vec<Range<usize>>,
}

impl LineDecor {
    /// Role of the byte at `offset` in the line.
    pub(crate) fn role_at(&self, offset: usize) -> Role {
        self.runs
            .iter()
            .find(|(range, _)| range.contains(&offset))
            .map_or(Role::Text, |(_, role)| *role)
    }
}

/// Splits `line` (without its line break) into roles, and finds the
/// matches of `query` in a match line's text. The layout read here is
/// the one `find_stream` writes.
pub(crate) fn decorate(line: &str, query: Option<&Regex>) -> LineDecor {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return LineDecor::default();
    }
    if trimmed.starts_with("Searching ") || super::is_summary(trimmed) {
        return LineDecor {
            runs: vec![(0..line.len(), Role::Meta)],
            hits: Vec::new(),
        };
    }
    if let Some(colon) = number_gutter(line) {
        let text_start =
            (colon + 1 + usize::from(line[colon + 1..].starts_with(' '))).min(line.len());
        let hits = query.map_or_else(Vec::new, |regex| {
            regex
                .find_iter(&line[text_start..])
                .filter(|hit| !hit.is_empty())
                .map(|hit| text_start + hit.start()..text_start + hit.end())
                .collect()
        });
        return LineDecor {
            runs: vec![
                (0..colon + 1, Role::Number),
                (colon + 1..line.len(), Role::Text),
            ],
            hits,
        };
    }
    if trimmed.ends_with(':') {
        return LineDecor {
            runs: vec![(0..line.len(), Role::Header)],
            hits: Vec::new(),
        };
    }
    LineDecor {
        runs: vec![(0..line.len(), Role::Text)],
        hits: Vec::new(),
    }
}

/// Byte index of the colon after an indented line number (`  12: …`).
fn number_gutter(line: &str) -> Option<usize> {
    if !line.starts_with(' ') {
        return None;
    }
    let colon = line.find(':')?;
    let digits = line[..colon].trim_start();
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(colon)
}

/// The query a results buffer was made for, read from its first line.
pub(crate) fn header_query(first_line: &str) -> Option<&str> {
    let rest = first_line.strip_prefix("Searching files for \"")?;
    let end = rest.rfind('"')?;
    Some(&rest[..end])
}

/// Matcher for the hits of the results in `first_line`'s search. A run
/// started in this session carries its own (with its case setting); a
/// restored tab is matched case-sensitively from its header, compiled
/// once and kept.
pub(crate) fn matcher(nav: &mut FindResultsNav, first_line: &str) -> Option<Regex> {
    let query = header_query(first_line)?;
    if nav.highlight.as_ref().map(|(key, _)| key.as_str()) != Some(query) {
        nav.highlight = Some((query.to_string(), Regex::new(query).ok()));
    }
    nav.highlight.as_ref().and_then(|(_, regex)| regex.clone())
}

/// Matcher of a run that is starting now.
pub(crate) fn run_matcher(query: &str, case_insensitive: bool) -> (String, Option<Regex>) {
    let regex = regex::RegexBuilder::new(query)
        .case_insensitive(case_insensitive)
        .build()
        .ok();
    (query.to_string(), regex)
}

#[cfg(test)]
#[path = "decor_tests.rs"]
mod tests;
