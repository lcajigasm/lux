//! Syntax-colored strokes through the cached model, and the work counters
//! that show a typed character or a scroll does not rebuild the minimap.

use super::*;

const KEYWORD: [u8; 3] = [249, 38, 114];

/// Colors every occurrence of `fn` like a highlight token would.
struct Keywords {
    stamp: u64,
    runs: Vec<ColorRun>,
}

impl Keywords {
    fn over(buffer: &Buffer, stamp: u64) -> Self {
        let text = buffer.text();
        let runs = text
            .match_indices("fn")
            .map(|(at, _)| ColorRun {
                start: at,
                end: at + 2,
                rgb: KEYWORD,
            })
            .collect();
        Keywords { stamp, runs }
    }
}

impl LineColors for Keywords {
    fn stamp(&self) -> u64 {
        self.stamp
    }
    fn runs(&self, start: usize, end: usize, out: &mut Vec<ColorRun>) {
        let first = self.runs.partition_point(|run| run.end <= start);
        out.extend(self.runs[first..].iter().take_while(|run| run.start < end));
    }
}

fn sample<'a>(
    buffer: &'a Buffer,
    edit: Option<LineEdit>,
    visible: Range<usize>,
    available_rows: usize,
    colors: &'a dyn LineColors,
) -> MinimapSample<'a> {
    MinimapSample {
        revision: buffer.revision(),
        document: 3,
        edit,
        buffer,
        visible,
        available_rows,
        folds: None,
        colors: Some(colors),
    }
}

fn shape(frame: &MinimapFrame, row: usize) -> Vec<(u16, u16, Option<[u8; 3]>)> {
    frame
        .row_spans(row)
        .iter()
        .map(|s| (s.start, s.len, s.color))
        .collect()
}

#[test]
fn each_row_carries_its_words_in_token_colors() {
    let buffer = Buffer::from_bytes(b"fn main() {\n    x\n}\n").unwrap();
    let colors = Keywords::over(&buffer, 1);
    let mut model = MinimapModel::default();
    let frame = model
        .frame(&Minimap::new(), sample(&buffer, None, 0..3, 100, &colors))
        .unwrap();
    assert_eq!(
        shape(&frame, 0),
        vec![(0, 2, Some(KEYWORD)), (3, 6, None), (10, 1, None)]
    );
    assert_eq!(shape(&frame, 1), vec![(4, 1, None)]);
    assert_eq!(shape(&frame, 2), vec![(0, 1, None)]);
    assert!(frame.row_spans(3).is_empty());
}

#[test]
fn a_shared_row_draws_the_densest_lines_strokes() {
    // Two lines per row: row 0 shows "fn longer", not "a".
    let buffer = Buffer::from_bytes(b"a\nfn longer\nb\n").unwrap();
    let colors = Keywords::over(&buffer, 1);
    let mut model = MinimapModel::default();
    let frame = model
        .frame(&Minimap::new(), sample(&buffer, None, 0..2, 2, &colors))
        .unwrap();
    assert_eq!(frame.rows[0].line, 1);
    assert_eq!(shape(&frame, 0), vec![(0, 2, Some(KEYWORD)), (3, 6, None)]);
}

#[test]
fn a_typed_character_reinks_only_its_row_and_keeps_the_others_colors() {
    let mut buffer = Buffer::from_bytes(b"fn a\nfn b\n").unwrap();
    let colors = Keywords::over(&buffer, 1);
    let mut model = MinimapModel::default();
    let first = model
        .frame(&Minimap::new(), sample(&buffer, None, 0..2, 100, &colors))
        .unwrap();
    let inked = model.stats().lines_inked;
    let before = buffer.revision();
    buffer.insert(4, "x");
    // The editor shifts its tokens on an edit; line 1's `fn` stays a keyword.
    let colors = Keywords::over(&buffer, 1);
    let edit = LineEdit {
        line: 0,
        before,
        after: buffer.revision(),
    };
    let after = model
        .frame(
            &Minimap::new(),
            sample(&buffer, Some(edit), 0..2, 100, &colors),
        )
        .unwrap();
    assert_eq!(model.stats().lines_inked, inked + 1);
    assert_eq!(shape(&after, 0), vec![(0, 2, Some(KEYWORD)), (3, 2, None)]);
    assert_eq!(shape(&after, 1), shape(&first, 1));
    assert_eq!(shape(&after, 1)[0].2, Some(KEYWORD));
}

#[test]
fn a_new_parse_result_recolors_every_row() {
    let buffer = Buffer::from_bytes(b"fn a\nfn b\n").unwrap();
    let plain = Keywords {
        stamp: 1,
        runs: Vec::new(),
    };
    let mut model = MinimapModel::default();
    let frame = model
        .frame(&Minimap::new(), sample(&buffer, None, 0..2, 100, &plain))
        .unwrap();
    assert_eq!(shape(&frame, 1)[0].2, None);
    let parsed = Keywords::over(&buffer, 2);
    let frame = model
        .frame(&Minimap::new(), sample(&buffer, None, 0..2, 100, &parsed))
        .unwrap();
    assert_eq!(shape(&frame, 0)[0].2, Some(KEYWORD));
    assert_eq!(shape(&frame, 1)[0].2, Some(KEYWORD));
}

/// The ST-MAP-01 cost claim on a 50k-line file: the first frame measures
/// every line once but inks only one line per minimap row; a scroll does no
/// work; a typed character measures and inks one line; a new parse result
/// re-inks at most one line per row, never the document.
#[test]
fn a_fifty_thousand_line_file_builds_incrementally() {
    let mut text = String::new();
    for i in 0..50_000 {
        text.push_str(&format!("    fn item_{i}() {{ call({i}); }}\n"));
    }
    let mut buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    let colors = Keywords::over(&buffer, 1);
    let rows = 300;
    let minimap = Minimap::new();
    let mut model = MinimapModel::default();

    let started = std::time::Instant::now();
    model
        .frame(&minimap, sample(&buffer, None, 0..40, rows, &colors))
        .unwrap();
    let full = started.elapsed();
    let first = model.stats();
    assert_eq!(first.lines_measured, buffer.len_lines() as u64);
    assert!(first.lines_inked <= rows as u64, "{first:?}");

    model
        .frame(&minimap, sample(&buffer, None, 900..940, rows, &colors))
        .unwrap();
    assert_eq!(model.stats(), first, "a scroll must not rebuild");

    let before = buffer.revision();
    let at = buffer.rope().line_to_char(25_000) + 4;
    buffer.insert(at, "pub ");
    let edit = LineEdit {
        line: 25_000,
        before,
        after: buffer.revision(),
    };
    let started = std::time::Instant::now();
    let frame = model
        .frame(&minimap, sample(&buffer, Some(edit), 0..40, rows, &colors))
        .unwrap();
    let patched = started.elapsed();
    let typed = model.stats();
    assert_eq!(typed.lines_measured, first.lines_measured + 1);
    assert!(typed.lines_inked <= first.lines_inked + 1, "{typed:?}");
    eprintln!("50k-line minimap: full build {full:?}, one typed character {patched:?}");
    let row = row_for_line(buffer.len_lines(), rows, 25_000);
    assert!(frame.rows[row].content_columns > 0);

    let reparsed = Keywords::over(&buffer, 2);
    model
        .frame(&minimap, sample(&buffer, None, 0..40, rows, &reparsed))
        .unwrap();
    let recolored = model.stats();
    assert_eq!(recolored.lines_measured, typed.lines_measured);
    assert!(recolored.lines_inked - typed.lines_inked <= rows as u64);
}
