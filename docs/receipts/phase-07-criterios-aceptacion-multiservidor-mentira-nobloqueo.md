# Receipt: Criterios de aceptación restantes -- 5 servidores, servidor de mentira, no bloqueo del hilo (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: Phase 7's acceptance criteria beyond its nine numbered
  deliverables and the fuzzer -- "funciona con al menos cinco servidores
  reales," "tests de integración contra un servidor de mentira que
  ejerza el protocolo," and "ninguna operación LSP bloquea el hilo de UI"
  -- are addressed and their gates pass with no failures.
- Verify: `cargo test -p lux-lsp --test multi_server --test
  fake_server_protocol --test no_thread_blocking`
- Iterations: 6 / N
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** `/close-phase 7`'s own audit (before this loop) found that
  the nine numbered deliverables' receipts all passed, but every one of
  them was verified against `rust-analyzer` only -- the phase's separate
  "Criterios de aceptación" paragraph asks for more: five real servers,
  a fake-server protocol test suite (distinct from the fuzzer, which
  tests malformed *bytes*, not protocol-level behavior), and evidence
  that LSP operations don't block a UI thread. This receipt covers that
  remaining gap. No `lux-ui` wiring -- still out of scope for all of
  Phase 7, same as every other receipt.

  **Five real servers
  (`crates/lux-lsp/tests/multi_server.rs`).** Same general sequence per
  language -- real spawn, real capability handshake, real `didOpen`,
  a real `documentSymbol` request -- against a small on-disk fixture in
  each of the five required languages. `documentSymbol`, not hover, is
  the shared probe: it needs no pinpoint cursor position, which keeps
  five differently-shaped fixtures honest without five sets of fragile
  position math. Required installing the four servers this machine
  didn't already have: `brew install gopls go` (gopls needs a real Go
  toolchain for meaningful analysis, not just its own binary),
  `npm install -g typescript-language-server pyright` (`clangd` ships
  with Xcode's command line tools, already present).

  **Two real client bugs found and fixed while wiring this up, both in
  `session.rs`'s `initialize` handshake:**
  1. `typescript-language-server` 5.3.0 failed every handshake with
     "Could not find a valid TypeScript installation," even against a
     workspace with a real, local `typescript` install. Root cause,
     found by manually driving the server's raw stdio with a throwaway
     script to isolate the client's exact contribution: this client only
     ever sent `workspace_folders`, never the (LSP-spec-deprecated, but
     still practically required by this specific server) `root_uri`.
     `workspace_folders` alone wasn't enough for it to resolve its
     `typescript` dependency. **Fix**: `session.rs` now sends `root_uri`
     alongside `workspace_folders` (`#[allow(deprecated)]`, with a
     comment explaining why it's still sent) -- verified by re-running
     the same manual probe with only `root_uri` added and getting a real
     `window/logMessage` confirming the server found its TypeScript
     install.
  2. The fixture's own `npm install typescript` initially resolved
     TypeScript's new native (Go-ported) `7.x` package, which has no
     `lib/tsserver.js` at all -- `typescript-language-server` can't use
     it, and fails with the *same* "Could not find a valid TypeScript
     installation" message, which reads exactly like a missing
     dependency rather than a version incompatibility. This is a test-
     fixture concern, not a client bug: pinned the fixture to
     `typescript@5`.

  **Fake-server protocol tests
  (`crates/lux-lsp/tests/fake_server_protocol.rs`, driving a new
  `lux-lsp-fake-server` test-only binary,
  `tests/support/fake_server_main.rs`).** A real subprocess speaking the
  real wire protocol (`async-lsp`'s own server-role `MainLoop`, real
  Content-Length framing over real stdio) with scripted, deterministic
  *responses* -- the "servidor de mentira que ejerza el protocolo" the
  acceptance criteria names as its own thing, separate from the fuzzer
  (which throws malformed bytes at the parser; this exercises correct
  protocol-level *behavior*, deterministically, in ways a real server
  can't be coerced into on demand). Configured via sentinel files in the
  workspace root rather than environment variables, deliberately: env
  vars are process-wide and `cargo test` runs this crate's tests
  concurrently across threads, so a shared env var would race between
  tests; a sentinel file lives in each test's own, private temp
  directory. Three tests, each proving something a real server can't:
  - `capability_handshake_receives_exactly_what_the_client_sent`: the
    fake server writes the `InitializeParams` it actually received to a
    file; the test asserts `root_uri` and `workspace_folders` match the
    session's real root exactly -- a real server has no way to report
    back what it was sent, so this is the only way to verify the
    client's own handshake payload directly rather than inferring it
    from whether a server happens to accept it.
  - `a_scripted_progress_sequence_is_tracked_exactly`: the fake server
    sends exactly one begin/report/end `$/progress` sequence; the test
    asserts `progress_events_seen == 3` exactly and that the completed
    entry is removed -- a real server's own progress event count varies
    run to run, so this is the only way to pin the exact count down.
  - `a_server_that_exits_on_its_own_triggers_a_restart_too`: the fake
    server exits right after `initialized`, not killed by any signal --
    a different code path than deliverable 1's existing `SIGKILL`-based
    crash test. The exit marker file is consumed (deleted) before the
    process exits, so the *restarted* instance doesn't see it and stays
    up -- the test confirms exactly one restart, not a crash loop.

  Needed one small, deliberate API change to build this at all:
  `ServerSession::spawn` was `pub(crate)`; promoted to `pub` (`ServerSpec`
  was already public) so a caller -- this test, or in principle a real
  editor wanting a language server outside `KNOWN_SERVERS` -- can spawn a
  session directly, bypassing `ServerRegistry`'s fixed, known-language
  lookup. `ServerRegistry::get_or_start` is now a convenience layer built
  on this primitive, not the only way in. Also needed enabling
  `async-lsp`'s `tokio` feature (for `PipeStdin`/`PipeStdout`'s
  non-blocking tokio-backed variant) purely for the fake server binary's
  own stdio -- production code (`session.rs`) is unaffected, since it
  talks to a real child process through `async-process`'s own
  `AsyncRead`/`AsyncWrite` impls either way.

  **No-thread-blocking proxy verification
  (`crates/lux-lsp/tests/no_thread_blocking.rs`).** The literal
  criterion -- "verificado con el monitor de frame time" -- needs a real
  running UI to measure, which doesn't exist yet (no `lux-ui` wiring, the
  same gap every Phase 7 receipt has documented). What *is* verifiable
  honestly from this crate: the mechanism golden rule 6 ("no I/O on the
  UI thread, ever") and this criterion both depend on -- that a slow,
  in-flight LSP request never blocks other concurrent async work sharing
  its executor thread. If that didn't hold, no amount of future `lux-ui`
  wiring could fix it; the block would be in this crate. Test: on a
  `current_thread` tokio runtime (the same shape a single UI thread
  driving both rendering and LSP calls would have -- a multi-thread
  runtime would hide a real bug by just running things on a different OS
  thread), issues a real hover request against the fake server
  configured (via the same sentinel-file mechanism) to delay its response
  by 1 second, concurrently with a tight tokio task ticking every 5ms and
  recording the largest gap between ticks. Asserts that gap stays under
  200ms (a generous margin over the 5ms nominal budget) for the whole
  duration of the slow, in-flight request. This proves the premise, not
  the full end-to-end criterion -- documented as exactly that, not
  claimed as more.

  **Verified on macOS (aarch64-apple-darwin):** `cargo test -p lux-lsp
  --test multi_server` (5 tests, one per required server, all real
  spawns/handshakes/`documentSymbol` requests), `cargo test -p lux-lsp
  --test fake_server_protocol` (3 tests), `cargo test -p lux-lsp --test
  no_thread_blocking` (1 test) -- all green. Full crate suite re-run
  clean after the `session.rs` `root_uri` change and the `ServerSession::
  spawn` visibility change (`cargo test -p lux-lsp`, all targets, no
  regressions across every prior Phase 7 deliverable's own tests).
  `cargo clippy -p lux-lsp --all-targets -- -D warnings` clean, `cargo
  fmt --check -p lux-lsp` clean, `./scripts/check-limits.sh` clean.

  **What's still honestly out of scope, not silently skipped:** "un
  servidor que se cuelga o muere no arrastra al editor" is covered at
  the `lux-lsp` layer (both the pre-existing `SIGKILL` test and this
  loop's new self-exit test restart cleanly), but "el editor" as a full
  application doesn't exist yet with `lux-lsp` wired in, so the
  full-system claim can't be verified end-to-end -- same `lux-ui`/`lux`
  wiring gap as everything else in this phase.
