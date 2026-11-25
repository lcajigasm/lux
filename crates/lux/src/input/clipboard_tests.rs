//! Per-caret copy/cut/paste driven through the keyboard path (ST-EDIT-02).

use super::*;
use lux_core::buffer::Buffer;
use lux_platform::{Modifiers, PlatformError};
use std::cell::RefCell;

#[derive(Default)]
struct TestClipboard(RefCell<String>);

impl Clipboard for TestClipboard {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(self.0.borrow().clone())
    }

    fn write_text(&self, text: &str) -> Result<(), PlatformError> {
        *self.0.borrow_mut() = text.to_owned();
        Ok(())
    }
}

fn app(text: &str) -> App {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
    app
}

fn chord(app: &mut App, clipboard: &dyn Clipboard, key: &str, shift: bool) {
    crate::menu::clear_claim();
    app.modifiers = Modifiers {
        meta: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        shift,
        ..Default::default()
    };
    app.handle_key_with_clipboard(
        &KeyEvent {
            key: Key::Character(key.into()),
            text: Some(key.into()),
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        },
        Some(clipboard),
    );
    app.modifiers = Modifiers::default();
}

fn select(app: &mut App, ranges: &[(usize, usize)]) {
    let selections = ranges
        .iter()
        .map(|&(from, to)| {
            Selection::new(
                Position::from_char(&app.buffer, from),
                Position::from_char(&app.buffer, to),
            )
        })
        .collect();
    app.cursors = MultiCursor::from_cursors(selections);
}

fn carets(app: &mut App, at: &[usize]) {
    let ranges: Vec<_> = at.iter().map(|&c| (c, c)).collect();
    select(app, &ranges);
}

fn offset(app: &App, needle: &str) -> usize {
    let text = app.buffer.text();
    text[..text.find(needle).unwrap()].chars().count()
}

#[test]
fn three_words_copied_from_three_lines_paste_one_per_caret() {
    let mut app =
        app("let alpha = 1;\nlet beta = 2;\nlet gamma = 3;\n\nf(, x);\nf(, y);\nf(, z);\n");
    let clipboard = TestClipboard::default();
    let words: Vec<_> = ["alpha", "beta", "gamma"]
        .iter()
        .map(|w| {
            let at = offset(&app, w);
            (at, at + w.len())
        })
        .collect();
    select(&mut app, &words);

    chord(&mut app, &clipboard, "c", false);
    assert_eq!(*clipboard.0.borrow(), "alpha\nbeta\ngamma");

    let targets: Vec<_> = ["(, x", "(, y", "(, z"]
        .iter()
        .map(|t| offset(&app, t) + 1)
        .collect();
    carets(&mut app, &targets);
    chord(&mut app, &clipboard, "v", false);

    assert_eq!(
        app.buffer.text(),
        "let alpha = 1;\nlet beta = 2;\nlet gamma = 3;\n\nf(alpha, x);\nf(beta, y);\nf(gamma, z);\n"
    );
    assert_eq!(app.cursors.count(), 3);
}

#[test]
fn multi_line_pieces_stay_whole_when_distributed() {
    let mut app = app("a1\na2\nb1\nb2\n--\n[]\n[]\n");
    let clipboard = TestClipboard::default();
    // Two selections, each spanning two lines.
    select(&mut app, &[(0, 5), (6, 11)]);
    chord(&mut app, &clipboard, "c", false);
    assert_eq!(*clipboard.0.borrow(), "a1\na2\nb1\nb2");

    let first = offset(&app, "[]") + 1;
    carets(&mut app, &[first, first + 3]);
    chord(&mut app, &clipboard, "v", false);

    assert_eq!(
        app.buffer.text(),
        "a1\na2\nb1\nb2\n--\n[a1\na2]\n[b1\nb2]\n"
    );
}

#[test]
fn one_piece_pastes_whole_at_every_caret() {
    let mut app = app("word\nx\ny\nz\n");
    let clipboard = TestClipboard::default();
    select(&mut app, &[(0, 4)]);
    chord(&mut app, &clipboard, "c", false);

    carets(&mut app, &[6, 8, 10]);
    chord(&mut app, &clipboard, "v", false);

    assert_eq!(app.buffer.text(), "word\nxword\nyword\nzword\n");
}

#[test]
fn pieces_paste_joined_at_a_single_caret() {
    let mut app = app("one two three\n");
    let clipboard = TestClipboard::default();
    select(&mut app, &[(0, 3), (4, 7), (8, 13)]);
    chord(&mut app, &clipboard, "c", false);

    carets(&mut app, &[14]);
    chord(&mut app, &clipboard, "v", false);

    assert_eq!(app.buffer.text(), "one two three\none\ntwo\nthree");
}

#[test]
fn mismatched_caret_count_pastes_whole_text_everywhere() {
    let mut app = app("a\nb\nc\n");
    let clipboard = TestClipboard::default();
    // Two pieces, one of them two lines long: three lines of text in total.
    select(&mut app, &[(0, 3), (4, 5)]);
    chord(&mut app, &clipboard, "c", false);
    assert_eq!(*clipboard.0.borrow(), "a\nb\nc");

    // Three carets must not split the remembered text by its lines.
    carets(&mut app, &[1, 3, 5]);
    chord(&mut app, &clipboard, "v", false);

    assert_eq!(app.buffer.text(), "aa\nb\nc\nba\nb\nc\nca\nb\nc\n");
}

#[test]
fn foreign_clipboard_text_ignores_remembered_pieces() {
    let mut app = app("ab\ncd\n");
    let clipboard = TestClipboard::default();
    select(&mut app, &[(0, 1), (3, 4)]);
    chord(&mut app, &clipboard, "c", false);

    // Another application replaces the clipboard with one line.
    *clipboard.0.borrow_mut() = "Z".into();
    carets(&mut app, &[2, 5]);
    chord(&mut app, &clipboard, "v", false);
    assert_eq!(app.buffer.text(), "abZ\ncdZ\n");

    // Foreign text with one line per caret still goes line by line.
    *clipboard.0.borrow_mut() = "1\n2".into();
    carets(&mut app, &[0, 4]);
    chord(&mut app, &clipboard, "v", false);
    assert_eq!(app.buffer.text(), "1abZ\n2cdZ\n");
}

#[test]
fn cut_remembers_pieces_and_paste_is_one_undo_step() {
    let mut app = app("[x] [y]\n");
    let clipboard = TestClipboard::default();
    select(&mut app, &[(1, 2), (5, 6)]);
    chord(&mut app, &clipboard, "x", false);
    assert_eq!(app.buffer.text(), "[] []\n");
    assert_eq!(*clipboard.0.borrow(), "x\ny");

    // Paste back at the collapsed carets: x then y, in document order.
    chord(&mut app, &clipboard, "v", false);
    assert_eq!(app.buffer.text(), "[x] [y]\n");

    chord(&mut app, &clipboard, "z", false);
    assert_eq!(app.buffer.text(), "[] []\n");
}

#[test]
fn keymap_paste_command_distributes_pieces() {
    let mut app = app("p q\n. .\n");
    let clipboard = TestClipboard::default();
    select(&mut app, &[(0, 1), (2, 3)]);
    dispatch::execute_keymap_command(&mut app, "copy", Some(&clipboard), None);
    carets(&mut app, &[5, 7]);
    crate::menu::clear_claim();
    dispatch::execute_keymap_command(&mut app, "paste", Some(&clipboard), None);
    assert_eq!(app.buffer.text(), "p q\n.p .q\n");
}
