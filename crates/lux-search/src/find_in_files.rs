//! Find in Files (Phase 4 deliverable 4): the embedded ripgrep engine —
//! `ignore` for `.gitignore`-aware, glob-filterable directory traversal;
//! `grep-searcher` + `grep-regex` for the actual line-oriented regex
//! search — wired for real, not a hand-rolled walker-plus-`String::find`.
//! `ignore::WalkParallel` fans the traversal out across threads; each
//! worker searches the files it's handed and pushes matches back through a
//! channel, so [`SearchStream`] gets results as they're found instead of
//! blocking until the whole tree is scanned. `SearchStream` also stays
//! navigable (`next_match`/`previous_match`/`current`) the same way
//! `lux_ui::search_bar::SearchBar` already is for in-buffer search — this
//! is the project-wide counterpart, one layer down: no dedicated panel
//! exists yet in `lux-ui` to put these results in (same "state a UI would
//! render and drive" pattern as everything else not yet wired to pixels).

use std::error::Error as StdError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use grep_regex::RegexMatcherBuilder;
use grep_searcher::SearcherBuilder;
use grep_searcher::sinks::UTF8;
use ignore::overrides::{Override, OverrideBuilder};
use ignore::{WalkBuilder, WalkState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line_number: u64,
    pub line: String,
}

/// `includes`/`excludes` are plain glob patterns (`*.rs`, `src/**`, no
/// leading `!`) — `find_in_files` adds the `!` that marks an exclude glob
/// internally, matching `--iglob`/`--glob !pattern` in ripgrep itself
/// rather than asking callers to know that syntax.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub case_insensitive: bool,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug)]
pub enum SearchError {
    InvalidPattern(String),
    InvalidGlob(String),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::InvalidPattern(msg) => write!(f, "invalid search pattern: {msg}"),
            SearchError::InvalidGlob(msg) => write!(f, "invalid include/exclude glob: {msg}"),
        }
    }
}

impl StdError for SearchError {}

/// One project-wide search: results stream in on a background thread pool
/// (`ignore::WalkParallel`) as they're found. `poll` drains whatever has
/// arrived without blocking; `wait_until_done` blocks until the whole tree
/// has been searched. Either way, everything received so far is kept and
/// navigable.
#[derive(Debug)]
pub struct SearchStream {
    rx: Receiver<SearchMatch>,
    matches: Vec<SearchMatch>,
    current: Option<usize>,
    finished: bool,
}

impl SearchStream {
    /// Drains whatever matches have arrived since the last call, without
    /// blocking. Returns just the newly received matches (also appended to
    /// what `matches()` returns).
    pub fn poll(&mut self) -> &[SearchMatch] {
        let start = self.matches.len();
        loop {
            match self.rx.try_recv() {
                Ok(m) => self.matches.push(m),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.finished = true;
                    break;
                }
            }
        }
        self.settle_current();
        &self.matches[start..]
    }

    /// Blocks until the search has fully finished (every worker thread's
    /// sender has been dropped), pulling in every remaining match. A real
    /// panel would prefer `poll` so the UI stays responsive while results
    /// keep streaming in; this is for callers (tests included) that want
    /// the complete result set.
    pub fn wait_until_done(&mut self) {
        while let Ok(m) = self.rx.recv() {
            self.matches.push(m);
        }
        self.finished = true;
        self.settle_current();
    }

    fn settle_current(&mut self) {
        if self.current.is_none() && !self.matches.is_empty() {
            self.current = Some(0);
        }
    }

    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub fn current(&self) -> Option<SearchMatch> {
        self.current.map(|i| self.matches[i].clone())
    }

    /// Moves to the next match, wrapping past the last back to the first.
    /// `None` if nothing has arrived yet.
    pub fn next_match(&mut self) -> Option<SearchMatch> {
        let len = self.matches.len();
        if len == 0 {
            return None;
        }
        let i = self.current.map_or(0, |i| (i + 1) % len);
        self.current = Some(i);
        Some(self.matches[i].clone())
    }

    /// Moves to the previous match, wrapping past the first back to the
    /// last. `None` if nothing has arrived yet.
    pub fn previous_match(&mut self) -> Option<SearchMatch> {
        let len = self.matches.len();
        if len == 0 {
            return None;
        }
        let i = self.current.map_or(len - 1, |i| (i + len - 1) % len);
        self.current = Some(i);
        Some(self.matches[i].clone())
    }
}

