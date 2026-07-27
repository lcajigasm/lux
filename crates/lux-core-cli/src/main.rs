//! `lux-core-cli`: a headless test harness for `lux-core`. Opens a file,
//! runs a line-oriented script of buffer operations against it, and dumps
//! (or asserts) the result — end-to-end tests of the core with no UI in
//! the loop, and the living proof that the core doesn't need one.
//!
//! Script grammar, one command per line (`#` starts a comment, blank lines
//! ignored):
//!
//!   open <path>              load a file (relative to the script's own
//!                            directory) as the current buffer
//!   insert <char_idx> <text> insert text at a char offset
//!   delete <char_idx> <len>  delete a run of chars
//!   undo                     undo the last insert/delete
//!   redo                     redo the last undone one
//!   dump                     print the current buffer text to stdout
//!   assert_eq <text>         fail (nonzero exit) if the buffer text
//!                            doesn't match exactly

use lux_core::buffer::Buffer;
use lux_core::edit::Edit;
use lux_core::history::{History, Transaction};
use lux_core::position::Position;
use lux_core::selection::Selection;
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Session {
    buffer: Option<Buffer>,
    history: History,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let script_path = match parse_args(&args) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    match run_script(&script_path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("lux-core-cli: {message}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: &[String]) -> Result<PathBuf, String> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--script" {
            return iter
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "--script requires a path argument".to_string());
        }
    }
    Err("usage: lux-core-cli --script <path>".to_string())
}

fn run_script(script_path: &Path) -> Result<(), String> {
    let script = std::fs::read_to_string(script_path)
        .map_err(|e| format!("reading script {}: {e}", script_path.display()))?;
    let script_dir = script_path.parent().unwrap_or_else(|| Path::new("."));

    let mut session = Session {
        buffer: None,
        history: History::new(),
    };

    for (line_no, raw_line) in script.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        execute(&mut session, line, script_dir)
            .map_err(|e| format!("{}:{}: {e}", script_path.display(), line_no + 1))?;
    }
    Ok(())
}

fn current_buffer(session: &mut Session) -> Result<&mut Buffer, String> {
    session
        .buffer
        .as_mut()
        .ok_or_else(|| "no buffer open — use `open <path>` first".to_string())
}

fn execute(session: &mut Session, line: &str, script_dir: &Path) -> Result<(), String> {
    let (command, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim_start();

    match command {
        "open" => {
            let path = script_dir.join(rest);
            let buffer =
                Buffer::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
            session.buffer = Some(buffer);
            session.history = History::new();
            Ok(())
        }
        "insert" => {
            let (idx_str, text) = rest
                .split_once(char::is_whitespace)
                .ok_or_else(|| "insert requires <char_idx> <text>".to_string())?;
            let idx: usize = idx_str
                .parse()
                .map_err(|_| format!("invalid char index: {idx_str}"))?;
            let buffer = current_buffer(session)?;
            let before = Selection::cursor(Position::from_char(buffer, idx));
            let edit = Edit::insert(idx, text);
            edit.apply(buffer);
            let after = Selection::cursor(Position::from_char(buffer, idx + text.chars().count()));
            session
                .history
                .record(Transaction::single(edit, before, after));
            Ok(())
        }
        "delete" => {
            let mut parts = rest.split_whitespace();
            let idx: usize = parts
                .next()
                .ok_or("delete requires <char_idx> <len>")?
                .parse()
                .map_err(|_| "invalid char index".to_string())?;
            let len: usize = parts
                .next()
                .ok_or("delete requires <char_idx> <len>")?
                .parse()
                .map_err(|_| "invalid length".to_string())?;
            let buffer = current_buffer(session)?;
            let before = Selection::cursor(Position::from_char(buffer, idx + len));
            let edit = Edit::delete(buffer, idx, len);
            edit.apply(buffer);
            let after = Selection::cursor(Position::from_char(buffer, idx));
            session
                .history
                .record(Transaction::single(edit, before, after));
            Ok(())
        }
        "undo" => {
            // Field-level destructuring, not `current_buffer(session)`: that
            // helper ties its returned `&mut Buffer` to the whole `&mut
            // Session`, so a second borrow of `session.history` in the same
            // call wouldn't be seen as disjoint. Splitting the struct here
            // borrows `buffer` and `history` independently.
            let Session { buffer, history } = session;
            let buffer = buffer
                .as_mut()
                .ok_or_else(|| "no buffer open — use `open <path>` first".to_string())?;
            history.undo(buffer);
            Ok(())
        }
        "redo" => {
            let Session { buffer, history } = session;
            let buffer = buffer
                .as_mut()
                .ok_or_else(|| "no buffer open — use `open <path>` first".to_string())?;
            history.redo(buffer);
            Ok(())
        }
        "dump" => {
            let buffer = current_buffer(session)?;
            println!("{}", buffer.text());
            Ok(())
        }
        "assert_eq" => {
            let buffer = current_buffer(session)?;
            let actual = buffer.text();
            if actual != rest {
                return Err(format!(
                    "assert_eq failed:\n  expected: {rest:?}\n  actual:   {actual:?}"
                ));
            }
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}
