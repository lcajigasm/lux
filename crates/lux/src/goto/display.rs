//! How a file result reads: its name on the first line, where it lives
//! on the second. The matcher ranked the full candidate path, so its
//! byte ranges are carried over to both lines rather than recomputed.

use std::ops::Range;
use std::path::{Component, Path, PathBuf};

/// A file row's two lines and the match ranges on each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileRow {
    pub name: String,
    pub name_hits: Vec<Range<usize>>,
    pub path: String,
    pub path_hits: Vec<Range<usize>>,
}

fn is_separator(ch: char) -> bool {
    ch == '/' || ch == std::path::MAIN_SEPARATOR
}

/// Ranges of `hits` that fall at or after `from`, moved so `from` lands
/// at `to`.
fn shift(hits: &[Range<usize>], from: usize, to: usize) -> Vec<Range<usize>> {
    hits.iter()
        .filter(|hit| hit.end > from)
        .map(|hit| (hit.start.max(from) - from + to)..(hit.end - from + to))
        .collect()
}

/// `candidate` is relative to the open folder or absolute. A relative
/// path is shown as is; an absolute one under `home` starts with `~`.
pub(crate) fn file_row(candidate: &str, hits: &[Range<usize>], home: Option<&str>) -> FileRow {
    let name_start = candidate.rfind(is_separator).map_or(0, |index| index + 1);
    let (prefix, keep_from) = match home {
        Some(home)
            if !home.is_empty()
                && candidate.len() > home.len()
                && candidate.starts_with(home)
                && candidate[home.len()..].starts_with(is_separator) =>
        {
            ("~", home.len())
        }
        _ => ("", 0),
    };
    FileRow {
        name: candidate[name_start..].to_string(),
        name_hits: shift(hits, name_start, 0),
        path: format!("{prefix}{}", &candidate[keep_from..]),
        path_hits: shift(hits, keep_from, prefix.len()),
    }
}

/// The home directory as text, for `~` abbreviation.
pub(crate) fn home() -> Option<String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| {
            home.to_string_lossy()
                .trim_end_matches(is_separator)
                .to_string()
        })
}

/// Top-level directories macOS links into `/private`: `/tmp` and
/// `/private/tmp` name one folder, so a file opened through either
/// spelling is inside a folder opened through the other.
const PRIVATE_ALIASES: [&str; 3] = ["tmp", "var", "etc"];

/// The other spelling of `root` when it is one of the `/private`
/// aliases. Pure path arithmetic: no file system call.
fn private_alias(root: &Path) -> Option<PathBuf> {
    let mut parts = root.components();
    if parts.next() != Some(Component::RootDir) {
        return None;
    }
    let first = parts.next()?;
    let is_alias = |part: Option<Component<'_>>| {
        matches!(part, Some(Component::Normal(name))
            if PRIVATE_ALIASES.iter().any(|alias| name == *alias))
    };
    if first == Component::Normal("private".as_ref()) {
        let rest = parts.as_path();
        is_alias(rest.components().next()).then(|| Path::new("/").join(rest))
    } else if is_alias(Some(first)) {
        Some(Path::new("/private").join(root.strip_prefix("/").ok()?))
    } else {
        None
    }
}

/// `path` relative to the open folder `root`, so an open tab and the
/// same file from the folder index are one candidate. Besides the folder
/// itself, only its macOS `/private` spelling counts (`/private/tmp/x`
/// for a `/tmp/x` folder and back): any other folder that merely ends
/// with the same names is a different folder.
pub(crate) fn folder_relative(path: &Path, root: &Path) -> Option<PathBuf> {
    let keep = |rel: &Path| (!rel.as_os_str().is_empty()).then(|| rel.to_path_buf());
    if let Ok(rel) = path.strip_prefix(root) {
        return keep(rel);
    }
    let alias = private_alias(root)?;
    path.strip_prefix(&alias).ok().and_then(keep)
}

