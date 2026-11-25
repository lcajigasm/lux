//! Preserve Case for replacement: the replacement text takes on the case
//! pattern of the text it replaces, as Sublime Text's "Preserve Case" find
//! option does. `foo` → `bar` turns `Foo` into `Bar` and `FOO` into `BAR`.
//!
//! The rules, first match wins:
//! 1. A match with no cased letters leaves the replacement as typed.
//! 2. An all-lowercase match lowercases the replacement.
//! 3. An all-uppercase match (two or more cased letters) uppercases it.
//! 4. A match made of several words (split at `_`, `-`, spaces, other
//!    punctuation, and camelCase humps), replaced by text with the same
//!    number of words, gets each match word's case applied to the
//!    corresponding replacement word: `Foo-Bar` replaced by `baz-qux` gives
//!    `Baz-Qux`, `fooBar` replaced by `baz_qux` gives `baz_Qux`. The
//!    replacement keeps its own separators.
//! 5. A match whose first cased letter is uppercase capitalizes the
//!    replacement's first letter.
//! 6. Otherwise the replacement is left as typed.

use std::ops::Range;

/// `replacement` recased to follow the case pattern of `matched`.
pub fn apply(matched: &str, replacement: &str) -> String {
    match Shape::of(matched) {
        Shape::Uncased => replacement.to_string(),
        Shape::Lower => replacement.to_lowercase(),
        Shape::Upper => replacement.to_uppercase(),
        Shape::Capitalized | Shape::Mixed => {
            let source = words(matched);
            let target = words(replacement);
            if source.len() > 1 && source.len() == target.len() {
                per_word(matched, &source, replacement, &target)
            } else if starts_upper(matched) {
                capitalize(replacement)
            } else {
                replacement.to_string()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Uncased,
    Lower,
    Upper,
    Capitalized,
    Mixed,
}

impl Shape {
    fn of(text: &str) -> Shape {
        let mut cased = text
            .chars()
            .filter(|c| c.is_lowercase() || c.is_uppercase());
        let Some(first) = cased.next() else {
            return Shape::Uncased;
        };
        let rest: Vec<char> = cased.collect();
        let rest_lower = rest.iter().all(|c| c.is_lowercase());
        let rest_upper = rest.iter().all(|c| c.is_uppercase());
        match (first.is_uppercase(), rest_lower, rest_upper) {
            (false, true, _) => Shape::Lower,
            (true, _, true) if !rest.is_empty() => Shape::Upper,
            (true, true, _) => Shape::Capitalized,
            _ => Shape::Mixed,
        }
    }
}

fn starts_upper(text: &str) -> bool {
    text.chars()
        .find(|c| c.is_lowercase() || c.is_uppercase())
        .is_some_and(char::is_uppercase)
}

fn capitalize(text: &str) -> String {
    let Some(at) = text.find(|c: char| c.is_lowercase() || c.is_uppercase()) else {
        return text.to_string();
    };
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..at]);
    let mut rest = text[at..].chars();
    if let Some(first) = rest.next() {
        out.extend(first.to_uppercase());
    }
    out.push_str(rest.as_str());
    out
}

fn recase(word: &str, shape: Shape) -> String {
    match shape {
        Shape::Lower => word.to_lowercase(),
        Shape::Upper => word.to_uppercase(),
        Shape::Capitalized => {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.as_str().to_lowercase().chars())
                    .collect(),
                None => String::new(),
            }
        }
        Shape::Uncased | Shape::Mixed => word.to_string(),
    }
}

fn per_word(
    matched: &str,
    source: &[Range<usize>],
    replacement: &str,
    target: &[Range<usize>],
) -> String {
    let mut out = String::with_capacity(replacement.len());
    let mut copied = 0;
    for (from, to) in source.iter().zip(target) {
        out.push_str(&replacement[copied..to.start]);
        let shape = match Shape::of(&matched[from.clone()]) {
            // A one-letter uppercase word (`getX`) reads as a capital.
            Shape::Upper if matched[from.clone()].chars().count() == 1 => Shape::Capitalized,
            shape => shape,
        };
        out.push_str(&recase(&replacement[to.clone()], shape));
        copied = to.end;
    }
    out.push_str(&replacement[copied..]);
    out
}

/// Byte ranges of the words in `text`: runs of letters and digits, split
/// again at camelCase humps (`fooBar`) and acronym ends (`HTTPServer`).
fn words(text: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &(at, c)) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if let Some(s) = start.take() {
                out.push(s..at);
            }
            continue;
        }
        let Some(s) = start else {
            start = Some(at);
            continue;
        };
        let prev = chars[i - 1].1;
        let next = chars.get(i + 1).map(|&(_, n)| n);
        let hump = c.is_uppercase() && (prev.is_lowercase() || prev.is_numeric());
        let acronym_end =
            c.is_uppercase() && prev.is_uppercase() && next.is_some_and(char::is_lowercase);
        if hump || acronym_end {
            out.push(s..at);
            start = Some(at);
        }
    }
    if let Some(s) = start {
        out.push(s..text.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_word_shapes() {
        assert_eq!(apply("foo", "bar"), "bar");
        assert_eq!(apply("Foo", "bar"), "Bar");
        assert_eq!(apply("FOO", "bar"), "BAR");
        assert_eq!(apply("foo", "BAR"), "bar");
        assert_eq!(apply("F", "bar"), "Bar");
    }

    #[test]
    fn uncased_match_keeps_replacement() {
        assert_eq!(apply("123", "Bar"), "Bar");
        assert_eq!(apply("--", "bAr"), "bAr");
    }

    #[test]
    fn snake_kebab_and_screaming_case() {
        assert_eq!(apply("foo_bar", "baz_qux"), "baz_qux");
        assert_eq!(apply("FOO_BAR", "baz_qux"), "BAZ_QUX");
        assert_eq!(apply("Foo_Bar", "baz_qux"), "Baz_Qux");
        assert_eq!(apply("Foo-Bar", "baz-qux"), "Baz-Qux");
        assert_eq!(apply("foo-BAR", "baz-qux"), "baz-QUX");
    }

    #[test]
    fn camel_and_pascal_case() {
        assert_eq!(apply("fooBar", "bazQux"), "bazQux");
        assert_eq!(apply("fooBar", "baz_qux"), "baz_Qux");
        assert_eq!(apply("FooBar", "bazqux"), "Bazqux");
        assert_eq!(apply("FooBar", "bazQux"), "BazQux");
        assert_eq!(apply("HTTPServer", "webHost"), "WEBHost");
        assert_eq!(apply("getX", "setY"), "setY");
    }

    #[test]
    fn mismatched_word_counts_follow_the_first_letter() {
        assert_eq!(apply("FooBar", "baz"), "Baz");
        assert_eq!(apply("fooBar", "baz"), "baz");
        assert_eq!(apply("Foo", "baz qux"), "Baz qux");
    }

    #[test]
    fn non_ascii_letters() {
        assert_eq!(apply("ÉTÉ", "año"), "AÑO");
        assert_eq!(apply("Été", "año"), "Año");
        assert_eq!(apply("STRASSE", "straße"), "STRASSE");
    }

    #[test]
    fn word_splitting() {
        let text = "fooBar_baz-QUX HTTPServer x1Y";
        let got: Vec<&str> = words(text).into_iter().map(|r| &text[r]).collect();
        assert_eq!(
            got,
            ["foo", "Bar", "baz", "QUX", "HTTP", "Server", "x1", "Y"]
        );
    }
}