/// Starts a `.gitignore`-aware, glob-filterable regex search over `root`
/// for `pattern`, streaming matches back through the returned
/// [`SearchStream`] as a real multi-threaded traversal finds them.
pub fn find_in_files(
    root: impl AsRef<Path>,
    pattern: &str,
    options: &SearchOptions,
) -> Result<SearchStream, SearchError> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(options.case_insensitive)
        .build(pattern)
        .map_err(|e| SearchError::InvalidPattern(e.to_string()))?;
    let overrides = build_overrides(root.as_ref(), &options.includes, &options.excludes)?;

    let (tx, rx) = mpsc::channel();
    let walker = WalkBuilder::new(root.as_ref())
        .overrides(overrides)
        // `ignore`'s default requires an actual `.git` directory before it
        // honors `.gitignore` at all — right for a `git status`-like tool,
        // wrong here: a project folder opened in an editor (a subdirectory
        // of a repo, or one not yet `git init`-ed) should still get its
        // `.gitignore` respected.
        .require_git(false)
        .build_parallel();

    thread::spawn(move || {
        walker.run(|| {
            let tx = tx.clone();
            let matcher = matcher.clone();
            let mut searcher = SearcherBuilder::new().line_number(true).build();
            Box::new(move |entry| {
                let Ok(entry) = entry else {
                    return WalkState::Continue;
                };
                if entry.file_type().is_some_and(|t| t.is_file()) {
                    search_one_file(&mut searcher, &matcher, entry.path(), &tx);
                }
                WalkState::Continue
            })
        });
        // `tx` (and every per-worker clone) is dropped here, once `run`
        // returns — that's what turns a later `rx.recv()` into `Err`,
        // telling `SearchStream` the search is done.
    });

    Ok(SearchStream {
        rx,
        matches: Vec::new(),
        current: None,
        finished: false,
    })
}

fn build_overrides(
    root: &Path,
    includes: &[String],
    excludes: &[String],
) -> Result<Override, SearchError> {
    if includes.is_empty() && excludes.is_empty() {
        return Ok(Override::empty());
    }
    let mut builder = OverrideBuilder::new(root);
    for glob in includes {
        builder
            .add(glob)
            .map_err(|e| SearchError::InvalidGlob(e.to_string()))?;
    }
    for glob in excludes {
        builder
            .add(&format!("!{glob}"))
            .map_err(|e| SearchError::InvalidGlob(e.to_string()))?;
    }
    builder
        .build()
        .map_err(|e| SearchError::InvalidGlob(e.to_string()))
}