/// The candidate text for `path`: folder-relative when it is inside the
/// open folder, otherwise the path as is.
pub(crate) fn candidate(path: &Path, root: Option<&Path>) -> String {
    root.and_then(|root| folder_relative(path, root))
        .as_deref()
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit<'a>(text: &'a str, hits: &[Range<usize>]) -> Vec<&'a str> {
        hits.iter().map(|hit| &text[hit.clone()]).collect()
    }

    #[test]
    fn a_relative_path_shows_its_name_and_keeps_both_lines_lit() {
        // "stat" matched in the file name; "s" also in "src".
        let hits = vec![0..1, 4..8];
        let row = file_row("src/status_bar.rs", &hits, Some("/Users/me"));
        assert_eq!(row.name, "status_bar.rs");
        assert_eq!(lit(&row.name, &row.name_hits), ["stat"]);
        assert_eq!(row.path, "src/status_bar.rs");
        assert_eq!(lit(&row.path, &row.path_hits), ["s", "stat"]);
    }

    #[test]
    fn a_path_under_home_is_abbreviated_with_a_tilde() {
        let candidate = "/Users/me/notes/todo.md";
        let start = candidate.find("todo").unwrap();
        let row = file_row(
            candidate,
            std::slice::from_ref(&(start..start + 4)),
            Some("/Users/me"),
        );
        assert_eq!(row.name, "todo.md");
        assert_eq!(row.path, "~/notes/todo.md");
        assert_eq!(lit(&row.path, &row.path_hits), ["todo"]);
        assert_eq!(lit(&row.name, &row.name_hits), ["todo"]);
    }

    #[test]
    fn a_path_outside_home_stays_absolute_and_a_sibling_prefix_is_not_home() {
        let row = file_row("/private/tmp/a/b.rs", &[], Some("/Users/me"));
        assert_eq!(row.path, "/private/tmp/a/b.rs");
        let row = file_row("/Users/meg/b.rs", &[], Some("/Users/me"));
        assert_eq!(row.path, "/Users/meg/b.rs");
        assert_eq!(row.name, "b.rs");
    }

    #[test]
    fn a_hit_that_straddles_the_name_is_clipped_to_it() {
        let row = file_row("ab/cd", std::slice::from_ref(&(1..4)), None);
        assert_eq!(row.name, "cd");
        assert_eq!(row.name_hits, vec![0..1]);
        assert_eq!(row.path_hits, vec![1..4]);
    }

    #[test]
    fn a_bare_name_is_its_own_path() {
        let row = file_row("untitled", std::slice::from_ref(&(0..2)), None);
        assert_eq!(row.name, "untitled");
        assert_eq!(row.path, "untitled");
        assert_eq!(row.name_hits, vec![0..2]);
    }

    #[test]
    fn open_tabs_under_the_folder_become_folder_relative() {
        let root = Some(Path::new("/tmp/fixture"));
        assert_eq!(
            candidate(Path::new("/tmp/fixture/src/main.rs"), root),
            "src/main.rs"
        );
        // macOS spells `/tmp` as `/private/tmp` once a file is open.
        assert_eq!(
            candidate(Path::new("/private/tmp/fixture/src/main.rs"), root),
            "src/main.rs"
        );
        assert_eq!(
            candidate(Path::new("/elsewhere/x.rs"), root),
            "/elsewhere/x.rs"
        );
        // A folder with the same last name elsewhere is not the folder.
        assert_eq!(
            candidate(Path::new("/home/fixture/src/main.rs"), root),
            "/home/fixture/src/main.rs"
        );
        // A copy of the folder elsewhere ends with the same names but is
        // another file: it must not take the folder file's candidate.
        assert_eq!(
            candidate(Path::new("/Volumes/backup/tmp/fixture/src/main.rs"), root),
            "/Volumes/backup/tmp/fixture/src/main.rs"
        );
        assert_eq!(
            candidate(Path::new("/private/var/tmp/fixture/src/main.rs"), root),
            "/private/var/tmp/fixture/src/main.rs"
        );
        assert_eq!(candidate(Path::new("/tmp/fixture"), root), "/tmp/fixture");
        assert_eq!(candidate(Path::new("/a/b.rs"), None), "/a/b.rs");
    }

    #[test]
    fn the_private_alias_works_both_ways_and_only_for_known_roots() {
        let var = Path::new("/var/folders/x/T/proj");
        let private = Path::new("/private/var/folders/x/T/proj");
        let rel = Some(PathBuf::from("src/a.rs"));
        assert_eq!(folder_relative(&private.join("src/a.rs"), var), rel);
        assert_eq!(folder_relative(&var.join("src/a.rs"), private), rel);
        assert_eq!(
            folder_relative(Path::new("/private/etc/app/a.rs"), Path::new("/etc/app")),
            Some(PathBuf::from("a.rs"))
        );
        // `/private/Users` is not an alias of `/Users`.
        assert_eq!(
            folder_relative(
                Path::new("/private/Users/me/p/a.rs"),
                Path::new("/Users/me/p")
            ),
            None
        );
        assert_eq!(
            folder_relative(
                Path::new("/Users/me/p/a.rs"),
                Path::new("/private/Users/me/p")
            ),
            None
        );
        // A backup that ends with the folder's names is another folder.
        assert_eq!(
            folder_relative(
                Path::new("/Volumes/backup/tmp/fixture/src/main.rs"),
                Path::new("/tmp/fixture")
            ),
            None
        );
        assert_eq!(
            folder_relative(Path::new("/private/tmp/fixture"), Path::new("/tmp/fixture")),
            None
        );
        assert_eq!(
            folder_relative(Path::new("src/a.rs"), Path::new("tmp/x")),
            None
        );
    }
}
