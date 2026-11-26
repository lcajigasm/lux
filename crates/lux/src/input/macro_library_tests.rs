//! ST-EXT-03 through the window keyboard path: naming a saved macro in the
//! palette prompt, playing it as "Macro: <name>", and what a macro records
//! for distributed paste, Transpose and Soft Undo.

use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::{Key, Modifiers, NamedKey};

use crate::app::App;
use crate::input::test_ctx::{TestCtx, app, chr, ctrl, press, primary};
use crate::macros::library::read_macros;
use crate::palette::PalettePrompt;

fn key(app: &mut App, ctx: &TestCtx, named: NamedKey) {
    press(app, ctx, Key::Named(named), Modifiers::default());
}

fn type_text(app: &mut App, ctx: &TestCtx, text: &str) {
    for ch in text.chars() {
        press(app, ctx, chr(&ch.to_string()), Modifiers::default());
    }
}

fn record(app: &mut App, ctx: &TestCtx) {
    press(app, ctx, chr("q"), ctrl());
}

fn play_last(app: &mut App, ctx: &TestCtx) {
    press(app, ctx, chr("q"), primary(true));
}

fn carets_at(app: &mut App, chars: &[usize]) {
    let selections = chars
        .iter()
        .map(|&c| Selection::cursor(Position::from_char(&app.buffer, c)))
        .collect();
    app.cursors = MultiCursor::from_cursors(selections);
}

/// Line starts of a buffer whose lines are all `width` chars plus `\n`.
fn line_starts(lines: usize, width: usize) -> Vec<usize> {
    (0..lines).map(|line| line * (width + 1)).collect()
}

struct MacroDir(tempfile::TempDir);

impl MacroDir {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        crate::macros::recording::set_test_dir(Some(dir.path().to_path_buf()));
        Self(dir)
    }
}

impl Drop for MacroDir {
    fn drop(&mut self) {
        crate::macros::recording::set_test_dir(None);
    }
}

