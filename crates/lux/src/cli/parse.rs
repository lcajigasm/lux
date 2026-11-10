//! Command-line locations, stdin, and the help text.

use std::path::PathBuf;

use super::Location;

pub(crate) fn help_text() -> &'static str {
    "\
Lux — Sublime-grade high-performance text editor

Usage: lux [OPTIONS] [DIRECTORY | FILE[:LINE[:COLUMN]] | -]...

Options:
  -h, --help       Print this help and exit
  -v, --version    Print the version and exit
  --wait           Block until every buffer opened by this command is closed
  --new-window     Open a window of its own instead of the one already running
  --palette        Open the command palette
  --goto           Open go to file
  --theme          Open the color scheme picker
  --settings       Open the settings page
  --query TEXT     Prefill the open selector
  -                Read stdin into a new untitled buffer
  FILE:LINE:COLUMN Open that file and place the cursor (lines and columns start at 1)

--wait blocks the client. When this process is the window itself, the
files open and the process does not wait on itself.
--new-window does not hand the paths to an editor that is already running.

The shell command is installed from the palette or the Tools menu.
It links `lux` at Contents/SharedSupport/bin/lux onto PATH.
"
}

/// `FILE:LINE` and `FILE:LINE:COLUMN`. A path that exists is kept whole,
/// so a file whose name really contains a colon stays that file.
pub(crate) fn split_location(arg: &str) -> (PathBuf, Option<u32>, Option<u32>) {
    let path = PathBuf::from(arg);
    if path.exists() {
        return (path, None, None);
    }
    if let Some((file, line, column)) = strip_numbers(arg) {
        return (PathBuf::from(file), Some(line), column);
    }
    (path, None, None)
}

fn strip_numbers(arg: &str) -> Option<(&str, u32, Option<u32>)> {
    let (head, column) = split_trailing_number(arg)?;
    if let Some((file, line)) = split_trailing_number(head) {
        if file.is_empty() || file.ends_with(':') {
            return None;
        }
        return Some((file, line, Some(column)));
    }
    if head.is_empty() || head.ends_with(':') {
        return None;
    }
    Some((head, column, None))
}

fn split_trailing_number(arg: &str) -> Option<(&str, u32)> {
    let (head, tail) = arg.rsplit_once(':')?;
    if tail.is_empty() || !tail.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: u32 = tail.parse().ok()?;
    if number == 0 {
        return None;
    }
    Some((head, number))
}

pub(crate) fn prepare(
    args: &[PathBuf],
    scratch: Option<PathBuf>,
) -> Result<(Vec<PathBuf>, Vec<Location>), String> {
    let mut paths = Vec::new();
    let mut locations = Vec::new();
    let mut saw_stdin = false;
    for arg in args {
        let text = arg.to_string_lossy();
        if text == "-" {
            if saw_stdin {
                return Err("stdin can be read once".into());
            }
            let Some(path) = scratch.clone() else {
                return Err("stdin was requested without a scratch file".into());
            };
            saw_stdin = true;
            paths.push(path.clone());
            locations.push(Location {
                path,
                scratch: true,
                ..Location::default()
            });
            continue;
        }
        let (path, line, column) = split_location(&text);
        paths.push(path.clone());
        locations.push(Location {
            path,
            line,
            column,
            ..Location::default()
        });
    }
    Ok((paths, locations))
}

/// Reads stdin into a scratch file. A terminal becomes an empty untitled
/// buffer instead of blocking the command.
pub(crate) fn read_stdin_scratch() -> Result<PathBuf, String> {
    use std::io::{IsTerminal, Read};
    let mut bytes = Vec::new();
    if !std::io::stdin().is_terminal() {
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
    }
    let path = std::env::temp_dir().join(format!("lux-stdin-{}.txt", std::process::id()));
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_names_the_client_flags() {
        let help = help_text();
        for flag in ["--help", "--wait", "--new-window", ":LINE:COLUMN", "stdin"] {
            assert!(help.contains(flag), "{flag}");
        }
    }

    #[test]
    fn file_line_column_splits_and_a_real_path_does_not() {
        let (path, line, column) = split_location("src/main.rs:12:3");
        assert_eq!(path, PathBuf::from("src/main.rs"));
        assert_eq!(line, Some(12));
        assert_eq!(column, Some(3));
        let (path, line, column) = split_location("src/main.rs:4");
        assert_eq!(path, PathBuf::from("src/main.rs"));
        assert_eq!(line, Some(4));
        assert_eq!(column, None);
        let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let (path, line, _) = split_location(here.to_str().unwrap());
        assert_eq!(path, here);
        assert_eq!(line, None);
    }

    #[test]
    fn stdin_becomes_a_scratch_buffer() {
        let scratch = PathBuf::from("/tmp/lux-stdin.txt");
        let (paths, locations) = prepare(
            &[PathBuf::from("-"), PathBuf::from("a.rs:2:8")],
            Some(scratch.clone()),
        )
        .unwrap();
        assert_eq!(paths, vec![scratch.clone(), PathBuf::from("a.rs")]);
        assert!(locations[0].scratch);
        assert_eq!(locations[1].line, Some(2));
        assert_eq!(locations[1].column, Some(8));
    }
}
