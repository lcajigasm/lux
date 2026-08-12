# Receipt: Diagnósticos + panel de problemas (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: The "Diagnósticos + panel de problemas" deliverable of Phase 7
  ("Cliente LSP real") from `LUX_PROMPT_MAESTRO.md` is complete and its
  gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-lsp diagnostics::` (unit tests plus a real
  `rust-analyzer` end-to-end test)
- Iterations: 5 / 8
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** Adds `lux-lsp::diagnostics` (`Diagnostics`,
  `DiagnosticsSummary`) on top of the server-management and document-sync
  deliverables. The master prompt's ask for this deliverable is
  "`publishDiagnostics` continuo, con panel de problemas filtrable,
  subrayado en el editor y navegación entre diagnósticos" — this loop
  builds the headless, testable half (continuous reception, filtering,
  summary counts, and diagnostic-to-diagnostic navigation); the panel UI
  and editor gutter underline are `lux-ui` work, out of scope by the same
  headless-library convention every prior Phase 7 receipt has followed.
  `docs/STATUS.md`'s "Diagnostics panel" row stays `not-started`.

  **`session.rs` needed one more small, necessary extension.** Its
  `Router` previously routed every unregistered notification — including
  `textDocument/publishDiagnostics` — to a no-op catch-all, so diagnostics
  were silently discarded before this loop; there was no way to build a
  problems view without fixing that first. Added a third
  `watch::Sender<Arc<BTreeMap<Url, Vec<Diagnostic>>>>`/`Receiver` pair
  (same shape as the socket and state channels from the two prior
  deliverables), a `PublishDiagnostics` handler registered on the Router,
  and `pub(crate) fn diagnostics()`/`watch_diagnostics()` accessors. Kept
  deliberately separate from `ServerState` rather than folded into it:
  diagnostics can be large and change often, and `ServerState` gets cloned
  on every unrelated status/progress check elsewhere in the crate — an
  `Arc`-wrapped map updated via its own channel keeps those clones cheap
  regardless of how much diagnostic data is live. `publishDiagnostics`
  always replaces a document's *full* diagnostic list per the spec, so the
  handler does exactly that (never merges deltas), and drops empty entries
  rather than keeping a stale empty one around. Diagnostics are cleared to
  empty on crash and on stop, since they're unknown until the server (or
  the sync deliverable's reconnect replay) republishes them for real.

  **Query logic is pure and separately proven from reception.** `filtered`,
  `summary`, and `next_after`/`previous_before` are free functions over
  `&BTreeMap<Url, Vec<Diagnostic>>` (not methods reaching into a live
  session) for the same reason `sync.rs`'s capability-negotiation logic
  is: fast, no-process unit tests against hand-built diagnostic data,
  covering the sorting order, the severity-filter boundary (including a
  diagnostic with no `severity` set at all, which the LSP spec leaves up
  to client interpretation — deliberately never hidden by a filter, since
  silently dropping an unmarked diagnostic would be worse than
  over-showing one), summary counts per severity, and navigation
  wraparound at both ends of the list. `Diagnostics`'s public methods are
  thin wrappers calling these with the session's current diagnostic map.

  **What got stuck, and what the fix actually was (this was the whole
  story — no repeated failures, one clean root-cause chase):** the first
  real-server test used `does_not_exist_fn()` (an unresolved function
  call) as the deliberate error, expecting rust-analyzer to flag it.
  Instrumented `session.rs` to log every `publishDiagnostics` notification
  (uri + count) and confirmed one arrived — with `count=0`. Tried an
  unresolved `use` import next (`unresolved-import` is one of
  rust-analyzer's best-known diagnostics) — same result, `count=0` both
  times, no follow-up event either. Concluded rust-analyzer's diagnostics
  in this configuration don't include those two specific checks (likely
  gated behind `cargo check`/flycheck or experimental settings this
  minimal fixture/session doesn't enable) rather than continuing to guess
  at more unresolved-name variants. Switched to a plain **syntax error**
  (`pub fn broken( -> i32 { 1 }` — a stray `(`), which is reported
  straight from parsing, unconditionally, with no dependency on semantic
  analysis, crate-graph loading, or flycheck being enabled — the same log
  instrumentation immediately showed `count=0` (before the edit) →
  `count=7` (the syntax error, which cascades into several parser
  diagnostics) → `count=0` (after the fix), on the first try. Debug
  instrumentation (the `eprintln!` and inherited `stderr`) was removed
  once confirmed.

  **Verified on macOS (aarch64-apple-darwin)**: `cargo test -p lux-lsp
  diagnostics::` — 8 tests: 7 pure unit tests (flattening/sort order,
  severity filtering with the unmarked-severity case, per-severity
  summary counts, next/previous navigation with wraparound, empty-map
  navigation returning `None`) and one test against a real
  `rust-analyzer` (open with a syntax error → real `ERROR`-severity
  diagnostic observed via both the raw watch channel and the
  `Diagnostics` query methods → incremental fix via `didChange` → the
  diagnostic list confirmed empty again, proving continuous updates, not
  a one-shot `didOpen` snapshot). Re-ran 3 times after formatting to rule
  out timing flakiness — 0 failures, ~2-3s each run. Also verified: the
  full crate suite (`cargo test -p lux-lsp`, 28 tests: the prior two
  deliverables' gates plus this one, all still green — the `session.rs`
  diagnostics-channel addition didn't regress either),
  `cargo clippy -p lux-lsp --all-targets -- -D warnings` (clean),
  `cargo fmt --check -p lux-lsp` (clean), `cargo check --workspace` (no
  regressions elsewhere), `./scripts/check-limits.sh` (clean — largest
  touched file is `sync.rs` at 422 lines, largest struct still
  `ServerState` at 6 fields).