/// Lets the save worker finish, then hands its result to the app.
fn finish_save(app: &mut App, ctx: &TestCtx, dir: &std::path::Path, name: &str) {
    let file = dir.join(format!("{name}.sublime-macro"));
    for _ in 0..500 {
        if file.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(file.exists(), "{} was not written", file.display());
    // The worker's result comes back through the same path the event loop
    // drains after the worker's wake.
    for _ in 0..500 {
        crate::ui_io::poll(app, ctx);
        if app.ui.macros.saved(name).is_some() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("save of {name} never reported back");
}

#[test]
fn save_macro_asks_for_a_name_and_two_saves_keep_two_files() {
    let dir = MacroDir::new();
    let ctx = TestCtx::default();
    let mut app = app("abc\n");
    carets_at(&mut app, &[0]);
    record(&mut app, &ctx);
    type_text(&mut app, &ctx, "// ");
    record(&mut app, &ctx);

    for name in ["comment", "other"] {
        crate::palette::execute_command(&mut app, &ctx, "macro.save_as");
        let palette = app.ui.command_palette.as_ref().expect("prompt opened");
        assert_eq!(palette.prompt, Some(PalettePrompt::SaveMacro));
        assert_eq!(palette.placeholder(), "Macro name, then Enter to save…");
        type_text(&mut app, &ctx, name);
        key(&mut app, &ctx, NamedKey::Enter);
        assert!(app.ui.command_palette.is_none(), "Enter closes the prompt");
        finish_save(&mut app, &ctx, dir.0.path(), name);
    }

    let on_disk = read_macros(&[dir.0.path().to_path_buf()]);
    let names: Vec<_> = on_disk.iter().filter_map(|m| m.name.as_deref()).collect();
    assert_eq!(
        names,
        ["comment", "other"],
        "the second save kept the first"
    );
    assert!(!dir.0.path().join("macro.sublime-macro").exists());
}

/// Waits for the save worker to report that the name is taken.
fn finish_refused_save(app: &mut App, ctx: &TestCtx) {
    for _ in 0..500 {
        crate::ui_io::poll(app, ctx);
        if let Some(palette) = app.ui.command_palette.as_ref() {
            assert!(matches!(
                palette.prompt,
                Some(PalettePrompt::ReplaceMacro { .. })
            ));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the save never asked before replacing");
}

#[test]
fn saving_a_taken_name_asks_and_an_edited_name_keeps_both() {
    let dir = MacroDir::new();
    let ctx = TestCtx::default();
    let mut app = app("abc\n");
    record(&mut app, &ctx);
    type_text(&mut app, &ctx, "x");
    record(&mut app, &ctx);
    let first = dir.0.path().join("wrap.sublime-macro");
    std::fs::write(&first, "[]").unwrap();

    crate::palette::execute_command(&mut app, &ctx, "macro.save_as");
    type_text(&mut app, &ctx, "wrap");
    key(&mut app, &ctx, NamedKey::Enter);
    finish_refused_save(&mut app, &ctx);
    assert_eq!(
        app.ui.command_palette.as_ref().unwrap().query.as_str(),
        "wrap"
    );
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "[]");

    type_text(&mut app, &ctx, "2");
    key(&mut app, &ctx, NamedKey::Enter);
    finish_save(&mut app, &ctx, dir.0.path(), "wrap2");
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "[]", "kept both");
}

#[test]
fn an_unusable_name_keeps_the_prompt_open() {
    let dir = MacroDir::new();
    let ctx = TestCtx::default();
    let mut app = app("abc\n");
    record(&mut app, &ctx);
    type_text(&mut app, &ctx, "x");
    record(&mut app, &ctx);

    crate::palette::execute_command(&mut app, &ctx, "macro.save_as");
    type_text(&mut app, &ctx, "../escape");
    key(&mut app, &ctx, NamedKey::Enter);
    assert!(app.ui.command_palette.is_some());
    let message = app.documents.message.clone().unwrap_or_default();
    assert!(message.starts_with("Cannot save macro"), "{message}");
    assert_eq!(std::fs::read_dir(dir.0.path()).unwrap().count(), 0);

    key(&mut app, &ctx, NamedKey::Escape);
    assert!(app.ui.command_palette.is_none());
}

#[test]
fn save_macro_without_a_recording_does_not_open_a_prompt() {
    let _dir = MacroDir::new();
    let ctx = TestCtx::default();
    let mut app = app("abc\n");
    crate::palette::execute_command(&mut app, &ctx, "macro.save_as");
    assert!(app.ui.command_palette.is_none());
    assert_eq!(
        app.documents.message.as_deref(),
        Some("No macro recorded to save")
    );
}

#[test]
fn a_saved_macro_is_a_palette_command_that_plays_on_every_caret() {
    let dir = MacroDir::new();
    let ctx = TestCtx::default();
    let mut source = app("x\n");
    record(&mut source, &ctx);
    type_text(&mut source, &ctx, "> ");
    press(
        &mut source,
        &ctx,
        Key::Named(NamedKey::End),
        Modifiers::default(),
    );
    type_text(&mut source, &ctx, ";");
    record(&mut source, &ctx);
    source.ui.macros.save_macro_as("quote", None).unwrap();
    assert!(dir.0.path().join("quote.sublime-macro").exists());

    // A fresh app learns about it from the disk read, not from the save.
    let mut app = app(&"line\n".repeat(10));
    app.ui
        .macros
        .set_saved(read_macros(&[dir.0.path().to_path_buf()]));
    let recorded_before = app.ui.macros.last_macro.clone();
    carets_at(&mut app, &line_starts(10, 4));

    app.open_command_palette("Macro: quote");
    let palette = app.ui.command_palette.as_ref().unwrap();
    let ctx_flags = lux_ui::command_palette::Context::new();
    let first = palette.results(&ctx_flags)[0].command.clone();
    assert_eq!(first.title, "Macro: quote");
    key(&mut app, &ctx, NamedKey::Enter);

    assert_eq!(app.buffer.text(), "> line;\n".repeat(10));
    assert_eq!(app.ui.macros.last_macro, recorded_before);
}

#[test]
fn a_distributed_paste_is_recorded_and_replayed_piece_by_piece() {
    let ctx = TestCtx::default();
    let mut app = app("ab\ncd\n--\n--\n--\n--\n");
    // Copy "ab" and "cd" from two selections.
    app.cursors = MultiCursor::from_cursors(vec![
        Selection::new(
            Position::from_char(&app.buffer, 0),
            Position::from_char(&app.buffer, 2),
        ),
        Selection::new(
            Position::from_char(&app.buffer, 3),
            Position::from_char(&app.buffer, 5),
        ),
    ]);
    press(&mut app, &ctx, chr("c"), primary(false));

    carets_at(&mut app, &[6, 9]);
    record(&mut app, &ctx);
    press(&mut app, &ctx, chr("v"), primary(false));
    record(&mut app, &ctx);
    assert_eq!(app.buffer.text(), "ab\ncd\nab--\ncd--\n--\n--\n");

    // Something else on the clipboard: the macro replays what it pasted.
    use lux_platform::Clipboard;
    ctx.write_text("other").unwrap();
    carets_at(&mut app, &[16, 19]);
    play_last(&mut app, &ctx);
    assert_eq!(app.buffer.text(), "ab\ncd\nab--\ncd--\nab--\ncd--\n");

    let saved = app.ui.macros.last_macro.clone().unwrap();
    let json = saved.to_sublime_json().unwrap();
    assert!(json.contains("\"insert_pieces\""), "{json}");
    assert!(!json.contains("\"paste\""), "{json}");
}

#[test]
fn transpose_replays_on_every_caret() {
    let ctx = TestCtx::default();
    let mut app = app(&"ab\n".repeat(10));
    carets_at(&mut app, &[1]);
    record(&mut app, &ctx);
    press(&mut app, &ctx, chr("t"), ctrl());
    record(&mut app, &ctx);
    assert!(app.buffer.text().starts_with("ba\nab\n"));

    let starts: Vec<usize> = line_starts(10, 2)
        .into_iter()
        .skip(1)
        .map(|c| c + 1)
        .collect();
    carets_at(&mut app, &starts);
    play_last(&mut app, &ctx);
    assert_eq!(app.buffer.text(), "ba\n".repeat(10));
}

#[test]
fn soft_undo_in_a_macro_steps_back_over_the_previous_replayed_step() {
    let ctx = TestCtx::default();
    let mut app = app("abcd\nabcd\n");
    carets_at(&mut app, &[0]);
    record(&mut app, &ctx);
    key(&mut app, &ctx, NamedKey::ArrowRight);
    key(&mut app, &ctx, NamedKey::ArrowRight);
    press(&mut app, &ctx, chr("u"), primary(false));
    type_text(&mut app, &ctx, "X");
    record(&mut app, &ctx);
    assert_eq!(app.buffer.text(), "aXbcd\nabcd\n");

    carets_at(&mut app, &[6]);
    play_last(&mut app, &ctx);
    assert_eq!(app.buffer.text(), "aXbcd\naXbcd\n");
}

#[test]
fn a_recorded_line_transform_replays_exactly_on_ten_carets() {
    let ctx = TestCtx::default();
    let mut app = app(&"abcd\n".repeat(11));
    carets_at(&mut app, &[0]);
    record(&mut app, &ctx);
    type_text(&mut app, &ctx, "<");
    key(&mut app, &ctx, NamedKey::End);
    key(&mut app, &ctx, NamedKey::Backspace);
    type_text(&mut app, &ctx, ">");
    key(&mut app, &ctx, NamedKey::Home);
    key(&mut app, &ctx, NamedKey::ArrowRight);
    type_text(&mut app, &ctx, "!");
    record(&mut app, &ctx);
    assert!(app.buffer.text().starts_with("<!abc>\nabcd\n"));

    let starts: Vec<usize> = (1..11).map(|line| 7 + (line - 1) * 5).collect();
    carets_at(&mut app, &starts);
    assert_eq!(app.cursors.count(), 10);
    play_last(&mut app, &ctx);
    assert_eq!(app.buffer.text(), "<!abc>\n".repeat(11));
}
