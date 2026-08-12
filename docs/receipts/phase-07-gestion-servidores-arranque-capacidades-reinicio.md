# Receipt: Gestión de servidores (arranque, capacidades, reinicio) (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: The "Gestión de servidores" deliverable of Phase 7 ("Cliente LSP
  real") from `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with
  no failures and no ignored tests.
- Verify: `cargo test -p lux-lsp session:: --test lsp_lifecycle` (against
  real `rust-analyzer`)
- Iterations: 2 / 12
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** `lux-lsp` didn't exist beyond an empty crate skeleton (a
  two-line `lib.rs`, no dependencies). This loop adds server management
  only — discovery, launch, the `initialize`/`initialized` capability
  handshake, `shutdown`/`exit` with a kill fallback, crash-triggered
  restart, one server per `(language, root)`, and a `watch`-channel
  `ServerState` (status, capabilities, pid, restart count, `$/progress`) —
  nothing else from Phase 7's much longer deliverable list (incremental
  document sync, diagnostics, completion, navigation, hover/inlay
  hints/semantic tokens, code actions, cross-file rename, formatting) and
  no wiring into `lux-ui`/the running `lux` binary. `docs/STATUS.md`'s
  "Server lifecycle management" row stays `not-started`, same convention
  as every other headless-library scope note in that file — see the note
  added to its Phase 7 section.

  **ADR 0011 closed out.** The ADR had deferred the final pick between
  `async-lsp`/`lsp-server`/`tower-lsp-server` to "hands-on ergonomics" in
  Phase 7. Picked `async-lsp` 0.2.4: `lux-lsp` is a *client* (talks
  outward to a spawned server process), and `async-lsp`'s
  `MainLoop::new_client` + `LanguageServer` omnitrait + `Router` fit that
  role directly, with the full `lsp_types` 0.95 protocol surface
  re-exported (`async_lsp::lsp_types`) so `lux-lsp` never adds `lsp-types`
  as its own dependency. `lsp-server` was ruled out hands-on: it's
  untyped (`serde_json::Value` params) and built for implementing a
  server's own side, not for driving one as a client. `tower-lsp-server`
  was ruled out on paper: its `ls-types` is at 0.0.6, far short of
  `lsp-types` 0.95's coverage. See ADR 0011 and `docs/DEPENDENCIES.md` for
  the full writeup.

  **Process transport: `async-process`, not `tokio::process`.**
  `async-lsp`'s `MainLoop::run_buffered` takes `impl AsyncRead`/`impl
  AsyncWrite` from the `futures` crate; `async_process::Child`'s
  `stdout`/`stdin` implement those directly, so the child's pipes plug
  straight into the main loop with no `tokio-util::compat` shim — the
  same pattern `async-lsp`'s own `examples/client_builder.rs` reference
  client uses (read before writing any code here, and it matches almost
  line for line). `tokio` is still used, but only for the supervision
  plumbing around that connection: `watch` for event-driven status/
  progress (golden rule 5 — no polling), `oneshot` for the shutdown
  signal, `tokio::spawn` for the per-session supervisor task and the main
  loop task, `tokio::time::timeout` for shutdown/restart deadlines.

  **Server-to-client requests are answered for real, not stubbed.**
  `rust-analyzer` calls `client/registerCapability`,
  `window/workDoneProgress/create`, and `workspace/configuration` during
  startup. The client `Router` answers all three: the first two with
  `Ok(())` (dynamic-registration bookkeeping and progress-token creation
  aren't tracked as separate objects in this deliverable — `$/progress`
  notifications are read directly, keyed by the token the server chose),
  and `workspace/configuration` with one `Value::Null` per requested item
  — the spec-correct "no client override, use your defaults" response,
  not a placeholder. Leaving the default `unhandled_request` handler
  (`METHOD_NOT_FOUND`) on these would have made every real
  `rust-analyzer` session's very first requests fail.

  **Crash simulation uses a real `SIGKILL`, not a mock.** The pid a
  session's spawned process gets is part of the publicly visible
  `ServerState` (`state.pid`) — a legitimate piece of "estado real y
  visible por servidor" in its own right, not a test-only backdoor — so
  `crash_triggers_automatic_restart` sends `libc::kill(pid, SIGKILL)` at
  the real OS process and then waits (via the `watch` channel, no
  polling) for `restarts >= 1 && status == Running` with a fresh pid and
  a re-completed capability handshake. Restart is capped at 3 attempts
  (`MAX_RESTARTS`); exceeding it moves the session to a terminal `Failed`
  status instead of restart-looping forever.

  **Discovery is a real `PATH` search**, not just `Command::new(name)
  .spawn()` and hoping: `discover_command` resolves the binary ahead of
  spawning, on the same `PATH` the OS would use, so a genuinely missing
  server reports a clear `Failed { reason: "... not found on PATH" }`
  instead of an opaque spawn error. `KNOWN_SERVERS` maps all five
  acceptance-criteria languages (`rust-analyzer`, `typescript-language-
  server --stdio`, `pyright-langserver --stdio`, `gopls`, `clangd`) to
  their launch spec, though only `rust-analyzer` is exercised by the gate
  — the other four aren't installed on this machine, and installing and
  testing against all five is explicitly the acceptance bar for the whole
  *phase*, not this one deliverable.

  **What actually got stuck for one iteration, and why it wasn't a code
  bug.** The first full gate run failed 4 of 5 tests with `Failed {
  reason: "initialize failed: service stopped" }`. `discover_command`
  correctly found `~/.cargo/bin/rust-analyzer` on `PATH` — but that's a
  `rustup` *proxy*, not a real binary: it resolves which toolchain's
  `rust-analyzer` to run based on the *spawned process's* working
  directory (walking up for a `rust-toolchain.toml` override), and the
  session spawns with `current_dir` set to the test's temp-dir project
  root, which has no override — so it fell through to the machine's
  *default* `rustup` toolchain (`stable`), which didn't have the
  `rust-analyzer` component installed (only this repo's pinned `1.97.1`
  override did, from earlier in this session). Fixed with `rustup
  component add rust-analyzer --toolchain stable` — a one-time host
  environment fix, not a `lux-lsp` change; temporary `eprintln!`
  instrumentation (capturing the main-loop task's actual error instead of
  discarding it, plus inheriting the child's `stderr`) found it in one
  pass and was removed before the passing run. Worth remembering:
  `rustup` shims are cwd-sensitive, so any future test that spawns a
  rustup-shimmed tool with a `current_dir` outside the repo needs the
  same toolchain available on whichever toolchain that directory resolves
  to, not just the repo's pinned one.

  **Verified on macOS (aarch64-apple-darwin)**: `cargo test -p lux-lsp
  session:: --test lsp_lifecycle` — 5 tests, all against a real
  `rust-analyzer` (no fake/mock server): capability handshake (asserts a
  real `hover_provider` capability), `$/progress` reporting during
  indexing, clean shutdown (`shutdown` + `exit`, idempotent), one-server-
  per-language-and-root reuse (`Arc::ptr_eq` across two `get_or_start`
  calls, distinct session for a distinct root), and crash-triggered
  restart via a real `SIGKILL`. Re-ran the exact gate command three more
  times after formatting to rule out timing flakiness — 0 failures across
  all runs. Also verified: `cargo test -p lux-lsp --lib` (4 unit tests,
  discovery + registry), `cargo clippy -p lux-lsp --all-targets -- -D
  warnings` (clean), `cargo fmt --check -p lux-lsp` (clean), `cargo check
  --workspace` (no regressions elsewhere), `./scripts/check-limits.sh`
  (clean — largest file is `session.rs`, well under 600 lines; largest
  struct, `ServerState`, has 6 fields).
