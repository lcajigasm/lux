//! Word strokes for one minimap row: each run of non-blank characters that
//! shares one syntax color becomes one stroke, so the minimap reads as
//! miniature text instead of a single bar per line.
//!
//! Colors come from the highlight tokens the editor already holds (the
//! caller implements [`LineColors`] over them). Nothing here parses: a line
//! without a color source, or a gap between tokens, takes the plain
//! foreground (`color: None`).

use lux_core::buffer::Buffer;

/// Columns scanned per line. The minimap strip is about 70 columns wide at
/// its default width, so a minified line past this is clipped anyway; the cap
/// keeps one pathological line from costing more than any other.
pub const MAX_INK_COLUMNS: usize = 200;

/// One stroke: `len` non-blank columns starting at column `start`, in one
/// color. Columns are characters, tabs counting one, as the density bars do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimapSpan {
    pub start: u16,
    pub len: u16,
    /// The token color, or `None` for the plain foreground.
    pub color: Option<[u8; 3]>,
}

/// One colored byte range of the document, absolute byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRun {
    pub start: usize,
    pub end: usize,
    pub rgb: [u8; 3],
}

/// The editor's syntax colors, as the minimap reads them.
pub trait LineColors {
    /// Changes whenever any color may have changed: a new parse result, a
    /// different theme, a different document. Strokes cached under one stamp
    /// are recomputed under another.
    fn stamp(&self) -> u64;
    /// Appends the colored runs overlapping `start..end`, in byte order.
    fn runs(&self, start: usize, end: usize, out: &mut Vec<ColorRun>);
}

/// The strokes of `line`, appended to `out`.
pub(crate) fn line_spans(
    buffer: &Buffer,
    line: usize,
    colors: Option<&dyn LineColors>,
    runs: &mut Vec<ColorRun>,
    out: &mut Vec<MinimapSpan>,
) {
    if line >= buffer.len_lines() {
        return;
    }
    let rope = buffer.rope();
    let slice = rope.line(line);
    let line_start = rope.line_to_byte(line);
    runs.clear();
    if let Some(colors) = colors {
        let scan_end = line_start + slice.len_bytes().min(MAX_INK_COLUMNS * 4);
        colors.runs(line_start, scan_end, runs);
    }
    let mut next_run = 0usize;
    let mut open: Option<MinimapSpan> = None;
    let mut byte = line_start;
    for (column, ch) in slice.chars().enumerate() {
        if column >= MAX_INK_COLUMNS || ch == '\n' || ch == '\r' {
            break;
        }
        let at = byte;
        byte += ch.len_utf8();
        if ch.is_whitespace() {
            out.extend(open.take());
            continue;
        }
        while next_run < runs.len() && runs[next_run].end <= at {
            next_run += 1;
        }
        let color = runs
            .get(next_run)
            .filter(|run| run.start <= at)
            .map(|run| run.rgb);
        match open.as_mut() {
            Some(span) if span.color == color => span.len += 1,
            _ => {
                out.extend(open.take());
                open = Some(MinimapSpan {
                    start: column as u16,
                    len: 1,
                    color,
                });
            }
        }
    }
    out.extend(open);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Vec<ColorRun>);

    impl LineColors for Fixed {
        fn stamp(&self) -> u64 {
            1
        }
        fn runs(&self, start: usize, end: usize, out: &mut Vec<ColorRun>) {
            out.extend(
                self.0
                    .iter()
                    .filter(|run| run.end > start && run.start < end),
            );
        }
    }

    fn spans(text: &str, line: usize, colors: Option<&dyn LineColors>) -> Vec<MinimapSpan> {
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let mut out = Vec::new();
        line_spans(&buffer, line, colors, &mut Vec::new(), &mut out);
        out
    }

    #[test]
    fn words_become_separate_strokes() {
        let got = spans("  let x = 1;\n", 0, None);
        let shape: Vec<(u16, u16)> = got.iter().map(|s| (s.start, s.len)).collect();
        assert_eq!(shape, vec![(2, 3), (6, 1), (8, 1), (10, 2)]);
        assert!(got.iter().all(|s| s.color.is_none()));
    }

    #[test]
    fn a_color_change_inside_a_word_splits_the_stroke() {
        let red = [200, 0, 0];
        let colors = Fixed(vec![ColorRun {
            start: 4,
            end: 7,
            rgb: red,
        }]);
        // "b\nfoo(bar)\n": line 1 starts at byte 2; bytes 4..7 are "o(b".
        let got = spans("b\nfoo(bar)\n", 1, Some(&colors));
        let shape: Vec<(u16, u16, Option<[u8; 3]>)> =
            got.iter().map(|s| (s.start, s.len, s.color)).collect();
        assert_eq!(shape, vec![(0, 2, None), (2, 3, Some(red)), (5, 3, None)]);
    }

    #[test]
    fn a_long_line_stops_at_the_column_cap() {
        let text = "x".repeat(MAX_INK_COLUMNS * 3);
        let got = spans(&text, 0, None);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].len as usize, MAX_INK_COLUMNS);
    }
}
