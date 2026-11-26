use super::*;

#[test]
fn minimap_defaults_to_enabled() {
    assert!(Minimap::new().enabled);
}

#[test]
fn toggle_flips_enabled() {
    let mut minimap = Minimap::new();
    minimap.toggle();
    assert!(!minimap.enabled);
    minimap.toggle();
    assert!(minimap.enabled);
}

#[test]
fn build_frame_returns_none_when_disabled() {
    let buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
    let minimap = Minimap { enabled: false };
    assert_eq!(build_frame(&minimap, &buffer, 0..2, 100, None), None);
}

#[test]
fn build_frame_is_one_to_one_when_the_document_fits() {
    let buffer = Buffer::from_bytes(b"one\n  two\nthree\n").unwrap();
    let minimap = Minimap::new();
    let frame = build_frame(&minimap, &buffer, 0..4, 100, None).unwrap();
    // "one\n  two\nthree\n" is 4 lines (ropey counts the trailing
    // newline as one more, empty line — same convention as LineIndex).
    assert_eq!(frame.rows.len(), 4);
    assert_eq!(
        frame.rows[0],
        MinimapRow {
            line: 0,
            indent_columns: 0,
            content_columns: 3
        }
    );
    assert_eq!(
        frame.rows[1],
        MinimapRow {
            line: 1,
            indent_columns: 2,
            content_columns: 3
        }
    );
    assert_eq!(
        frame.rows[3],
        MinimapRow {
            line: 3,
            indent_columns: 0,
            content_columns: 0
        }
    );
}

#[test]
fn build_frame_downsamples_and_picks_the_densest_line_per_row() {
    // 6 lines, 3 rows available -> 2 lines per row. Row 0 covers lines
    // 0-1 ("a" and "much longer line"); the longer one should win.
    let buffer = Buffer::from_bytes(b"a\nmuch longer line\nb\nc\nd\ne\n").unwrap();
    let minimap = Minimap::new();
    let frame = build_frame(&minimap, &buffer, 0..1, 3, None).unwrap();
    assert_eq!(frame.rows.len(), 3);
    assert_eq!(frame.rows[0].line, 1);
    assert_eq!(frame.rows[0].content_columns, "much longer line".len());
}

#[test]
fn build_frame_never_pads_with_empty_trailing_rows() {
    // 5 lines into 3 available rows: 2 lines/row needs ceil(5/2) = 3
    // rows exactly (2, 2, 1) — no row is ever empty.
    let buffer = Buffer::from_bytes(b"1\n2\n3\n4\n5").unwrap();
    let minimap = Minimap::new();
    let frame = build_frame(&minimap, &buffer, 0..1, 3, None).unwrap();
    assert_eq!(frame.rows.len(), 3);

    // And when the document is smaller than the rows on offer, only as
    // many rows as there are lines are produced — never padded to 100.
    // ("1\n2\n" is 3 lines, not 2: a trailing newline always yields one
    // more, empty final line — see `LineIndex`'s own doc comment.)
    let small = Buffer::from_bytes(b"1\n2\n").unwrap();
    let frame = build_frame(&minimap, &small, 0..1, 100, None).unwrap();
    assert_eq!(frame.rows.len(), 3);
}

#[test]
fn viewport_indicator_matches_the_one_to_one_visible_range() {
    let buffer = Buffer::from_bytes(b"1\n2\n3\n4\n5").unwrap();
    let minimap = Minimap::new();
    let frame = build_frame(&minimap, &buffer, 1..3, 100, None).unwrap();
    assert_eq!(frame.viewport.rows, 1..3);
}

#[test]
fn viewport_indicator_tracks_the_visible_range_through_downsampling() {
    // 10 lines, 5 rows -> 2 lines/row. Visible lines 4..6 (0-indexed)
    // fall in rows 2 and 3.
    let buffer = Buffer::from_bytes(b"0\n1\n2\n3\n4\n5\n6\n7\n8\n9").unwrap();
    let minimap = Minimap::new();
    let frame = build_frame(&minimap, &buffer, 4..6, 5, None).unwrap();
    assert_eq!(frame.viewport.rows, 2..3);
}

#[test]
fn viewport_indicator_is_empty_for_an_empty_visible_range() {
    let buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
    let minimap = Minimap::new();
    let frame = build_frame(&minimap, &buffer, 0..0, 100, None).unwrap();
    assert_eq!(frame.viewport.rows, 0..0);
}

#[test]
fn line_for_row_maps_a_click_back_to_the_right_source_line() {
    // 10 lines, 5 rows -> row 2 covers source lines 4-5.
    assert_eq!(line_for_row(10, 5, 2), 4);
}

#[test]
fn line_for_row_clamps_an_out_of_range_row_to_the_last_line() {
    assert_eq!(line_for_row(4, 100, 99), 3);
}

#[test]
fn empty_document_is_a_single_empty_row() {
    let buffer = Buffer::from_bytes(b"").unwrap();
    let minimap = Minimap::new();
    let frame = build_frame(&minimap, &buffer, 0..1, 100, None).unwrap();
    assert_eq!(frame.rows.len(), 1);
    assert_eq!(frame.rows[0].content_columns, 0);
}

#[test]
fn build_frame_skips_folded_lines() {
    let buffer = Buffer::from_bytes(b"header\nhidden 1\nhidden 2\ntail\n").unwrap();
    let minimap = Minimap::new();
    let mut folds = lux_core::folding::FoldState::new();
    folds.set_foldable_ranges(vec![lux_core::folding::FoldRange::new(0, 2)]);
    folds.fold(0);

    let frame = build_frame(&minimap, &buffer, 0..2, 100, Some(&folds)).unwrap();
    // 5 total lines originally, lines 1..=2 hidden -> 3 visual lines: header, tail, trailing empty
    assert_eq!(frame.rows.len(), 3);
    assert_eq!(frame.rows[0].line, 0);
    assert_eq!(frame.rows[1].line, 3);
}

