//! Build-tool error locations (`file:line` / `file:line:col`) scanned from
//! terminal text. Jumping is returning a [`SourceLocation`]; opening a
//! buffer is `lux-ui`'s job.

use std::path::{Path, PathBuf};

use crate::pty::Pty;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Line;

/// A path the compiler named, plus 1-based line (and optional column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: u32,
    pub column: Option<u32>,
}

/// Scan `text` for rustc `--> path:line:col`, gcc/clang/tsc
/// `path:line:col:`, and Python `File "path", line N`.
///
/// ponytail: three literal patterns, not a grammar. Add a pattern when a
/// real tool we run emits a shape these miss.
pub fn detect_links(text: &str) -> Vec<SourceLocation> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(loc) = rustc_arrow(line)
            .or_else(|| gcc_style(line))
            .or_else(|| python_file(line))
        {
            out.push(loc);
        }
    }
    out
}

fn rustc_arrow(line: &str) -> Option<SourceLocation> {
    let rest = line.trim_start().strip_prefix("--> ")?;
    parse_path_line_col(rest.trim())
}

fn gcc_style(line: &str) -> Option<SourceLocation> {
    // `file:line:col:` or `file:line:` then a message. Skip URLs (`://`).
    if line.contains("://") {
        return None;
    }
    parse_path_line_col_prefix(line.trim())
}

fn python_file(line: &str) -> Option<SourceLocation> {
    let rest = line.trim_start().strip_prefix("File \"")?;
    let (path, after) = rest.split_once("\", line ")?;
    let line_n: u32 = after
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    if path.is_empty() {
        return None;
    }
    Some(SourceLocation {
        path: PathBuf::from(path),
        line: line_n,
        column: None,
    })
}

fn parse_path_line_col(s: &str) -> Option<SourceLocation> {
    parse_path_line_col_prefix(s.trim())
}

fn parse_path_line_col_prefix(s: &str) -> Option<SourceLocation> {
    let colon = s.find(':')?;
    let path = &s[..colon];
    let rest = &s[colon + 1..];
    let (line_s, after_line) = leading_digits(rest)?;
    let line: u32 = line_s.parse().ok()?;
    if line == 0 {
        return None;
    }
    let column = after_line.strip_prefix(':').and_then(|r| {
        leading_digits(r)
            .and_then(|(n, _)| n.parse().ok())
            .filter(|c| *c > 0)
    });
    location(path, line, column)
}

fn leading_digits(s: &str) -> Option<(&str, &str)> {
    let n = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    (n > 0).then_some((&s[..n], &s[n..]))
}

fn location(path: &str, line: u32, column: Option<u32>) -> Option<SourceLocation> {
    let path = path.trim();
    if path.is_empty() || Path::new(path).file_name().is_none() {
        return None;
    }
    Some(SourceLocation {
        path: PathBuf::from(path),
        line,
        column,
    })
}

impl Pty {
    /// Entire VT buffer (scrollback + viewport) as plain text, one line
    /// per grid row — what link detection wants, not just the live view.
    pub fn buffer_text(&self) -> String {
        let term = self.term.lock();
        let grid = term.grid();
        let history = grid.total_lines().saturating_sub(grid.screen_lines());
        let start = -(history as i32);
        let end = grid.screen_lines() as i32;
        let mut lines = Vec::with_capacity((end - start) as usize);
        for row in start..end {
            let mut text = String::with_capacity(grid.columns());
            for cell in &grid[Line(row)] {
                text.push(cell.c);
            }
            lines.push(text.trim_end().to_string());
        }
        lines.join("\n")
    }

    pub fn detect_links(&self) -> Vec<SourceLocation> {
        detect_links(&self.buffer_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rustc_gcc_and_python_shapes() {
        let text = "\
error: nope
 --> src/main.rs:12:5
src/lib.rs:3:1: error: missing
File \"tests/foo.py\", line 9, in <module>
http://example.com:8080/nope
";
        let links = detect_links(text);
        assert_eq!(
            links,
            vec![
                SourceLocation {
                    path: PathBuf::from("src/main.rs"),
                    line: 12,
                    column: Some(5),
                },
                SourceLocation {
                    path: PathBuf::from("src/lib.rs"),
                    line: 3,
                    column: Some(1),
                },
                SourceLocation {
                    path: PathBuf::from("tests/foo.py"),
                    line: 9,
                    column: None,
                },
            ]
        );
    }
}

#[cfg(all(test, unix))]
mod pty_tests {
    use super::*;
    use crate::{CommandBuilder, PtySize};
    use std::time::{Duration, Instant};

    fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if condition() {
                return true;
            }
            if Instant::now() >= deadline {
                return condition();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn cargo_style_error_in_real_shell_output_is_a_link() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "printf '%s\\n' ' --> crates/lux-term/src/lib.rs:4:1'"]);
        let pty = Pty::spawn(
            cmd,
            PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
        )
        .expect("spawn");
        assert!(
            wait_until(Duration::from_secs(5), || !pty.detect_links().is_empty()),
            "no links in:\n{}",
            pty.buffer_text()
        );
        let loc = &pty.detect_links()[0];
        assert_eq!(loc.path, PathBuf::from("crates/lux-term/src/lib.rs"));
        assert_eq!(loc.line, 4);
        assert_eq!(loc.column, Some(1));
        let _ = pty.kill_and_wait();
    }
}
