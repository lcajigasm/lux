//! Tab titles that do not fit their tab. As in Sublime Text, the middle of
//! the name goes and the extension stays: `very_long_module_name.rs`
//! becomes `very_lo…name.rs`. Cuts fall between grapheme clusters, never
//! inside a UTF-8 sequence or a combined character.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

const ELLIPSIS: &str = "…";

/// `title` as it fits in `max_width`, measured by `measure`. Returns the
/// title itself when it fits.
pub(crate) fn elide<'a>(
    title: &'a str,
    max_width: f32,
    measure: &mut dyn FnMut(&str) -> f32,
) -> Cow<'a, str> {
    if measure(title) <= max_width {
        return Cow::Borrowed(title);
    }
    let (stem, ext) = split_extension(title);
    let graphemes: Vec<&str> = stem.graphemes(true).collect();
    // Keep the extension and as much of both ends of the stem as fits,
    // one more grapheme at the front when the count is odd.
    let fits = |keep: usize, measure: &mut dyn FnMut(&str) -> f32| {
        let text = middle_cut(&graphemes, keep, ext);
        (measure(&text) <= max_width).then_some(text)
    };
    if let Some(best) = largest_fit(graphemes.len().saturating_sub(1), |keep| {
        fits(keep, measure)
    }) {
        return Cow::Owned(best);
    }
    // Not even `…` and the extension fit: cut the whole title at the end.
    let all: Vec<&str> = title.graphemes(true).collect();
    let tail_cut = |keep: usize, measure: &mut dyn FnMut(&str) -> f32| {
        let text = format!("{}{ELLIPSIS}", all[..keep].concat());
        (measure(&text) <= max_width).then_some(text)
    };
    let best = largest_fit(all.len().saturating_sub(1), |keep| tail_cut(keep, measure));
    Cow::Owned(best.unwrap_or_default())
}

/// The extension, dot included, when the name has a stem before it.
/// `.gitignore` and `Makefile` have none.
fn split_extension(title: &str) -> (&str, &str) {
    match title.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < title.len() => title.split_at(dot),
        _ => (title, ""),
    }
}

fn middle_cut(graphemes: &[&str], keep: usize, ext: &str) -> String {
    let front = keep.div_ceil(2);
    let back = keep - front;
    let mut text = graphemes[..front].concat();
    text.push_str(ELLIPSIS);
    text.push_str(&graphemes[graphemes.len() - back..].concat());
    text.push_str(ext);
    text
}

/// The result for the largest `keep` in `0..=max` that `try_keep` accepts.
/// Wider text never measures narrower, so a binary search finds it.
fn largest_fit(max: usize, mut try_keep: impl FnMut(usize) -> Option<String>) -> Option<String> {
    let mut best = try_keep(0)?;
    let (mut low, mut high) = (1, max);
    while low <= high {
        let mid = low + (high - low) / 2;
        match try_keep(mid) {
            Some(text) => {
                best = text;
                low = mid + 1;
            }
            None => high = mid - 1,
        }
    }
    Some(best)
}

#[cfg(test)]
mod tests {
    use super::elide;

    /// One unit per character, like a monospace face.
    fn chars(max: usize, title: &str) -> String {
        elide(title, max as f32, &mut |s: &str| s.chars().count() as f32).into_owned()
    }

    #[test]
    fn a_title_that_fits_is_unchanged() {
        assert_eq!(chars(20, "main.rs"), "main.rs");
    }

    #[test]
    fn the_middle_of_the_stem_goes_and_the_extension_stays() {
        let cut = chars(16, "very_long_module_name.rs");
        assert_eq!(cut, "very_l…e_name.rs");
        assert_eq!(cut.chars().count(), 16);
        assert!(cut.ends_with(".rs"));
    }

    #[test]
    fn names_without_an_extension_are_cut_in_the_middle() {
        assert_eq!(chars(7, "Makefile.long"), "M….long");
        assert_eq!(chars(7, ".gitignore_extra"), ".gi…tra");
        assert_eq!(chars(5, "README"), "RE…ME");
    }

    #[test]
    fn a_tab_too_narrow_for_the_extension_cuts_at_the_end() {
        assert_eq!(chars(4, "component.tsx"), "com…");
        assert_eq!(chars(1, "component.tsx"), "…");
        assert_eq!(chars(0, "component.tsx"), "");
    }

    #[test]
    fn cuts_never_split_a_character_or_a_grapheme() {
        // `é` as e + combining acute, an emoji with a skin tone, and CJK.
        let title = "cafe\u{301}_\u{1F44D}\u{1F3FD}_\u{6F22}\u{5B57}\u{6F22}\u{5B57}.md";
        for max in 0..20 {
            let cut = chars(max, title);
            assert!(cut.chars().count() <= max, "{max}: {cut}");
            // Every grapheme of the result is a whole grapheme of the title
            // or the ellipsis.
            use unicode_segmentation::UnicodeSegmentation;
            let whole: Vec<&str> = title.graphemes(true).collect();
            for g in cut.graphemes(true) {
                assert!(g == "…" || whole.contains(&g), "{max}: {g:?} in {cut:?}");
            }
        }
    }

    #[test]
    fn proportional_widths_are_respected() {
        // `W` is wide, everything else narrow.
        let mut measure = |s: &str| s.chars().map(|c| if c == 'W' { 3.0 } else { 1.0 }).sum();
        let cut = elide("WWWWWWWWaaaa.rs", 10.0, &mut measure).into_owned();
        assert!(measure(&cut) <= 10.0, "{cut}");
        assert!(cut.ends_with(".rs") && cut.contains('…'), "{cut}");
    }
}
