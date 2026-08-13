# Receipt: Autocompletado con resolve (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: The "Autocompletado con resolve" deliverable of Phase 7 ("Cliente
  LSP real") from `LUX_PROMPT_MAESTRO.md` is complete and its gate passes
  with no failures and no ignored tests.
- Verify: `cargo test -p lux-lsp completion::` (unit tests plus real
  `rust-analyzer` end-to-end tests)
- Iterations: 5 / 10
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** Adds `lux-lsp::completion` (`Completion`, `CompletionError`)
  on top of the prior three Phase 7 deliverables. The master prompt's ask
  is "autocompletado con resolve, documentación, ordenación, snippets del
  servidor y filtrado incremental sin latencia perceptible" — this loop
  covers `textDocument/completion` + `completionItem/resolve`,
  server-mandated sort order, faithful snippet-item passthrough (not
  re-implementing snippet expansion, which is already Phase 1's
  `lux-core` job), and a staleness-rejection mechanism for the
  "sin latencia perceptible" requirement. No `lux-ui` wiring (popup,
  keystroke triggering) — same headless-library scope every prior Phase 7
  receipt has followed. `docs/STATUS.md`'s "Autocomplete" row stays
  `not-started`.

  **Sorting is spec-mandated, not a nicety.** The LSP spec makes
  `CompletionItem.sortText` (falling back to `label` when absent) the
  server's way of asking the *client* to order the list — servers commonly
  return items in a semantically meaningful but not lexically sorted
  order and rely on the client for final ordering. `sort_items` is a free
  function so it has pure unit tests independent of any server (three
  cases: `sortText` present, absent, and mixed within one list), then
  proven wired into the real request path by a real-server test asserting
  the returned list — around 200 items for `Vec<i32>`'s inherent and
  Deref-borrowed slice methods — is already in that exact order end to
  end.

  **`resolve` needed a `session.rs` capability-declaration fix, found by
  inspecting the real payload instead of guessing.** The first attempt at
  the resolve test failed: `caps.completion_provider.resolve_provider` was
  `Some(false)`. Rather than assume rust-analyzer simply doesn't support
  resolve, printed the full `CompletionOptions` capability rust-analyzer
  actually returned to confirm the exact value, which pointed at the real
  cause: rust-analyzer only advertises `resolveProvider` when the *client*
  first declares
  `textDocument.completion.completionItem.resolveSupport` (which
  properties it wants lazily resolved). Added that (`documentation`,
  `detail`) plus `snippetSupport: true` to `session.rs`'s `initialize`
  handshake — a small, necessary extension to shared infrastructure, the
  same category of change the sync and diagnostics deliverables' loops
  each needed once. Confirmed via the same capability inspection:
  `resolveProvider` came back `Some(true)` afterward.

  **Filtrado incremental sin latencia perceptible → staleness rejection,
  not artificial delay.** async-lsp gives no way for a *client* to cancel
  its own outgoing request (`ConcurrencyLayer`'s `$/cancelRequest` handling
  is for incoming requests the crate is asked to fulfill, the opposite
  direction) — confirmed by reading `concurrency.rs` before designing
  around it rather than assuming the capability existed. The practical
  fix real editors also apply regardless of protocol-level cancellation:
  each `complete()` call tags itself with a monotonically increasing
  generation counter (`AtomicU64`); if the counter has moved on by the
  time a response arrives, that response is discarded
  (`CompletionError::Superseded`) instead of returned. A fast typist's
  screen can never show an older keystroke's results flashing in after a
  newer one's, independent of real network reordering.

  **What got stuck, in order, and the actual fix (this was the whole
  story — one clean chase per issue, no repeats):**
  1. `resolveProvider` came back `false` — root-caused (not guessed at
     retries) by printing the real capability payload, fixed by declaring
     `resolveSupport` client-side, described above.
  2. Real-server label assertions (`i.label == "push"`) failed once
     capabilities were fixed: rust-analyzer's actual labels include a
     signature suffix (`"push(…)"`), not the bare method name. Fixed the
     three assertions to `starts_with("push(")`.
  3. With labels fixed, the *first* real-server test still failed: the
     completion list that came back was postfix-snippet names only
     (`ref`, `deref`, `dbg`, ...), no `push` — the very first non-empty
     response can arrive before rust-analyzer's type-aware member
     completions are ready, and the retry helper was accepting any
     non-empty list. Fixed by changing the retry helper to take a
     predicate (`complete_until`, checking for `push(` specifically)
     instead of a bare "non-empty" check, applied to all three
     real-server tests including the staleness-race test's warm-up call.
  4. The staleness-race test itself first failed the other way: the
     "stale" spawned request came back `Ok(...)` instead of
     `Superseded`. Root cause: a `sleep(50ms)` between spawning the stale
     request and firing the fresh one gave the stale request's full real
     round trip (fast once rust-analyzer is warmed up from the earlier
     calls in the same test) enough wall-clock time to complete
     *entirely*, including its own generation check, before the fresh
     call had even started — so of course it wasn't stale yet. Replaced
     the sleep with a single `tokio::task::yield_now().await`: enough to
     let the spawned task run its synchronous prefix (bump the
     generation, dispatch the request) and suspend at the real
     network-response await point, but not remotely enough wall-clock
     time for a genuine cross-process round trip to finish — the
     precise middle ground a fixed sleep duration can't reliably hit
     either way.

  **Verified on macOS (aarch64-apple-darwin)**: `cargo test -p lux-lsp
  completion::` — 6 tests: 3 pure unit tests (sort-by-`sortText`,
  label-fallback, and a case exercising both within one list, no process
  spawned) and 3 tests against a real `rust-analyzer`: completions listed
  and confirmed already in server-sort order end to end, `resolve`
  confirmed to fill in real, non-empty documentation text for
  `Vec::push` (not just "the call didn't error"), and the
  staleness-rejection race, proven with an actually in-flight spawned
  request that a fresher direct request supersedes. Re-ran 4 times after
  the final fix to specifically rule out timing flakiness in the race
  test — 0 failures, ~3.4s each run. Also verified: the full crate suite
  (`cargo test -p lux-lsp`, 34 tests — the prior three deliverables' gates
  plus this one, all still green; the `session.rs` client-capabilities
  addition didn't regress any of them), `cargo clippy -p lux-lsp
  --all-targets -- -D warnings` (clean), `cargo fmt --check -p lux-lsp`
  (clean), `cargo check --workspace` (no regressions elsewhere),
  `./scripts/check-limits.sh` (clean — largest touched file is
  `session.rs` at 428 lines).