fn sample<'a>(
    buffer: &'a Buffer,
    edit: Option<LineEdit>,
    visible: Range<usize>,
    available_rows: usize,
) -> MinimapSample<'a> {
    MinimapSample {
        revision: buffer.revision(),
        document: 7,
        edit,
        buffer,
        visible,
        available_rows,
        folds: None,
    }
}

fn bars(frame: &MinimapFrame) -> Vec<usize> {
    frame.rows.iter().map(|row| row.content_columns).collect()
}

#[test]
fn scrolling_reuses_the_row_bars() {
    let mut text = String::new();
    for i in 0..200 {
        text.push_str(&format!("line {i}\n"));
    }
    let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    let minimap = Minimap::new();
    let mut model = MinimapModel::default();
    let top = model
        .frame(&minimap, sample(&buffer, None, 0..10, 40))
        .unwrap();
    let scrolled = model
        .frame(&minimap, sample(&buffer, None, 80..100, 40))
        .unwrap();
    assert_eq!(top.rows, scrolled.rows);
    assert_ne!(top.viewport.rows, scrolled.viewport.rows);
}

#[test]
fn one_typed_line_patches_that_bar() {
    let mut buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
    let minimap = Minimap::new();
    let mut model = MinimapModel::default();
    let first = model
        .frame(&minimap, sample(&buffer, None, 0..2, 100))
        .unwrap();
    let before = buffer.revision();
    buffer.insert(0, "zzz");
    let edit = LineEdit {
        line: 0,
        before,
        after: buffer.revision(),
    };
    let after = model
        .frame(&minimap, sample(&buffer, Some(edit), 0..2, 100))
        .unwrap();
    assert_eq!(first.rows[0].content_columns, 1);
    assert_eq!(after.rows[0].content_columns, 4);
    assert_eq!(after.rows[1], first.rows[1]);
}

#[test]
fn a_stale_line_edit_rebuilds_every_bar() {
    let mut buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
    let minimap = Minimap::new();
    let mut model = MinimapModel::default();
    model.frame(&minimap, sample(&buffer, None, 0..2, 100));
    let before = buffer.revision();
    buffer.insert(0, "x");
    let edit = LineEdit {
        line: 0,
        before,
        after: buffer.revision(),
    };
    // A later edit on another line that recorded no line edit.
    buffer.insert(3, "yy");
    let frame = model
        .frame(&minimap, sample(&buffer, Some(edit), 0..2, 100))
        .unwrap();
    assert_eq!(bars(&frame), vec![2, 3, 0]);
}

#[test]
fn a_line_edit_from_an_older_cache_rebuilds_every_bar() {
    let mut buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
    let minimap = Minimap::new();
    let mut model = MinimapModel::default();
    model.frame(&minimap, sample(&buffer, None, 0..2, 100));
    buffer.insert(3, "yy");
    let before = buffer.revision();
    buffer.insert(0, "x");
    let edit = LineEdit {
        line: 0,
        before,
        after: buffer.revision(),
    };
    let frame = model
        .frame(&minimap, sample(&buffer, Some(edit), 0..2, 100))
        .unwrap();
    assert_eq!(bars(&frame), vec![2, 3, 0]);
}

#[test]
fn a_replaced_buffer_with_the_same_line_count_refreshes() {
    let buffer = Buffer::from_bytes(b"a\nb\n").unwrap();
    let minimap = Minimap::new();
    let mut model = MinimapModel::default();
    model.frame(&minimap, sample(&buffer, None, 0..2, 100));
    let replaced = Buffer::from_bytes(b"longer\nb\n").unwrap();
    let frame = model
        .frame(&minimap, sample(&replaced, None, 0..2, 100))
        .unwrap();
    assert_eq!(bars(&frame), vec![6, 1, 0]);
}

#[test]
fn a_huge_file_samples_the_first_line_of_the_row() {
    let path = std::env::temp_dir().join(format!("lux-minimap-huge-{}.txt", std::process::id()));
    std::fs::write(&path, "a\nshort\nthis line is the long one\n").unwrap();
    let buffer = lux_core::buffer::Buffer::open_with_policy(
        &path,
        lux_core::huge_file::HugeFilePolicy::new(1),
    )
    .unwrap();
    assert!(buffer.is_huge_file_mode());
    let frame = build_frame(&Minimap::new(), &buffer, 0..1, 1, None).unwrap();
    assert_eq!(frame.rows.len(), 1);
    assert_eq!(frame.rows[0].line, 0);
    assert_eq!(frame.rows[0].content_columns, 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_huge_file_skips_the_cached_row_model() {
    let path =
        std::env::temp_dir().join(format!("lux-minimap-model-huge-{}.txt", std::process::id()));
    std::fs::write(&path, "a\nshort\nthis line is the long one\n").unwrap();
    let buffer = lux_core::buffer::Buffer::open_with_policy(
        &path,
        lux_core::huge_file::HugeFilePolicy::new(1),
    )
    .unwrap();
    assert!(buffer.is_huge_file_mode());
    let mut model = MinimapModel::default();
    let frame = model
        .frame(&Minimap::new(), sample(&buffer, None, 0..1, 1))
        .unwrap();
    assert_eq!(frame.rows.len(), 1);
    assert_eq!(frame.rows[0].line, 0);
    assert!(model.lines.is_empty());
    let _ = std::fs::remove_file(&path);
}