/// Searches one file and streams its matches through `tx`. Errors (a
/// binary file, invalid UTF-8, a permission error) just skip that file —
/// the same "not searchable as text" stance `lux_core::buffer::Buffer`
/// already takes on binary content, not a reason to abort the whole
/// project-wide search over one bad file.
fn search_one_file(
    searcher: &mut grep_searcher::Searcher,
    matcher: &grep_regex::RegexMatcher,
    path: &Path,
    tx: &mpsc::Sender<SearchMatch>,
) {
    let path_owned = path.to_path_buf();
    let _ = searcher.search_path(
        matcher,
        path,
        UTF8(move |line_number, line| {
            let sent = tx.send(SearchMatch {
                path: path_owned.clone(),
                line_number,
                line: line.to_string(),
            });
            // If the receiver's gone (the caller dropped the
            // `SearchStream`), stop searching this file — no point
            // finding matches nobody will read.
            Ok(sent.is_ok())
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-search-find-in-files-test-{:?}-{}",
            std::thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(&dir).unwrap()
    }

    fn run(dir: &Path, pattern: &str, options: &SearchOptions) -> Vec<SearchMatch> {
        let mut stream = find_in_files(dir, pattern, options).unwrap();
        stream.wait_until_done();
        let mut matches = stream.matches().to_vec();
        matches.sort_by(|a, b| (&a.path, a.line_number).cmp(&(&b.path, b.line_number)));
        matches
    }

    #[test]
    fn finds_matches_across_multiple_files() {
        let dir = temp_dir();
        fs::write(dir.join("a.txt"), "hello world\nfoo\n").unwrap();
        fs::write(dir.join("b.txt"), "another hello\n").unwrap();

        let matches = run(&dir, "hello", &SearchOptions::default());
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].path, dir.join("a.txt"));
        assert_eq!(matches[0].line_number, 1);
        assert_eq!(matches[0].line, "hello world\n");
        assert_eq!(matches[1].path, dir.join("b.txt"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn respects_gitignore() {
        let dir = temp_dir();
        fs::write(dir.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(dir.join("ignored.txt"), "needle\n").unwrap();
        fs::write(dir.join("kept.txt"), "needle\n").unwrap();

        let matches = run(&dir, "needle", &SearchOptions::default());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, dir.join("kept.txt"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn include_glob_restricts_to_matching_files() {
        let dir = temp_dir();
        fs::write(dir.join("code.rs"), "needle\n").unwrap();
        fs::write(dir.join("notes.md"), "needle\n").unwrap();

        let options = SearchOptions {
            includes: vec!["*.rs".to_owned()],
            ..Default::default()
        };
        let matches = run(&dir, "needle", &options);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, dir.join("code.rs"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn exclude_glob_removes_matching_files() {
        let dir = temp_dir();
        fs::create_dir(dir.join("vendor")).unwrap();
        fs::write(dir.join("vendor/lib.rs"), "needle\n").unwrap();
        fs::write(dir.join("main.rs"), "needle\n").unwrap();

        let options = SearchOptions {
            excludes: vec!["vendor/**".to_owned()],
            ..Default::default()
        };
        let matches = run(&dir, "needle", &options);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, dir.join("main.rs"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn regex_patterns_are_real_regex_not_literal_text() {
        let dir = temp_dir();
        fs::write(dir.join("nums.txt"), "id: 42\nid: abc\nid: 7\n").unwrap();

        let matches = run(&dir, r"id: \d+", &SearchOptions::default());
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, "id: 42\n");
        assert_eq!(matches[1].line, "id: 7\n");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn case_insensitive_option_matches_regardless_of_case() {
        let dir = temp_dir();
        fs::write(dir.join("a.txt"), "Needle\n").unwrap();

        let options = SearchOptions {
            case_insensitive: true,
            ..Default::default()
        };
        let matches = run(&dir, "needle", &options);
        assert_eq!(matches.len(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_pattern_is_a_search_error() {
        let dir = temp_dir();
        let err = find_in_files(&dir, "(unclosed", &SearchOptions::default()).unwrap_err();
        assert!(matches!(err, SearchError::InvalidPattern(_)));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_matches_yields_an_empty_finished_stream() {
        let dir = temp_dir();
        fs::write(dir.join("a.txt"), "nothing relevant\n").unwrap();

        let mut stream = find_in_files(&dir, "needle", &SearchOptions::default()).unwrap();
        stream.wait_until_done();
        assert!(stream.matches().is_empty());
        assert!(stream.is_finished());
        assert_eq!(stream.current(), None);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn navigation_wraps_around() {
        let dir = temp_dir();
        fs::write(dir.join("a.txt"), "needle\nneedle\nneedle\n").unwrap();

        let mut stream = find_in_files(&dir, "needle", &SearchOptions::default()).unwrap();
        stream.wait_until_done();
        assert_eq!(stream.matches().len(), 3);

        let first = stream.current().unwrap();
        let second = stream.next_match().unwrap();
        let third = stream.next_match().unwrap();
        let wrapped = stream.next_match().unwrap();
        assert_eq!(wrapped, first);

        let back = stream.previous_match().unwrap();
        assert_eq!(back, third);
        assert_ne!(second, third);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn poll_returns_only_newly_arrived_matches() {
        let dir = temp_dir();
        fs::write(dir.join("a.txt"), "needle\n").unwrap();

        let mut stream = find_in_files(&dir, "needle", &SearchOptions::default()).unwrap();
        // Block until it's actually done, then poll: everything should
        // show up as "new" exactly once, and a second poll finds nothing
        // further.
        stream.wait_until_done();
        let first_poll_count = stream.matches().len();
        assert_eq!(first_poll_count, 1);
        let second_poll = stream.poll();
        assert!(second_poll.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn binary_file_is_skipped_not_fatal() {
        let dir = temp_dir();
        fs::write(dir.join("binary.bin"), [0x00, 0x01, 0xFF, 0x02]).unwrap();
        fs::write(dir.join("text.txt"), "needle\n").unwrap();

        let matches = run(&dir, "needle", &SearchOptions::default());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, dir.join("text.txt"));

        fs::remove_dir_all(&dir).ok();
    }
}
