//! ST-FIX-03 Acceptance Tests:
//! Continuous typing latency < 8ms on 20,000-line files and non-blocking incremental syntax sync.

use std::time::Instant;

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_syntax::LanguageId;

use crate::app::App;

fn large_test_app(line_count: usize) -> App {
    let mut text = String::with_capacity(line_count * 30);
    for i in 0..line_count {
        text.push_str(&format!("fn func_{i}() -> usize {{ {i} + 1 }}\n"));
    }

    let font_bytes = include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec();
    let mut app = App::new(font_bytes);
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app.cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
    app
}

#[test]
fn test_continuous_typing_latency_large_file_under_8ms() {
    // 20,000 lines of real parseable Rust
    let line_count = 20_000;
    let mut app = large_test_app(line_count);

    // Initialize syntax highlighting
    app.set_language_mode(Some(LanguageId::Rust));
    assert!(app.syntax.is_some());

    // Position cursor in the middle of the document
    let mid_char = app.buffer.len_chars() / 2;
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(
        &app.buffer,
        mid_char,
    )));

    // Perform continuous typing of 30 characters
    let mut latencies = Vec::with_capacity(30);
    for ch in "abcdefghijklmnopqrstuvwxyz1234".chars() {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);

        let t0 = Instant::now();
        app.insert_at_cursor(s);
        let elapsed = t0.elapsed();

        latencies.push(elapsed);
    }

    // Verify all keystrokes completed in under 8 ms (120 FPS frame budget)
    latencies.sort();
    let p99_latency = latencies[(latencies.len() as f64 * 0.99) as usize];
    let max_latency = *latencies.last().unwrap();

    assert!(
        p99_latency.as_millis() < 8,
        "p99 typing latency was {:?}, expected < 8ms",
        p99_latency
    );
    assert!(
        max_latency.as_millis() < 8,
        "max typing latency was {:?}, expected < 8ms",
        max_latency
    );

    // Verify background highlighter receives jobs and updates
    let syn = app.syntax.as_ref().unwrap();
    let res = syn
        .highlighter
        .recv()
        .expect("worker should produce parse result");
    assert!(res.version > 0);
    app.poll_syntax_highlighter();
    assert!(!app.syntax.as_ref().unwrap().tokens.is_empty());
}

#[test]
fn test_incremental_syntax_edits_preserve_cached_tokens() {
    let mut app = large_test_app(100);
    app.set_language_mode(Some(LanguageId::Rust));
    assert!(app.syntax.is_some());

    let initial_token_count = app.syntax.as_ref().unwrap().tokens.len();
    assert!(initial_token_count > 0);

    // Typing inside a function body
    app.cursors = MultiCursor::new(Selection::cursor(Position::from_char(&app.buffer, 15)));
    app.insert_at_cursor("let y = 10; ");

    // The tokens vector should immediately adjust without waiting for background thread
    assert!(
        !app.syntax.as_ref().unwrap().tokens.is_empty(),
        "cached tokens should be preserved and shifted immediately"
    );

    // Let the background thread catch up and poll
    let syn = app.syntax.as_ref().unwrap();
    let _ = syn.highlighter.recv();
    app.poll_syntax_highlighter();
    assert!(app.syntax.as_ref().unwrap().tokens.len() >= initial_token_count);
}
