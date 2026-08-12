# Receipt: Historial, blame en línea y por commit (Phase 6)

- Phase: 06
- Crate(s): lux-git
- Goal: The "Historial, blame en línea y por commit" deliverable of Phase
  6 ("Git integrado con `gix`") from `LUX_PROMPT_MAESTRO.md` is complete
  and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-git history:: blame::` (over temporary
  repositories; run as two separate `cargo test` invocations, since `cargo
  test` only accepts one `TESTNAME` filter at a time — `cargo test -p
  lux-git history::` then `cargo test -p lux-git blame::`)
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-11
- Closed: 2026-08-11
- Notes:

  **Scope.** Two new modules, matching the gate's two test namespaces and
  the deliverable's own two halves: `history` (repository-wide and
  per-file commit history, the master prompt's "historial de repositorio
  y por fichero") and `blame` (per-line and per-commit blame, "blame en
  línea y por commit"). `docs/STATUS.md`'s "History & blame" row stays
  `not-started`, the same convention as every scope-noted row in this
  document: real, tested read operations, nothing wired into `lux-ui` or
  the running `lux` binary yet.

  **The headline finding: `gix` already has a real blame algorithm —
  ADR 0010's directive to check before assuming a gap paid off
  immediately.** Before writing anything, `gix`'s Cargo.toml was checked
  for a `blame` feature (`gix-0.86.0`'s feature list includes `blame =
  ["dep:gix-blame", "blob-diff"]`), and it's a genuine, first-party
  implementation — `Repository::blame_file()`, backed by the `gix-blame`
  crate, doing a real history walk with diffing at each step, not a
  stub. Its output (`gix_blame::Outcome.entries: Vec<BlameEntry>`) is
  *already* grouped into hunks by the commit that introduced each run of
  lines — meaning "blame por commit" is what `gix` hands back natively,
  and "blame en línea" (`blame::expand_to_lines`) is just a pure,
  repository-free flattening of that into one entry per line. No line-
  attribution algorithm was written in this crate; the actual work is a
  thin wrapper (`blame_by_commit`) around `Repository::blame_file()`.
  This is the ADR working as intended: it explicitly warns against
  assuming `gix` doesn't cover an operation without checking, and
  checking here turned a feature that could have eaten most of the
  iteration budget into two functions.

  **`history::repository_history` uses `gix`'s real revision walk**
  (`Repository::rev_walk`), sorted `ByCommitTime(CommitTimeOrder::
  NewestFirst)` — not a hand-rolled "follow `.parents[0]`" loop, which
  would silently miss the non-first-parent side of merge commits.

  **`history::file_history` is a deliberately narrower algorithm than
  `git log`'s full path-limited history, and the receipt says so plainly
  rather than overclaiming parity.** For each commit the revision walk
  visits, its blob at the target path is compared against every parent's
  blob at that same path; the commit is included only if it differs from
  *all* of them. This matches `git log <path>`'s common-case behavior
  (hiding a merge commit that's identical to at least one parent) without
  implementing `git`'s full history-simplification machinery (which also
  handles more exotic cases involving rewritten merge topology). Revisit
  with the full algorithm only if a concrete gap surfaces — this is
  exactly the kind of scope call ADR 0010 asks to be explicit about
  instead of quietly asserting full compatibility.

  **No struggle worth narrating.** Both `history::` and `blame::` passed
  their full test suites on the first real attempt — the only fixes
  needed were two compile errors caught immediately by `cargo check`, not
  logic bugs found by failing tests:
  1. `gix::revision::walk::CommitTimeOrder` doesn't exist at that path —
     the type actually lives at `gix::traverse::commit::simple::
     CommitTimeOrder` (`gix::revision::walk::Sorting::ByCommitTime` takes
     it as a parameter but doesn't re-export the type itself). One-line
     import fix, found by reading the compiler's "private item" error
     pointing at the real definition.
  2. `gix_actor::SignatureRef::time` is a raw `&str` field (unparsed, to
     keep round-tripping lossless per its own doc comment), not the
     parsed `gix_date::Time` — the parsed value comes from calling
     `.time()`, a method with the same name as the field. Caught by a
     type-mismatch error naming the exact wrong type; fixed by calling
     `.time()` instead of accessing `.time` as a field.

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-git
  history::` — 4 tests, 0 failures, 0 ignored (repository history lists
  every commit newest-first with correct parent links, `CommitInfo`
  correctly decodes author/email and splits a multi-line message into
  summary + full text, file history includes only commits that touched
  the path and excludes an unrelated one, file history of a path that
  never existed is empty rather than an error). `cargo test -p lux-git
  blame::` — 4 tests, 0 failures, 0 ignored (a single-commit file blames
  entirely to that commit at both the hunk and expanded-line level, lines
  added in a second commit attribute to it while the untouched lines keep
  the first commit, an edited middle line attributes to the edit while
  its unchanged neighbors don't, blaming an earlier revision by its
  commit id ignores all later history). All against real temporary
  repositories built with `gix`'s own object-writing API (reusing
  `staging::stage` and `commit::commit` from earlier Phase 6 loops to set
  up fixtures) — no `git` binary anywhere. Also clean: `cargo clippy -p
  lux-git --all-targets -- -D warnings`, `cargo fmt --check -p lux-git`,
  `./scripts/check-limits.sh` (`history.rs` 251 lines, `blame.rs` 222
  lines, both well under the 600-line cap), `cargo test -p lux-git` (all
  42 tests across `gutter`, `diff_panel`, `staging`, `commit`, `history`,
  and `blame` still pass together), and `cargo build --workspace`.
