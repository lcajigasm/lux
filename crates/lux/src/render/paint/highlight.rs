//! Which glyphs of a list row to draw as a match. The positions come from
//! the matcher that ranked the row (`lux_search::fuzzy`), carried on the
//! row itself, so the list lights exactly the characters that were scored —
//! never a re-scan of the label that could pick different letters.

use std::ops::Range;

/// Whether any byte range in `spans` overlaps the glyph cluster `range`.
pub(crate) fn span_covers(spans: &[Range<usize>], range: &Range<usize>) -> bool {
    spans
        .iter()
        .any(|span| span.start < range.end && range.start < span.end)
}

#[cfg(test)]
mod tests {
    use super::span_covers;

    #[test]
    fn a_cluster_is_lit_only_when_a_span_overlaps_it() {
        let spans = [0..1, 5..6];
        assert!(span_covers(&spans, &(0..1)));
        assert!(span_covers(&spans, &(4..6)));
        assert!(!span_covers(&spans, &(1..2)));
        assert!(!span_covers(&[], &(0..1)));
    }
}
