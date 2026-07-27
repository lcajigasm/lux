//! Parses a minimal TextMate-style snippet body: `$1`, `${1}`, `${1:default
//! text}`, `$0` for the final tab stop, and `\$`/`\\`/`\}` escapes. Not the
//! full TextMate grammar (no nested placeholders, no transforms/variables)
//! — just enough for real, navigable tab stops with defaults and mirrors,
//! which is what v0 never had (it stripped snippet syntax with a string
//! `strip()` and left plain text, no placeholders at all).

use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabstopSpec {
    pub index: u32,
    /// Char ranges into the parsed `text`, one per occurrence — mirrors of
    /// the same index. The first is the primary occurrence.
    pub ranges: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSnippet {
    pub text: String,
    /// In navigation order: ascending by index, with `0` (the final
    /// position) always last, since that's what tab stop `0` means
    /// regardless of where it appears in the template.
    pub tabstops: Vec<TabstopSpec>,
}

enum Segment {
    Text(String),
    /// `own_default` is `Some` only when this occurrence used `${N:...}`
    /// (even if empty, `${N:}`) — a bare `$N` or `${N}` is `None`, meaning
    /// "mirror whatever default this index's group ends up with".
    Tabstop {
        index: u32,
        own_default: Option<String>,
    },
}

/// Two passes: first split the template into text/tabstop segments without
/// committing final positions, then resolve each index's "canonical"
/// default (the first `${N:...}` for that index anywhere in the template)
/// so that bare mirrors of an index with a default actually show it —
/// `"${1:x} $1"` renders as `"x x"`, not `"x "`.
pub fn parse(template: &str) -> ParsedSnippet {
    let segments = split_into_segments(template);

    let mut canonical: BTreeMap<u32, String> = BTreeMap::new();
    for seg in &segments {
        if let Segment::Tabstop {
            index,
            own_default: Some(default),
        } = seg
        {
            canonical.entry(*index).or_insert_with(|| default.clone());
        }
    }

    let mut text = String::new();
    let mut occurrences: Vec<(u32, usize, usize)> = Vec::new();
    for seg in segments {
        match seg {
            Segment::Text(t) => text.push_str(&t),
            Segment::Tabstop { index, own_default } => {
                let content = own_default
                    .unwrap_or_else(|| canonical.get(&index).cloned().unwrap_or_default());
                let start = text.chars().count();
                text.push_str(&content);
                let end = text.chars().count();
                occurrences.push((index, start, end));
            }
        }
    }

    let text_len = text.chars().count();
    ParsedSnippet {
        text,
        tabstops: group_tabstops(text_len, occurrences),
    }
}

fn split_into_segments(template: &str) -> Vec<Segment> {
    let chars: Vec<char> = template.chars().collect();
    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            current_text.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '$' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            flush_text(&mut segments, &mut current_text);
            let (index, next_i) = read_digits(&chars, i + 1);
            segments.push(Segment::Tabstop {
                index,
                own_default: None,
            });
            i = next_i;
            continue;
        }
        if c == '$'
            && i + 1 < chars.len()
            && chars[i + 1] == '{'
            && let Some((index, own_default, next_i)) = read_braced(&chars, i + 2)
        {
            flush_text(&mut segments, &mut current_text);
            segments.push(Segment::Tabstop { index, own_default });
            i = next_i;
            continue;
        }
        current_text.push(c);
        i += 1;
    }
    flush_text(&mut segments, &mut current_text);
    segments
}

fn flush_text(segments: &mut Vec<Segment>, current_text: &mut String) {
    if !current_text.is_empty() {
        segments.push(Segment::Text(std::mem::take(current_text)));
    }
}

fn read_digits(chars: &[char], start: usize) -> (u32, usize) {
    let mut j = start;
    let mut num = String::new();
    while j < chars.len() && chars[j].is_ascii_digit() {
        num.push(chars[j]);
        j += 1;
    }
    (num.parse().expect("at least one digit was matched"), j)
}

/// Parses `N}` or `N:default}` (the part after `${`), honoring `\}`
/// escapes inside the default text. Returns `None` if it isn't a valid
/// tabstop (no digits right after `{`), so the caller can fall back to
/// treating `$` as a literal character.
fn read_braced(chars: &[char], start: usize) -> Option<(u32, Option<String>, usize)> {
    if start >= chars.len() || !chars[start].is_ascii_digit() {
        return None;
    }
    let (index, mut j) = read_digits(chars, start);
    let has_colon = j < chars.len() && chars[j] == ':';
    if has_colon {
        j += 1;
    }
    let mut default = String::new();
    while j < chars.len() && chars[j] != '}' {
        if chars[j] == '\\' && j + 1 < chars.len() {
            default.push(chars[j + 1]);
            j += 2;
        } else {
            default.push(chars[j]);
            j += 1;
        }
    }
    let next_i = if j < chars.len() { j + 1 } else { j };
    Some((index, has_colon.then_some(default), next_i))
}

