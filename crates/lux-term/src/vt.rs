//! VT conformance pins and a flood proxy for "100 MB/s without missing a
//! frame": `snapshot` of a live grid stays under 16 ms after a large
//! parser advance. The literal 100 MB/s rate is measured in `--release`.

use std::io::Write;
use std::sync::mpsc;
use std::time::Instant;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Line;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

struct GridSize {
    rows: usize,
    cols: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

struct Discard(mpsc::Sender<Event>);

impl EventListener for Discard {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

fn term() -> (Term<Discard>, Processor<StdSyncHandler>) {
    let (tx, _rx) = mpsc::channel();
    let term = Term::new(
        Config::default(),
        &GridSize { rows: 24, cols: 80 },
        Discard(tx),
    );
    (term, Processor::new())
}

fn snapshot(term: &Term<Discard>) -> String {
    let grid = term.grid();
    let top = -(grid.display_offset() as i32);
    let mut lines = Vec::with_capacity(grid.screen_lines());
    for row in 0..grid.screen_lines() {
        let mut text = String::with_capacity(grid.columns());
        for cell in &grid[Line(top + row as i32)] {
            text.push(cell.c);
        }
        lines.push(text.trim_end().to_string());
    }
    lines.join("\n")
}

#[test]
fn sgr_and_cursor_addressing_land_on_the_grid() {
    let (mut term, mut processor) = term();
    // CUP row 2 col 1, print, SGR red (parser must not drop the text).
    processor.advance(&mut term, b"\x1b[2;1Hhello\x1b[31mX\x1b[0m");
    let snap = snapshot(&term);
    assert!(
        snap.lines().nth(1).is_some_and(|l| l.starts_with("helloX")),
        "unexpected grid:\n{snap}"
    );
}

#[test]
fn newline_wraps_and_scrolls_inside_the_viewport() {
    let (mut term, mut processor) = term();
    for i in 1..=30 {
        processor.advance(&mut term, format!("row-{i}\r\n").as_bytes());
    }
    let snap = snapshot(&term);
    assert!(
        snap.contains("row-30"),
        "last line should be visible:\n{snap}"
    );
    assert!(
        !snap.contains("row-1\n") && !snap.starts_with("row-1"),
        "row-1 should have scrolled off a 24-row screen:\n{snap}"
    );
}

#[test]
fn snapshot_stays_under_one_frame_after_a_flood() {
    let (mut term, mut processor) = term();
    let mut chunk = Vec::with_capacity(80 * 4_000);
    for i in 0..4_000 {
        let _ = writeln!(&mut chunk, "flood-{i:04}");
    }
    processor.advance(&mut term, &chunk);
    let t0 = Instant::now();
    let snap = snapshot(&term);
    let elapsed = t0.elapsed();
    assert!(!snap.is_empty());
    assert!(
        elapsed.as_millis() < 16,
        "snapshot took {elapsed:?}, budget is one 60 Hz frame"
    );
}

#[test]
fn vt_parser_sustains_100mb_per_second() {
    let (mut term, mut processor) = term();
    let line =
        b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n";
    let mut buf = Vec::new();
    // 1.25 MB in debug so the test stays short; 32 MB in release for the
    // named 100 MB/s budget.
    let target = if cfg!(debug_assertions) {
        1_250_000usize
    } else {
        32_000_000usize
    };
    while buf.len() < target {
        buf.extend_from_slice(line);
    }
    let t0 = Instant::now();
    processor.advance(&mut term, &buf);
    let elapsed = t0.elapsed().as_secs_f64().max(1e-9);
    let mb_s = buf.len() as f64 / elapsed / 1_000_000.0;
    let min = if cfg!(debug_assertions) { 10.0 } else { 100.0 };
    assert!(
        mb_s >= min,
        "VT parse rate {mb_s:.1} MB/s below {min} MB/s ({} bytes in {elapsed:.3}s)",
        buf.len()
    );
}
