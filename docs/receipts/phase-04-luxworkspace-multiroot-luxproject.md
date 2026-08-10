# Receipt: `lux-workspace`: multi-root, `.lux-project` (deliverable 1 of Phase 4)

- Phase: 04
- Crate(s): lux-workspace
- Goal: The "`lux-workspace`: multi-root, `.lux-project`" deliverable of
  Phase 4 ("Projects, files and global search") from `LUX_PROMPT_MAESTRO.md`
  is complete and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-workspace project::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Starting state.** `lux-workspace` was an empty stub crate (`src/lib.rs`
  was two blank lines) — Phase 0's placeholder, never touched since,
  matching every other not-yet-built crate in the workspace. Nothing in the
  repo referenced it.

  **Scope.** The master prompt's deliverable text is one line: "project
  folders, real multi-root (search, commands, and state per root), project
  files `.lux-project` with folders, exclusions, and settings." Only this
  bullet was in scope — not the file tree explorer, native watchers, find
  in files, or the incremental index, which are deliverables 2-6 of the
  same phase and untouched here. `docs/STATUS.md`'s "Multi-root workspaces"
  row stays `not-started` (rule 1): this is the headless data model later
  deliverables will key their per-root state off, not a user-visible
  capability by itself — no explorer, no watcher, no search exists yet to
  actually use it, and nothing is wired into the `lux` binary. Same pattern
  as Phase 2's `lux-render` context/surface scope note.

  **What "search, commands, and state per root" means at this layer.**
  Actual search (Find in Files) and command execution don't exist yet —
  building fake versions of either would violate rule 1 ("nothing
  simulated"). What this deliverable owns is the primitive those future
  subsystems need to scope themselves to one root: `RootId`, a stable
  identifier that survives removal of *other* roots (a plain incrementing
  counter, not a `Vec` index — removing root A must not silently change
  what root B's id means to a caller holding it), and
  `Workspace::root_for_path`, which resolves "which root does this file
  belong to" via longest-prefix match so nested roots resolve to the most
  specific one. Both are exercised directly
  (`root_ids_stay_stable_across_removal_of_other_roots`,
  `root_for_path_picks_the_most_specific_root_when_nested`) rather than
  left implicit.

  **`.lux-project` format: plain JSON, not JSONC — a real decision, written
  up as ADR 0015.** `docs/DEPENDENCIES.md` already earmarked `serde` +
  JSONC for `lux-config` (Phase 5) but explicitly deferred picking a JSONC
  crate to when that phase lands. Committing `.lux-project` to the same
  undecided JSONC crate now would mean either blocking on a Phase 5
  decision or half-deciding it here, out of scope. Plain JSON via
  `serde`/`serde_json` (1.0.229 / 1.0.151) sidesteps that: real dependency,
  not hand-rolled, and if Phase 5 lands a JSONC parser later,
  `ProjectFile`'s derive-based (de)serialization makes swapping it a parser
  change, not a schema change. First non-domain-specific dependency added
  to `lux-workspace`; both crates are MIT/Apache-2.0, already-allowed in
  `deny.toml`, so `cargo deny check` needed no new entries. Recorded in
  `docs/DEPENDENCIES.md` and ADR 0015.

  **`settings` is an untyped JSON object, deliberately.** The actual
  settings schema (what keys exist, their types, precedence) is
  `lux-config`'s job in Phase 5, not this crate's. `.lux-project` just
  needs to carry whatever settings a project defines through to whoever
  reads them later — a `serde_json::Map<String, Value>` round-trips
  arbitrary JSON without this crate inventing a schema it has no business
  owning.

  **`excludes` are stored as opaque glob strings, not matched.** Real glob
  matching against `.gitignore` semantics is Find in Files' job
  (deliverable 4, the `ignore` crate) — implementing even a partial matcher
  here would be exactly the kind of scope creep rule 7 forbids, and a
  half-correct matcher would be worse than none (it would look like
  exclusion works when it silently doesn't cover some case). This
  deliverable only guarantees the strings survive a save/load round trip.

  **`add_root` is idempotent by path, not by canonicalized path.** Adding
  the same `PathBuf` twice returns the existing root's id instead of
  creating a duplicate (`add_root_is_idempotent_for_the_same_path`).
  Canonicalizing (resolving symlinks, `..`, case) was considered and
  dropped: it requires the path to exist on disk, which would make
  `Workspace` — a pure in-memory structure everywhere else in this module —
  suddenly fallible and IO-dependent for what's a straightforward "don't
  duplicate what the caller already added" guard. Real deduplication
  against symlink aliasing is a real gap, but not one this deliverable's
  text asks for; flagged here rather than silently patched over.

  **`Workspace::discover` is the single entry point tying folders and
  `.lux-project` together.** The deliverable's first clause ("project
  folders") and its third ("`.lux-project` files") read as two sides of
  the same flow, not two unrelated features: opening a folder checks for a
  `.lux-project` at its root (like `.editorconfig`/`.gitignore`) and, if
  present, treats its `folders` as the real multi-root set (paths resolved
  relative to the project file's directory; absolute paths pass through
  unchanged); if absent, the folder itself becomes a single-root workspace.
  That's the only real IO in the module beyond `ProjectFile::load`/`save`
  themselves.

  **Error handling follows `lux-core::buffer::Buffer`'s existing
  convention instead of introducing a new error type.** `ProjectFile::load`
  wraps `serde_json::Error` into `io::Error::new(InvalidData, e)` — the
  same pattern `Buffer::open` already uses for `DecodeError`. Adding a
  dedicated `ProjectFileError` enum was the first instinct but dropped:
  `io::Result` is what every other file-facing API in this codebase
  returns, and the error content (a `Display`-able source with a `Debug`
  impl) doesn't need a bespoke type to be inspectable.

  **Actual verification.** `cargo test -p lux-workspace project::`: 21/21
  passed, 0 ignored, 0 failed on the first run — no iteration needed to
  reach green (empty crate, no existing behavior to break). Coverage:
  empty workspace, single-root `open_folder` with derived name, add/remove
  idempotency and stability of ids across removal, `root_for_path`
  (contained, outside, and nested-most-specific cases), `ProjectFile`
  JSON round-trip (including a relative `../` folder path and a settings
  value), missing-field defaults (`excludes`/`settings` absent from JSON),
  a real missing-file `NotFound` error, a real invalid-JSON
  `InvalidData` error, a real save-then-load round trip through the
  filesystem, `Workspace::from_project_file`'s relative-vs-absolute folder
  resolution, `to_project_file`'s round trip back through
  `from_project_file`, and `discover`'s both branches (project file
  present vs. absent) against real temp directories. Full repo gate also
  run and green: `cargo fmt --all --check`; `cargo clippy --workspace
  --all-targets -- -D warnings` (zero warnings, including on the new
  `serde`/`serde_json` dependency); `cargo test --workspace` (all crates,
  no regressions — `lux-core`'s 235 still pass); `cargo deny check`
  (advisories/bans/licenses/sources all `ok`, no new `deny.toml` entries
  needed); `scripts/check-limits.sh` (`project.rs` is under the 600-line
  cap; no struct exceeds 4 fields, well under the 25-field limit).
  `scripts/check-core-isolation.sh` doesn't apply here (it only checks
  `lux-core`). Verified on macOS (aarch64-apple-darwin) only — same
  documented limitation as the rest of `docs/STATUS.md` (no non-macOS dev
  environment available yet).

  **What's out of scope and untouched:** the file tree explorer, native
  `notify`-based watchers, Find in Files, replace-in-project, the
  incremental index, sessions/recovery, and huge/binary file handling
  (Phase 4 deliverables 2-8); wiring any of this into the `lux` binary or
  any UI; glob-matching `excludes` against real files; symlink-aware root
  deduplication.