fn group_tabstops(text_len: usize, occurrences: Vec<(u32, usize, usize)>) -> Vec<TabstopSpec> {
    let mut groups: BTreeMap<u32, Vec<Range<usize>>> = BTreeMap::new();
    for (index, start, end) in occurrences {
        groups.entry(index).or_default().push(start..end);
    }
    // `0` is the final cursor position, navigated to last regardless of its
    // numeric value, and always exists — implicitly at the snippet's end if
    // the template didn't place one. This is a `Vec<Range<usize>>` (mirror
    // ranges), not the numeric range clippy's `single_range_in_vec_init`
    // suggestion assumes.
    #[allow(clippy::single_range_in_vec_init)]
    groups.entry(0).or_insert_with(|| vec![text_len..text_len]);

    let mut indices: Vec<u32> = groups.keys().copied().collect();
    indices.sort_by_key(|&i| if i == 0 { u32::MAX } else { i });
    indices
        .into_iter()
        .map(|index| TabstopSpec {
            index,
            ranges: groups
                .remove(&index)
                .expect("index came from this map's keys"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_has_no_tabstops_but_gets_an_implicit_final_one() {
        let parsed = parse("hello world");
        assert_eq!(parsed.text, "hello world");
        assert_eq!(parsed.tabstops.len(), 1);
        assert_eq!(parsed.tabstops[0].index, 0);
        assert_eq!(parsed.tabstops[0].ranges, vec![11..11]);
    }

    #[test]
    fn simple_numbered_tabstops_in_order() {
        let parsed = parse("$1 and $2");
        assert_eq!(parsed.text, " and ");
        assert_eq!(parsed.tabstops.len(), 3); // 1, 2, and the implicit 0
        assert_eq!(parsed.tabstops[0].index, 1);
        assert_eq!(parsed.tabstops[0].ranges, vec![0..0]);
        assert_eq!(parsed.tabstops[1].index, 2);
        assert_eq!(parsed.tabstops[1].ranges, vec![5..5]);
        assert_eq!(parsed.tabstops[2].index, 0);
    }

    #[test]
    fn braced_tabstop_with_default_value() {
        let parsed = parse("let ${1:name} = ${2:value};");
        assert_eq!(parsed.text, "let name = value;");
        assert_eq!(parsed.tabstops[0].index, 1);
        assert_eq!(parsed.tabstops[0].ranges, vec![4..8]);
        assert_eq!(parsed.tabstops[1].index, 2);
        assert_eq!(parsed.tabstops[1].ranges, vec![11..16]);
    }

    #[test]
    fn braced_tabstop_without_default_is_empty() {
        let parsed = parse("(${1})");
        assert_eq!(parsed.text, "()");
        assert_eq!(parsed.tabstops[0].ranges, vec![1..1]);
    }

    #[test]
    fn mirrored_tabstops_share_one_group_with_multiple_ranges() {
        let parsed = parse("${1:x} + $1 = 2 * $1");
        assert_eq!(parsed.text, "x + x = 2 * x");
        let group = parsed.tabstops.iter().find(|t| t.index == 1).unwrap();
        assert_eq!(group.ranges, vec![0..1, 4..5, 12..13]);
    }

    #[test]
    fn bare_mirror_before_the_default_still_picks_it_up() {
        // The default appears on the *second* occurrence — the canonical
        // default is "whichever occurrence specifies one, anywhere in the
        // template", not just "the first occurrence textually".
        let parsed = parse("$1 and ${1:x}");
        assert_eq!(parsed.text, "x and x");
    }

    #[test]
    fn explicit_final_tabstop_navigates_last_regardless_of_position() {
        let parsed = parse("$0 after $1 before");
        let last = parsed.tabstops.last().unwrap();
        assert_eq!(last.index, 0);
        assert_eq!(last.ranges, vec![0..0]);
    }

    #[test]
    fn escaped_dollar_and_brace_are_literal() {
        let parsed = parse(r"cost: \$5, set: \{${1:a}\}");
        assert_eq!(parsed.text, "cost: $5, set: {a}");
    }

    #[test]
    fn default_value_can_contain_an_escaped_closing_brace() {
        let parsed = parse(r"${1:a\}b}");
        assert_eq!(parsed.text, "a}b");
        assert_eq!(parsed.tabstops[0].ranges, vec![0..3]);
    }

    #[test]
    fn unicode_default_text_uses_char_offsets_not_byte_offsets() {
        let parsed = parse("${1:café} ok");
        assert_eq!(parsed.text, "café ok");
        // "café" is 4 chars but 5 bytes — ranges must be in chars.
        assert_eq!(parsed.tabstops[0].ranges, vec![0..4]);
    }
}
