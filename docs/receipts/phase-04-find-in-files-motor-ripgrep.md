# Receipt: Find in Files (embedded ripgrep engine) (deliverable 4 of Phase 4)

- Phase: 04
- Crate(s): lux-search, lux-benches
- Goal: The "Find in Files (embedded ripgrep engine)" deliverable of
  Phase 4 ("Projects, files and global search") from
  `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no failures
  and no ignored tests.
- Verify: `cargo test -p lux-search find_in_files::` + the 1 GB benchmark
  (`cargo bench -p lux-benches --bench phase4_budgets`)
- Iterations: 3 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt's deliverable text: "Find in Files with the
  embedded ripgrep engine (`ignore` + `grep-searcher` + `grep-regex`):
  respecting `.gitignore`, include/exclude globs, regex, streamed results
  in a dedicated, navigable panel." Only this bullet was in scope — not
  replace-in-project with diff preview (deliverable 5, the next natural
  step on top of this one) or the incremental index (deliverable 6).
  `docs/STATUS.md`'s "Find in files (ripgrep engine)" row stays
  `not-started` (rule 1): no dedicated panel exists in `lux-ui`, nothing is
  wired into the `lux` binary — this is the engine a panel would consume.

  **The crate choice itself wasn't a fresh decision — ADR 0009 already
  made it** (`ignore` + `grep-searcher` + `grep-regex` for Find in Files,
  `nucleo` for fuzzy matching, both cited together back in Phase 3). What
  was actually this deliverable's work: wiring the engine for real, and
  re-verifying versions before building on them, per `docs/DEPENDENCIES.md`'s
  own re-verification cadence. `ignore = "0.4.31"` (Phase 0's researched
  version) now resolves to `0.4.33` — a real drift caught by `cargo tree`,
  not assumed away. `grep-searcher`/`grep-regex`/`ignore` all turned out to
  be dual-licensed `Unlicense OR MIT`; no `deny.toml` change was needed
  since `MIT` was already allowed (unlike `notify`'s bare `CC0-1.0` in the
  previous loop, which did need a new entry).

  **Streaming is real architecture, following the same shape
  `lux_workspace::watcher::FileWatcher` already established in the loop
  before this one**: a background thread runs `ignore::WalkParallel`
  (fanning the actual directory traversal out across CPU cores, not a
  single-threaded walk with a `String::find` pass), each worker pushes
  matches through an `mpsc::Sender` clone as `grep-searcher` finds them,
  and `SearchStream` on the caller side offers both `poll()` (non-blocking
  drain, what a real panel would call from its own event loop so it stays
  responsive while a search is still running) and `wait_until_done()`
  (blocks for the complete result set — what tests and any "give me
  everything" caller wants). The channel closing (every worker's `Sender`
  clone dropped once `WalkParallel::run` returns) is what tells
  `SearchStream` the search actually finished, the same "channel closing
  is the done signal" idiom the watcher used.

  **Navigable reuses `lux_ui::search_bar::SearchBar`'s exact design, one
  layer down.** `next_match`/`previous_match`/`current`/`current_index`
  wrap around past the ends the same way, over whatever's accumulated in
  `SearchStream.matches` so far — deliberately usable mid-stream, not only
  after `wait_until_done`, since a real Find in Files panel lets you start
  jumping between results while more are still arriving. Returns owned
  `SearchMatch` clones rather than borrows (`SearchMatch` isn't `Copy` —
  it owns a `PathBuf` and a `String` — unlike `search_bar`'s `Selection`,
  which is `Copy` and returns by value for free); the cost is one small
  clone per navigation action, not a hot path.

  **`.gitignore` respect needed one real fix, not zero-config trust in the
  library's defaults.** A first attempt trusted `ignore::WalkBuilder`'s
  defaults outright and `respects_gitignore` failed: `ignore` requires an
  actual `.git` directory before it honors `.gitignore` at all
  (`require_git`, on by default) — correct for a `git status`-like tool,
  wrong for an editor, where a project folder can be a subdirectory of a
  repo, or a folder with a `.gitignore` that hasn't been `git init`-ed yet.
  Fixed with `.require_git(false)`, documented inline with the reasoning,
  not just the flag.

  **Binary/non-UTF-8 files are skipped, not fatal — matching
  `lux_core::buffer::Buffer`'s own stance on binary content, applied at
  the per-file level instead of aborting the whole project search.**
  `grep_searcher::sinks::UTF8` returns an error on invalid UTF-8; that
  error is swallowed per-file (`let _ = searcher.search_path(...)`) so one
  bad file doesn't stop the rest of the tree from being searched. Verified
  directly (`binary_file_is_skipped_not_fatal`).

  **Cancellation is real, not aspirational: dropping a `SearchStream`
  actually stops the workers finding more matches for nobody.** The
  `UTF8` sink closure checks whether `tx.send` still succeeds; once the
  caller drops the receiver (drops the `SearchStream`), the next `send`
  fails and the closure returns `Ok(false)`, which tells `grep-searcher`
  to stop searching that file. This doesn't halt *other* files' workers
  instantly, but it's a real, low-cost win, not a TODO — reaching for a
  full cancellation-token system wasn't justified for what this
  deliverable asks.

  **The 1 GB benchmark is a real on-disk corpus, not a synthetic
  estimate** (`benches/benches/phase4_budgets.rs`): 2,000 files across 20
  subdirectories, ~1.0722 GB total, every 500th line a literal needle
  (30,000 real matches), generated fresh before the benchmark run and
  removed after — regenerating it for all of criterion's 10 samples would
  make the benchmark impractically slow, the same reasoning
  `phase1_budgets.rs`'s 100 MB benchmark already used for its own
  `sample_size(10)`. Measured **43.6 ms** (mean, 10 samples) against the
  **< 2 s** budget — a ~46× margin, documented in
  `docs/PERFORMANCE.md` along with the two real reasons for it (multi-core
  parallelism via `WalkParallel`, and OS page-cache warmth on repeated
  samples after the first) rather than letting the number stand
  unqualified.

  **Actual verification.** `cargo test -p lux-search find_in_files::`:
  11/11 passed, 0 ignored — 10 passed on the very first run; only
  `respects_gitignore` failed initially (the `require_git` fix above),
  fixed and reverified, all green on the second run. Coverage: matches
  found across multiple files with correct path/line-number/line-content;
  `.gitignore` respected; include-glob restricting to matching files;
  exclude-glob removing matching files; real regex (not literal-text
  matching, `id: \d+` against mixed numeric/non-numeric lines);
  case-insensitive option; an invalid regex pattern surfacing as
  `SearchError::InvalidPattern` before any search runs; an empty,
  zero-match search correctly finishing with no current match; navigation
  wrapping both directions; `poll()` returning only genuinely new matches
  on a second call; a binary file not aborting the search. Full repo gate
  also run and green: `cargo run -p xtask` — `cargo fmt --all --check`;
  `cargo clippy --workspace --all-targets -- -D warnings` (zero warnings);
  `cargo test --workspace` (all crates, no regressions — `lux-search` now
  at 19 tests total, 8 previous `fuzzy::` + 11 new `find_in_files::`, every
  other crate's count unchanged from the prior two loops); `cargo deny
  check` (advisories/bans/licenses/sources all `ok`, no new `deny.toml`
  entries needed); `scripts/check-limits.sh` (`find_in_files.rs` at 451
  lines and `phase4_budgets.rs` at 75, both under the 600 cap; `SearchMatch`/
  `SearchOptions`/`SearchStream` — the crate's largest new structs — have
  3, 3, and 4 fields respectively, well under 25); `scripts/check-core-
  isolation.sh` doesn't apply (only checks `lux-core`). Verified on macOS
  (aarch64-apple-darwin) only.

  **What's out of scope and untouched:** replace-in-project with diff
  preview, the incremental index, sessions/recovery, and huge/binary file
  handling (Phase 4 deliverables 5-8); a dedicated, navigable results
  panel in `lux-ui` (no interactive UI exists there yet — this deliverable
  is the engine such a panel would call into); wiring `find_in_files` into
  the `lux` binary; Windows/Linux verification (no non-macOS dev
  environment available).
