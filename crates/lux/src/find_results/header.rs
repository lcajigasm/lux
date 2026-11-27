//! The two meta lines that open a `Find Results` tab.
//!
//! ```text
//! Searching files for "query" (case sensitive)
//! Searching in /absolute/search/root
//! ```
//!
//! The tab's file headers are relative to the search root, and the tab is
//! restored from the session as plain text, so the root and the query
//! flags live in the text itself: F4, Enter and the match boxes read them
//! back from here instead of from state that does not survive a restart.
//! Both lines start with `Searching `, which every reader of the tab
//! already skips as meta.

use std::path::{Path, PathBuf};

const QUERY_PREFIX: &str = "Searching files for \"";
const ROOT_PREFIX: &str = "Searching in ";
const CASE_SENSITIVE: &str = " (case sensitive)";

/// The header for a search of `query` under `root`, ending in the blank
/// line that precedes the first file block.
pub(crate) fn header(query: &str, root: Option<&Path>, case_insensitive: bool) -> String {
    // Like Sublime, only the non-default setting is written: Find in
    // Files is case-insensitive unless the user turned that off.
    let flags = if case_insensitive { "" } else { CASE_SENSITIVE };
    let mut out = format!("{QUERY_PREFIX}{query}\"{flags}\n");
    if let Some(root) = root {
        out.push_str(&format!("{ROOT_PREFIX}{}\n", root.display()));
    }
    out.push('\n');
    out
}

/// The query and whether it was case-insensitive, read from the tab's
/// first line. Tabs from before the flag was written read as the default,
/// case-insensitive, which is what their searches ran with unless the
/// user had turned it off.
pub(crate) fn query(first_line: &str) -> Option<(&str, bool)> {
    let rest = first_line.strip_prefix(QUERY_PREFIX)?;
    let (rest, case_insensitive) = match rest.strip_suffix(CASE_SENSITIVE) {
        Some(rest) => (rest, false),
        None => (rest, true),
    };
    Some((rest.strip_suffix('"')?, case_insensitive))
}

/// The absolute search root, read from the tab's second line. `None` for
/// tabs from before the root was written (their headers are absolute).
pub(crate) fn root(second_line: &str) -> Option<PathBuf> {
    let path = second_line
        .trim_end_matches(['\r', '\n'])
        .strip_prefix(ROOT_PREFIX)?;
    let path = PathBuf::from(path);
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_flags_and_root_round_trip_through_the_text() {
        let root = std::env::temp_dir().join("a \"quoted\" dir");
        let text = header("say \"hi\"", Some(&root), false);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "Searching files for \"say \"hi\"\" (case sensitive)"
        );
        assert_eq!(query(lines[0]), Some(("say \"hi\"", false)));
        assert_eq!(super::root(lines[1]), Some(root));
        assert_eq!(lines[2], "");

        let text = header("x", None, true);
        assert_eq!(text, "Searching files for \"x\"\n\n");
        assert_eq!(query(text.lines().next().unwrap()), Some(("x", true)));
    }

    #[test]
    fn old_tabs_read_as_case_insensitive_without_a_root() {
        assert_eq!(query("Searching files for \"a.b\""), Some(("a.b", true)));
        assert_eq!(super::root(""), None);
        assert_eq!(super::root("/tmp/old.rs:"), None);
        assert_eq!(super::root("Searching in relative/dir"), None);
        assert_eq!(query("plain text"), None);
    }
}
