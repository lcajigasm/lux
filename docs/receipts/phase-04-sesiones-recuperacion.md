# Receipt: Sessions and recovery (deliverable 7 of Phase 4)

- Phase: 04
- Crate(s): lux-workspace
- Goal: The "Sessions and recovery" deliverable of Phase 4 ("Projects,
  files and global search") from `LUX_PROMPT_MAESTRO.md` is complete and
  its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-workspace session::`
- Iterations: 1 / 6
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt's deliverable text: "Sessions and recovery:
  restore tabs, cursors, scroll, and folders on open; unsaved buffers
  recovered after an unexpected close." Only this bullet was in scope —
  not huge/binary file handling (deliverable 8, the last of Phase 4).
  `docs/STATUS.md`'s "Sessions & crash recovery" row stays `not-started`
  (rule 1): no tab strip anywhere in this codebase actually tracks
  cursors/scroll today (`lux_ui::tabs::TabBar` — Phase 3 — is titles,
  pinning, and ordering only), and no unified `lux` binary exists to own a
  real editor session that would call any of this. This is the
  persistence/recovery mechanism such a session manager would call into,
  not a user-visible capability by itself.

  **Two distinct kinds of state, not one blob — because they have
  different write frequency and different failure semantics.** `Session`
  (folders/tabs/cursors/scroll/active-tab) is cheap, whole-file JSON,
  rewritten on every real UI change. `RecoveryStore` holds actual unsaved
  buffer *text*, which can be large and only needs to exist for buffers
  with real unsaved edits — writing it inside the same JSON blob as
  session layout would mean rewriting potentially megabytes of buffer
  content every time a cursor moves. Splitting them was a design decision
  made before writing code, not something discovered by refactoring
  partway through.

  **Crash detection is a real, tested protocol — not a comment promising
  future work.** A `Session` is persisted with `clean_shutdown = false`
  the moment it's active and `= true` only as the literal last step
  `end_cleanly` performs on a normal quit. Loading a session and finding
  `clean_shutdown` still `false` is the actual signal that whatever wrote
  it never reached that last step — a crash, a `kill -9`, a power loss,
  anything that isn't a clean exit. This needed no new dependency (no
  lock-file crate, no PID-file library): the boolean already living inside
  the session JSON *is* the lock, since a crash by definition never gets
  to flip it to `true`. Verified directly
  (`needs_recovery_is_true_after_save_but_false_after_end_cleanly`) by
  simulating exactly that sequence — save (still running) vs. end_cleanly
  (normal quit) — and checking what a subsequent `load` sees in each case.

  **`Session::to_workspace()` is a real integration point, not a stub
  method for symmetry.** "Restore folders on open" needed a concrete
  target to restore *into*, and `project::Workspace` (deliverable 1,
  three loops ago) already is the multi-root model this codebase uses —
  reusing it via `add_root` per saved folder, rather than inventing a
  second "list of open folders" concept that `Workspace` would then need
  to be reconciled with later. Verified directly
  (`to_workspace_adds_one_root_per_folder`).

  **`RecoveryStore` hashes paths into filenames instead of trying to
  escape them.** An arbitrary file path (spaces, unicode, path
  separators inside a name on some platforms) turned into a safe
  single-component filename is exactly the kind of thing that looks
  simple and has a long tail of edge cases (`lux_ui::file_tree` and
  `lux_search` never needed to solve this — they operate on real paths
  directly, never as filenames-within-a-filename). Hashing the path
  (`std::collections::hash_map::DefaultHasher`, already stdlib, no new
  dependency) into a fixed-width hex filename sidesteps the whole
  problem: the real path lives inside the snapshot's own JSON content,
  never encoded into the filename at all. Collisions are a real
  theoretical possibility with a 64-bit hash but not a practical concern
  at the scale of "how many distinct file paths does one open project
  have."

  **Both types stay caller-driven — no timer, no automatic wiring to
  anything, on purpose.** `RecoveryStore::save` doesn't get called by
  this module itself; a future buffer/document manager calls it on real
  edit events, the same "caller marks state explicitly, module just
  tracks it" shape `watcher::FileWatcher::mark_dirty` already established
  two loops ago. Building an automatic "watch every open buffer and
  snapshot it every N seconds" mechanism here would need buffers that
  don't exist yet in this codebase, and would violate golden rule #5's
  ban on polling besides.

  **Actual verification.** `cargo test -p lux-workspace session::`: 12/12
  passed, 0 ignored, on the first run — no iteration needed. Coverage: a
  fresh empty session when no file exists yet (and it correctly reports
  `needs_recovery() == true`, since nothing has cleanly shut down);
  folders/tabs/active-tab round-tripping through save/load exactly;
  clean-shutdown detection in both directions; a malformed session file
  surfacing as `io::ErrorKind::InvalidData`, not a panic or silent data
  loss; a minimal `{}` file defaulting every field; `to_workspace`
  producing the right root count and paths; a recovery snapshot appearing
  in `recoverable()` after `save`; a second `save` for the same path
  overwriting rather than duplicating; two different paths tracked
  independently; `clear` actually removing a snapshot; `clear` of a path
  that was never saved being a no-op, not an error; `recoverable()`
  returning empty (not erroring) when the recovery directory doesn't exist
  yet. Full repo gate also run and green: `cargo run -p xtask` — `cargo
  fmt --all --check`; `cargo clippy --workspace --all-targets -- -D
  warnings` (zero warnings); `cargo test --workspace` (all crates, no
  regressions — `lux-workspace` now at 53 tests total: 21 `project::` +
  10 `watcher::` + 10 `index::` from the three prior loops + 12 new
  `session::`); `cargo deny check` (advisories/bans/licenses/sources all
  `ok`, no new `deny.toml` entries — no new dependency at all, `serde`/
  `serde_json` were already in `lux-workspace` since the `.lux-project`
  deliverable); `scripts/check-limits.sh` (`session.rs` at 406 lines,
  under the 600 cap; `Session` — its largest struct — has 4 fields, well
  under 25); `scripts/check-core-isolation.sh` doesn't apply (only checks
  `lux-core`). Verified on macOS (aarch64-apple-darwin) only.

  **What's out of scope and untouched:** huge/binary file handling
  (Phase 4 deliverable 8, the last one); wiring `Session`/`RecoveryStore`
  into a real editor session, `lux_ui::tabs::TabBar`, or the `lux` binary
  (no real cursor/scroll state exists anywhere yet to persist); an actual
  process-kill crash test (the protocol is verified via the same on-disk
  state sequence a real crash produces, not a literal `kill -9` in a test
  harness); Windows/Linux verification (no non-macOS dev environment
  available).
