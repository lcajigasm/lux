# Receipt: Sincronización incremental de documentos (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: The "Sincronización incremental de documentos" deliverable of Phase
  7 ("Cliente LSP real") from `LUX_PROMPT_MAESTRO.md` is complete and its
  gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-lsp sync::` (unit tests plus real
  `rust-analyzer` end-to-end tests)
- Iterations: 6 / 10
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** Adds `lux-lsp::sync` (`DocumentSync`, `TextEdit`, `SyncError`)
  on top of the server-management deliverable's `ServerSession`/
  `ServerRegistry` — nothing else from Phase 7's remaining list
  (diagnostics, completion, navigation, hover/inlay hints/semantic tokens,
  code actions, cross-file rename, formatting) and no `lux-ui`/`lux`
  wiring. `docs/STATUS.md`'s "Incremental document sync" row stays
  `not-started`, same headless-library convention as every other scope
  note in that file.

  **`ServerSession` needed a small, necessary extension first.** Before
  this loop, the `ServerSocket` handle used to talk to a running server
  only ever existed inside `supervise()`'s local scope — there was no way
  for anything outside `session.rs` to send a single notification, so
  document sync was structurally impossible without touching it. Added a
  second `watch::Sender<Option<ServerSocket>>`/`Receiver` pair alongside
  the existing state channel: `Some(socket)` published right after the
  `Running` transition, cleared to `None` on crash/stop/failure. Exposed
  as `pub(crate) fn socket()`/`watch_socket()` — deliberately not `pub`,
  since `ServerSocket`/`async_lsp` types have no reason to leak into
  `lux-lsp`'s public API for this deliverable's scope.

  **Incremental vs. full sync is read from the real capability, never
  assumed.** `change_kind_for`/`save_wants_text_for` are free functions
  over `Option<&ServerCapabilities>` (not methods reaching into a live
  session), specifically so the negotiation logic has pure, no-process
  unit tests against hand-built capability values (bare `Kind`,
  `Options.change`, unspecified defaulting to `FULL` -- never guessing
  `INCREMENTAL` without the server asking for it). The real-server test
  separately confirms this against `rust-analyzer`'s own advertised
  capability, so the logic is proven both in isolation and end-to-end.

  **Position math is UTF-16-aware, not byte-offset.** `apply_edit`/
  `position_to_byte_offset` resolve LSP `Range`s as UTF-16 code units per
  line (the spec's default position encoding), converting to the correct
  UTF-8 byte boundary — a `héllo` test case with the edit starting right
  after `é` (2 UTF-8 bytes, 1 UTF-16 unit) catches the class of bug a
  naive byte-offset implementation would produce.

  **Crash restart (previous loop) and document sync interact, and that
  interaction is handled, not left to silently break.** Without
  intervention, a session that reconnects after a `SIGKILL`-triggered
  restart would leave every previously-open document unknown to the new
  server process — `didOpen` was only ever sent once, to the now-dead
  process. `DocumentSync::new` spawns a small task watching the session's
  socket transitions (event-driven, `tokio::sync::watch`, golden rule 5);
  on any reconnect it replays `didOpen` for every still-tracked document
  with its current in-memory text. Proven for real, not just asserted: the
  `reopens_documents_after_a_crash_restart` test edits a document only in
  memory (never written to disk), `SIGKILL`s the real process, and
  confirms via a fresh `hover` request that the *edited* content is what
  the restarted server sees — the on-disk file alone couldn't produce that
  answer, so this specifically catches a regression to "restart drops
  sync silently."

  **What got stuck, in order, and what each fix actually was (6 of the 10
  iterations were spent here):**
  1. First real-server run: `ERROR duplicate DidOpenTextDocument` in
     rust-analyzer's own log (visible after temporarily inheriting its
     `stderr`), then a 60s hover timeout. Root cause:
     `ServerSession::watch_socket()` clones `self.socket_rx`, but that
     original `Receiver` is never itself read by `ServerSession` -- so
     every fresh clone inherited a *stale* "last observed version"
     (the channel's construction-time value), making the clone's very
     first `changed()` call fire immediately for whatever the *current*
     value already was, misread as "just reconnected" even on the
     process's first-ever connect. Fixed with one `socket_rx
     .borrow_and_update()` right after subscribing, in
     `replay_on_reconnect`, to mark the current value as already-seen
     before entering the watch loop.
  2. After that fix, hover still timed out for 60s on every attempt
     (`Ok(None)`, never an error, never useful markup) — including on a
     *baseline* hover with no edit involved at all, which narrowed the
     bug away from incremental sync specifically. Root cause: the test's
     file `Url` was built from the tempdir's raw path
     (`/var/folders/...` on macOS), while `ServerRegistry::get_or_start`
     canonicalizes the session root before using it as the
     `initialize`d workspace URI (`/private/var/folders/...` — macOS's
     `/var` is a symlink to `/private/var`). The two URIs disagreed, so
     rust-analyzer never associated the opened document with a crate in
     its workspace and had nothing to hover. Fixed in the test helper
     (`lib_rs_uri`), not in `lux-lsp` itself — this is a caller
     obligation (build URIs consistently), not a library bug; confirmed
     by printing the real `Hover` payload before and after the fix
     (`"pub fn one() -> i32"` proved it once URIs matched).
  3. `sync.rs` crossed the 600-line limit once `cargo fmt` expanded the
     new struct-literal-heavy real-server tests. Split the real-server
     tests into `crates/lux-lsp/src/sync/real_server.rs` (a `#[cfg(test)]
     mod real_server;` submodule, not `tests/`, since it needs
     `pub(crate)` access to `ServerSession::socket()` for its `hover`
     verification oracle) — both files back under the limit
     (`sync.rs` 422 lines, `sync/real_server.rs` 283).
  Each of the above was a single root-cause fix confirmed with real
  `stderr`/response instrumentation before being applied, then the
  temporary instrumentation was removed — no repeated guesses at the same
  failure.

  **Verified on macOS (aarch64-apple-darwin)**: `cargo test -p lux-lsp
  sync::` — 11 tests: 8 pure unit tests (edit application including the
  UTF-16 case, sync/save capability negotiation against hand-built
  `ServerCapabilities`, zero process spawns) and 3 tests against a real
  `rust-analyzer` (incremental change reflected via `hover`, save/close
  round-trip with `NotOpen` enforced after close, and crash-restart
  replay via a real `SIGKILL`). Re-ran 3 more times after the final
  formatting pass to rule out timing flakiness — 0 failures, ~3s each
  run. Also verified: `cargo test -p lux-lsp session:: --test
  lsp_lifecycle` (the prior deliverable's gate, still 5/5 green — the
  `ServerSession` socket-channel addition didn't regress it),
  `cargo clippy -p lux-lsp --all-targets -- -D warnings` (clean),
  `cargo fmt --check -p lux-lsp` (clean), `cargo check --workspace` (no
  regressions elsewhere), `./scripts/check-limits.sh` (clean).
