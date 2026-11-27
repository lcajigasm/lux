//! The minimap's marks and strokes, painted from the app's real state: git
//! markers from the same diff the git worker runs on a temporary repository,
//! bookmarks from the app, and stroke colors from the live highlighter.

use super::*;
use crate::app::App;
use std::path::Path;

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

fn strip() -> Strip {
    Strip {
        x: 700.0,
        y: 30.0,
        w: 90.0,
        h: 600.0,
        row_h: 3.0,
        rows: 200,
        stroke: 1.0,
    }
}

fn marks(app: &App, theme: &Theme) -> Vec<QuadInstance> {
    let doc = MinimapDoc {
        id: 1,
        buffer: &app.buffer,
        colors: None,
        bookmarks: &app.bookmarks,
        folds: &app.fold_state,
    };
    let mut quads = Vec::new();
    let marks = Marks {
        git: &app.ui.git.markers,
        search: app.ui.search_bar.as_ref(),
    };
    paint_marks(Some(&marks), &doc, &strip(), theme, &mut quads);
    quads
}

#[test]
fn a_modified_line_gets_a_modified_mark_on_its_row() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = lux_git::gix::init(dir.path()).expect("init");
    let file = dir.path().join("note.txt");
    std::fs::write(&file, "alpha\nbeta\ngamma\n").unwrap();
    lux_git::staging::stage(&repo, Path::new("note.txt")).unwrap();
    lux_git::commit::commit(&repo, "add note", "Lux Test", "lux-test@example.com").unwrap();

    let mut app = app();
    let text = "alpha\nBETA\ngamma\n";
    app.buffer = lux_core::buffer::Buffer::from_bytes(text.as_bytes()).unwrap();
    let shot = crate::git::snapshot::file_shot(Some(&file), text);
    app.ui.git.markers = shot.markers;
    assert!(
        app.ui
            .git
            .markers
            .iter()
            .any(|m| m.kind == HunkKind::Modified && m.line == 2),
        "{:?}",
        app.ui.git.markers
    );

    let theme = Theme::MONOKAI;
    let quads = marks(&app, &theme);
    let s = strip();
    let modified = color(gutter_color(GutterKind::Modified, theme.background));
    assert!(
        quads
            .iter()
            .any(|q| q.rect == [s.x, s.row_y(1), 2.0, s.row_h] && q.color == modified),
        "no modified mark on row 1"
    );
    assert!(
        !quads.iter().any(|q| q.rect[1] == s.row_y(0)),
        "line 1 is unchanged"
    );
}

#[test]
fn a_bookmark_gets_a_mark_on_the_right_edge() {
    let mut app = app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"a\nb\nc\nd\n").unwrap();
    app.bookmarks.toggle(2);
    let s = strip();
    let quads = marks(&app, &Theme::MONOKAI);
    assert!(
        quads
            .iter()
            .any(|q| q.rect == [s.x + s.w - 2.0, s.row_y(2), 2.0, s.row_h] && q.color == BOOKMARK)
    );
}

#[test]
fn strokes_take_the_highlighters_token_colors() {
    let mut app = app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"fn main() {}\n").unwrap();
    app.set_language_mode(Some(lux_syntax::LanguageId::Rust));
    let syntax = app.syntax.as_ref().expect("highlighter");
    let keyword = syntax
        .tokens
        .iter()
        .find(|t| t.start_byte == 0)
        .map(|t| syntax.theme.color_for_token(t))
        .expect("a token on `fn`");
    let colors = SyntaxColors(syntax);
    let frame = app
        .ui
        .minimap
        .frame(1, &app.buffer, 0..1, 100, None, Some(&colors))
        .unwrap();
    let first = frame.row_spans(0)[0];
    assert_eq!((first.start, first.len), (0, 2));
    assert_eq!(first.color, Some([keyword.r, keyword.g, keyword.b]));
}

/// The color at byte 0 of the first row, as the minimap paints it from the
/// live highlighter.
fn first_stroke_color(app: &mut App) -> Option<[u8; 3]> {
    let syntax = app.syntax.as_ref().expect("highlighter");
    let colors = SyntaxColors(syntax);
    let frame = app
        .ui
        .minimap
        .frame(1, &app.buffer, 0..1, 100, None, Some(&colors))
        .unwrap();
    frame.row_spans(0)[0].color
}

fn token_color_at_start(app: &App) -> Option<[u8; 3]> {
    let syntax = app.syntax.as_ref().expect("highlighter");
    syntax
        .tokens
        .iter()
        .find(|t| t.start_byte == 0)
        .map(|t| syntax.theme.color_for_token(t))
        .map(|c| [c.r, c.g, c.b])
}

#[test]
fn switching_the_language_recolors_strokes_without_an_edit() {
    let mut app = app();
    app.buffer = lux_core::buffer::Buffer::from_bytes(b"def f():\n    pass\n").unwrap();
    app.set_language_mode(Some(lux_syntax::LanguageId::Rust));
    let rust = first_stroke_color(&mut app);
    assert_eq!(rust, token_color_at_start(&app));

    app.set_language_mode(Some(lux_syntax::LanguageId::Python));
    let python = token_color_at_start(&app);
    assert!(python.is_some(), "Python highlights `def`");
    assert_ne!(python, rust, "the two languages color `def` differently");
    assert_eq!(first_stroke_color(&mut app), python);
}
