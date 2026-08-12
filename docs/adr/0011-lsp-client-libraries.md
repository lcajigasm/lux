# 0011. LSP client foundation: `async-lsp`, not `lsp-server` or the stalled `tower-lsp`/`lsp-types` pair

## Status

Accepted. Final pick (`async-lsp`) made in Phase 7's server-management loop
(see Consequences) — originally deferred pending hands-on ergonomics.

## Context

`lux-lsp` needs a JSON-RPC/LSP transport and lifecycle foundation for a real
client (server discovery, incremental sync, diagnostics, completion,
navigation, code actions, rename, formatting — Phase 7). Two of the
historically popular choices are confirmed dead:

- `tower-lsp` 0.20.0 — no changes since 2023-08-11, confirmed still
  unmaintained on re-verification.
- `lsp-types` 0.97.0 — no changes since 2024-06-04, confirmed still
  stalled.

Live alternatives: `async-lsp` 0.2.4, `lsp-server` 0.10.0 (both actively
published), and `tower-lsp-server` 0.23.0, a maintained fork of `tower-lsp`
that pairs with `ls-types` instead of the stalled `lsp-types`.

## Decision

Build `lux-lsp` on `async-lsp` 0.2.4. It is used as a *client* (`lux-lsp`
talks outward to a spawned `rust-analyzer`/etc. process): `MainLoop::new_client`
returns a `ServerSocket` for issuing `initialize`/`shutdown`/`exit`/etc.
via the generated `LanguageServer` omnitrait, while a `Router` handles
server-to-client traffic (`$/progress`, `client/registerCapability`,
`workspace/configuration`, `window/workDoneProgress/create`). It depends on
`lsp-types` 0.95 internally and re-exports it as `async_lsp::lsp_types`, so
`lux-lsp` never adds `lsp-types` as its own direct dependency — satisfying
this ADR's original constraint without extra ceremony. Process spawning
uses `async-process` (not `tokio::process`): its `Child::stdout`/`stdin`
implement the `futures::AsyncRead`/`AsyncWrite` traits `MainLoop::run_buffered`
needs directly, with no `tokio-util::compat` shim, matching the pattern in
`async-lsp`'s own `examples/client_builder.rs` reference client (built and
run against real `rust-analyzer` during this loop, including a genuine
`SIGKILL`-and-restart pass. `tokio` itself is still used for the
supervision plumbing around that connection: `watch` for event-driven
status/progress (golden rule 5), `oneshot` for shutdown signaling, and
`tokio::time::timeout` for shutdown/restart deadlines.

`lsp-server` 0.10.0 was not picked: it's a synchronous, untyped
(`serde_json::Value`-params) JSON-RPC transport built primarily for
implementing rust-analyzer's own *server* side. Using it as a client would
mean hand-rolling every request/notification's typed params/result against
the LSP spec ourselves for all nine Phase 7 deliverables (completion,
hover, code actions with `WorkspaceEdit`, cross-file rename, ...) — the
mature `lsp_types` surface `async-lsp` already brings is exactly that work,
done and maintained. `tower-lsp-server` 0.23.0 + `ls-types` was not picked
either: `ls-types` is at 0.0.6, far short of `lsp-types` 0.95's protocol
coverage, and `tower-lsp`'s ecosystem (examples, docs) is overwhelmingly
server-side, the opposite of `lux-lsp`'s role.

## Alternatives considered

`tower-lsp` + `lsp-types` — rejected, both confirmed stalled/abandoned.
`lsp-server` — rejected, see Decision. `tower-lsp-server` + `ls-types` —
rejected, see Decision. Writing a from-scratch JSON-RPC/LSP layer —
rejected as unnecessary; the protocol surface Phase 7 requires (server
lifecycle, `$/progress`, incremental `didChange`, `publishDiagnostics`,
code actions with `WorkspaceEdit`, cross-file rename) is exactly what
`async-lsp` + `lsp_types` already model.

## Consequences

`lux-lsp`'s later Phase 7 deliverables (document sync, diagnostics,
completion, navigation, code actions, rename, formatting) build on the same
`ServerSocket`/`Router`/`lsp_types` surface the server-management loop
established — no foundation change expected for the rest of the phase.
Phase 7's test plan still requires a fuzzer against the JSON-RPC stream
parser (something v0 never had); that exercises `async-lsp`'s own framing,
not code `lux-lsp` owns, and remains a later deliverable's job.
