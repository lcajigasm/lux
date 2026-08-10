# Receipt: Native watchers (`notify` + debouncer) (deliverable 3 of Phase 4)

- Phase: 04
- Crate(s): lux-workspace
- Goal: The "Native watchers (`notify` + debouncer)" deliverable of Phase 4
  ("Projects, files and global search") from `LUX_PROMPT_MAESTRO.md` is
  complete and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-workspace watcher::`
- Iterations: 5 / 10
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt's deliverable text: "Native watchers with
  `notify` 8.2.0 and a debouncer: no polling. Handling of inotify's limits
  on Linux, FSEvents on macOS, and Windows' particularities; reload and a
  conflict warning when a file changes on disk while there are unsaved
  edits." Only this bullet was in scope — not the file tree (deliverable
  2, already closed) or the incremental index it will eventually feed
  (deliverable 6). `docs/STATUS.md`'s "Native FS watchers" row stays
  `not-started` (rule 1): `FileWatcher` is verified against the real OS
  backend, but nothing consumes its output yet — no auto-refresh, no
  reload prompt, nothing wired into the `lux` binary.

  **Dependency versions: not a fresh decision, but not a rubber stamp
  either.** `docs/DEPENDENCIES.md` already fixed `notify` at 8.2.0 back in
  Phase 0. What wasn't fixed was the debouncer — `cargo add
  notify-debouncer-full` on its own resolves to `0.8.0-rc.2`, which tracks
  `notify` 9's still-unstable RC, not 8.2.0. Confirmed the actual pairing
  in a scratch project (`cargo add notify@8.2.0` then `cargo add
  notify-debouncer-full`, which self-selected `0.7.0`, `cargo tree` showing
  one resolved `notify` version with no duplicates) rather than trusting
  crates.io's "latest" tag. `notify` itself turned out to be CC0-1.0
  licensed, not yet in `deny.toml`'s allow list — added there (public-
  domain-equivalent, no copyleft, compatible with MIT OR Apache-2.0) rather
  than working around it.

  **"No polling" is the actual architecture, not just a docstring claim.**
  `FileWatcher::recv`/`try_recv` read from an `mpsc::Receiver` that a
  debouncer background thread pushes into; that thread itself blocks on
  the OS's own notification primitive (FSEvents/inotify/
  `ReadDirectoryChangesW`) and only wakes on a real change. `recv`'s
  `timeout` parameter bounds how long the *caller* waits on the channel —
  it is not a re-check interval, the same distinction any blocking-
  channel-read-with-timeout already has. No `if now - last_check >
  interval` anywhere in this module, per golden rule #5.

  **"Handling FSEvents/inotify/Windows particularities" is real because
  the debouncer already exists to solve exactly that — using it correctly
  is the deliverable, not writing three parallel backend-specific code
  paths.** `notify-debouncer-full`'s own job is normalizing each backend's
  raw event noise (inotify's `rename`+`write`+`chmod` burst for one atomic
  save, FSEvents' coalescing, Windows' own shape) into one clean event per
  path within a window. Reinventing that per-OS in `lux-workspace` would
  duplicate a well-tested library for no reason. What *is* this
  deliverable's own work: Linux's inotify watch-limit error surfaced
  distinctly (`is_watch_limit_reached`, checking
  `notify::ErrorKind::MaxFilesWatch`) instead of a generic error a caller
  can't act on — the one Linux-specific constraint the master prompt names
  explicitly and that a generic "something went wrong" message would hide.

  **Two real macOS quirks were found only by testing against the live
  backend, not by reading `notify`'s docs — and both would have shipped a
  wrong classification if skipped.** A first attempt trusted `event.kind`
  directly (`Create`→`Created`, `Modify`→`Reload`/`Conflict`,
  `Remove`→`Removed`) and 4 of 10 tests failed against the real FSEvents
  backend:

  1. *Editing a file that already existed before `watch()` reports another
     `Create`, not a `Modify`.* Verified this isn't a first-edit timing
     fluke by writing a throwaway debug binary that drained the initial
     historical-replay batch, slept well past any debounce window, then
     wrote to the file a *second* time — still `Create`. Fixed by having
     `watch()` do one synchronous recursive scan of the root into a
     `known`-paths set (documented as real I/O a future UI-thread caller
     must dispatch off-thread, same caveat already on
     `lux_ui::file_tree::FileTree::open`); `classify` then treats a
     `Create` for an already-known path the same as a `Modify`.
  2. *Deleting a file shortly after creating it silently drops the real
     `Remove` event entirely*, even across genuinely separate `fs::write`/
     `fs::remove_file` calls with real wall-clock gaps between them — not
     a same-instant race. Traced with a second debug binary straight
     against the bare `notify` crate (bypassing the debouncer) to confirm
     `notify` itself *does* emit a correct `Remove(File)`; then read
     `notify-debouncer-full` 0.7.0's own source
     (`~/.cargo/registry/.../notify-debouncer-full-0.7.0/src/lib.rs`,
     `push_remove_event`) to find the actual cause: FSEvents coalesces the
     whole create-then-remove lifecycle into one raw batch, and the
     debouncer's own "a `Create` immediately followed by a `Remove`
     cancels out to nothing" dedup heuristic fires on it — correct for a
     genuine same-batch create+delete, wrong here since the two operations
     were real and separate. The real `Remove` never reaches the output
     channel; only the batch's straggling `Modify` events do. Fixed by
     having `classify` treat any event whose path no longer exists on disk
     (`!path.exists()`) as `Removed`, regardless of what kind the debouncer
     attached — a direct, evidence-based correction, not a guess.

  Both fixes are documented inline in `watcher.rs`'s module doc comment
  with the reasoning above, not just the code change, so a future Linux/
  Windows verification pass knows which parts of the classification logic
  were macOS-specific discoveries versus general design.

  **Dirty-conflict tracking is deliberately caller-driven, not automatic.**
  No buffer/document manager exists yet in this codebase to ask "does this
  open file have unsaved edits" — that's a future editor session's
  territory. `mark_dirty`/`mark_clean`/`is_dirty` are the two calls
  whichever layer eventually owns open buffers will drive; `classify`
  itself only ever *reads* that set, never mutates it beyond what `watch`/
  `known`-tracking needs for its own bookkeeping (e.g. a `Removed` event
  clears `known` for that path, but deliberately leaves `dirty` alone —
  the caller owns that decision, not the watcher).

  **Actual verification.** `cargo test -p lux-workspace watcher::`: 10/10
  passed, 0 ignored, on the final run — not the first: the first run (all
  logic-only tests passing, 4 real-filesystem tests failing on a path-
  canonicalization mismatch — macOS's FSEvents backend reports paths
  resolved through symlinks, e.g. `/tmp` → `/private/tmp`, so watching an
  unresolved path meant every reported event failed to string-match) was
  iteration 1; the canonicalization fix was iteration 2; the "known
  problems" investigation and both FSEvents-quirk fixes above were
  iterations 3-4; the `deny.toml` CC0-1.0 entry was iteration 5. Re-ran the
  full `watcher::` suite 3× consecutively after the fixes landed, all green
  each time, to rule out flakiness from the real timing involved (debounce
  window, FSEvents settle delay). Coverage: created/removed/modified
  (clean → reload, dirty → conflict) detection against a real temp
  directory and real `fs` operations; `mark_clean` reverting a path back
  to reload classification; per-path independence of dirty tracking;
  `try_recv`/`recv`-with-timeout both correctly empty with no changes;
  `unwatch` actually stopping further reports from that root; the watch-
  limit-error predicate (tested against a synthetically constructed
  `notify::Error`, not a real inotify exhaustion — see the "not yet
  exercised" note below). Full repo gate also run and green: `cargo run -p
  xtask` — `cargo fmt --all --check`; `cargo clippy --workspace
  --all-targets -- -D warnings` (zero warnings); `cargo test --workspace`
  (all crates, no regressions — `lux-core`'s 235, `lux-workspace`'s
  `project::` 21 from an earlier loop, and `lux-ui`'s `file_tree::` 19 from
  the loop before that, all still pass); `cargo deny check`
  (advisories/bans/sources `ok`; licenses `ok` after adding `CC0-1.0`,
  reasoned above); `scripts/check-limits.sh` (`watcher.rs` at 487 lines,
  under the 600 cap; `FileWatcher` — its largest struct — has 4 fields,
  well under 25); `scripts/check-core-isolation.sh` doesn't apply (only
  checks `lux-core`). Verified on macOS (aarch64-apple-darwin) only.

  **What's out of scope and untouched, and what's genuinely unverified:**
  Find in Files, replace-in-project, the incremental index, sessions/
  recovery, and huge/binary file handling (Phase 4 deliverables 4-8);
  wiring `FileWatcher` into the file tree, an editor session, or the `lux`
  binary; Linux's inotify backend and Windows' `ReadDirectoryChangesW`
  backend — genuinely not run, not just "assumed fine," since there's no
  non-macOS dev environment available (same documented limitation as the
  rest of `docs/STATUS.md`); the actual `MaxFilesWatch` limit under real
  exhaustion (not practical to trigger in a unit test — the predicate is
  tested against a hand-built error value, which verifies the classifier
  logic but not that inotify really produces that exact error kind at the
  real limit).
