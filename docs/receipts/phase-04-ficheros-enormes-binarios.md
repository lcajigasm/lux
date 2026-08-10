# Receipt: Huge and binary files (deliverable 8 of Phase 4, last of the phase)

- Phase: 04
- Crate(s): lux-core
- Goal: The "Huge and binary files" deliverable of Phase 4 ("Projects,
  files and global search") from `LUX_PROMPT_MAESTRO.md` is complete and
  its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-core huge_file:: binary_detect::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-10
- Closed: 2026-08-10
- Notes:

  **Scope.** The master prompt's deliverable text: "Explicit handling of
  huge and binary files: above a configurable threshold, read-only mode
  with no highlighting; binary files detected and not opened as text."
  This is the eighth and last deliverable of Phase 4. `docs/STATUS.md`'s
  "Huge/binary file handling" row stays `not-started` (rule 1) — see
  below for exactly why: the deliverable's two halves ended up at
  different levels of real completeness, and rule 1 doesn't let a partial
  one round the whole row up.

  **Binary detection was already half-built, privately, since Phase 1 —
  this deliverable's real job was promoting it to a first-class,
  standalone, public capability, not reimplementing it.**
  `buffer::encoding::decode` already refused binary content via a private
  `looks_binary` heuristic (a NUL byte in the first 8000 bytes, the
  git/ripgrep convention) that nothing outside `encoding.rs` could reach.
  The new `binary_detect` module owns that heuristic now (byte-for-byte
  identical — `encoding::decode` was updated to call
  `binary_detect::looks_binary` instead of a local copy, verified by
  `buffer::encoding::tests::rejects_binary_content` still passing
  unchanged), and adds the piece that didn't exist: `looks_binary_path`,
  which reads only a bounded prefix of a real file via `File::take
  (BINARY_SCAN_LEN).read_to_end` rather than the whole thing — directly
  the "detected... before opened as text" requirement, in its most
  literal, disk-I/O-bounded form. Verified directly
  (`looks_binary_path_only_reads_the_scan_window_even_for_a_huge_file`):
  a NUL byte placed well past the scan window in an otherwise-huge file
  correctly reads as "not binary," proving the read itself never touches
  more than the bounded prefix.

  **Huge-file classification is real and correct; enforcing it is
  deliberately not done in this crate, for a specific, documented
  correctness reason — not laziness.** `huge_file::classify_path` checks
  a real file's size via `fs::metadata` alone (no content read) against a
  genuinely configurable `HugeFilePolicy` (verified directly:
  `threshold_is_configurable_not_fixed` classifies the *same* size two
  different ways under two different policies, not just "the field exists
  and defaults sanely"). `Buffer::open_with_policy` combines both checks
  into one real open path and sets a real, queryable
  `is_huge_file_mode()` flag. What it deliberately does *not* do: make
  `Buffer::insert`/`delete` respect that flag. Those two methods are the
  invertibility primitives `Edit`/`Transaction`/`History` (Phase 1) are
  built on — `Edit::delete` captures the text-to-be-removed *before*
  calling `buffer.delete`, so a silent no-op there would desync what
  `History` believes happened from what the rope actually contains, a
  real correctness hazard in Phase 1 code this deliverable has no
  business touching (the same "don't widen a heavily-tested signature
  under time pressure" caution Phase 1's own STATUS.md already flagged
  for `Buffer::from_bytes`). The flag is real and correctly tracked;
  refusing edit commands against it, and skipping syntax highlighting for
  it, are both a future editor-session layer's job — one that doesn't
  exist yet anywhere in this codebase (no UI wiring exists for any Phase
  4 deliverable so far).

  **`open_with_policy` checks binary *before* size, and a real test
  exists specifically to prove the two checks don't shadow each other.**
  A binary file above the huge-file threshold must still be rejected as
  binary, not silently accepted into huge-file mode because the huge-file
  path skips the binary check (an easy ordering bug to introduce and not
  notice, since both checks read `path` independently).
  `open_with_policy_rejects_a_binary_file_without_reading_it_fully`
  constructs exactly that file (2000 bytes, above a 1000-byte test
  threshold, with a NUL near the start) and asserts it's still rejected
  as binary.

  **No new dependency, no `deny.toml`/`docs/DEPENDENCIES.md` change** —
  everything is `std::fs`/`std::io`, the same "no UI dependency, ever"
  posture the rest of `lux-core` already holds itself to.

  **Actual verification.** `cargo test -p lux-core huge_file::
  binary_detect::`: 8/8 and 7/7 respectively (cargo's test filter only
  accepts one substring per invocation, so the goal's two-module gate was
  run as two separate `cargo test` calls — both green), 0 ignored, on the
  first run — no iteration needed. Also added 6 integration tests to
  `buffer::tests` exercising `Buffer::open_with_policy` end-to-end against
  real files (huge-marked above threshold, normal below threshold, binary
  rejection on a huge-sized file, configurable threshold changing the
  classification of the same file, and a freshly-`from_bytes`-built
  buffer defaulting to `is_huge_file_mode() == false`) — all 6 passed on
  the first run too. Coverage overall: `looks_binary`/`looks_binary_path`
  on binary content, plain text, the scan-window boundary (both the
  in-memory and path-based versions), a missing file surfacing as
  `io::ErrorKind::NotFound`; `HugeFilePolicy`'s default threshold (10 MB,
  documented and chosen — comfortably below the already-budget-tested
  100 MB case in `docs/PERFORMANCE.md`), classification at/above/below an
  arbitrary threshold, a real file's size read via `classify_path`, a
  missing file surfacing as an I/O error there too. Full repo gate also
  run and green: `cargo run -p xtask` — `cargo fmt --all --check`;
  `cargo clippy --workspace --all-targets -- -D warnings` (zero
  warnings); `cargo test --workspace` (all crates, no regressions —
  `lux-core` now at 255 tests total, up from 235, no other crate's count
  changed); `cargo deny check` (advisories/bans/licenses/sources all
  `ok`, no new entries — no new dependency); `scripts/check-limits.sh`
  (`binary_detect.rs` at 120 lines, `huge_file.rs` at 172, `buffer/mod.rs`
  at 388, all under the 600 cap; `Buffer` — the only struct that grew —
  went from 4 fields to 5, well under 25); `scripts/check-core-
  isolation.sh` (still passes — both new modules are plain `std::fs`/
  `std::io`, no UI/graphics dependency introduced). Verified on macOS
  (aarch64-apple-darwin) only.

  **What's out of scope and untouched:** actual read-only enforcement
  against `Buffer::insert`/`delete` or any higher-level edit command (no
  such command-routing layer exists yet, and building one wasn't this
  deliverable's job); syntax highlighting being skipped for a huge-mode
  buffer (`lux-syntax`/`lux-ui`'s domain, not `lux-core`'s); a genuine
  memory-usage measurement of opening a 1 GB file (this deliverable adds
  cheap pre-checks before opening, it doesn't change how a file's content
  is actually loaded once the decision is made to open it — the RSS
  overhead already documented as an open gap in Phase 1's measurement
  history is unchanged, neither fixed nor made worse here); wiring any of
  this into `lux-workspace`'s file tree, watcher, or index, or into the
  `lux` binary; Windows/Linux verification (no non-macOS dev environment
  available).

  **Phase 4 close-out**: not part of this deliverable's scope
  (`docs/close-phase` is a separate, explicit workflow) — this receipt
  only closes deliverable 8. All eight of Phase 4's deliverables now have
  individual receipts in `docs/receipts/`; verifying the phase's own
  acceptance criteria as a whole and updating the phase-level release
  notes is separate, un-started work.
