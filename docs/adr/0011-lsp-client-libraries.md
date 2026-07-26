# 0011. LSP client foundation: `async-lsp`/`lsp-server`, not the stalled `tower-lsp`/`lsp-types` pair

## Status

Accepted.

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

Build `lux-lsp` on `async-lsp` 0.2.4 or `lsp-server` 0.10.0 (final pick made
in Phase 7 based on hands-on ergonomics against the five target servers:
`rust-analyzer`, `typescript-language-server`, `pyright`, `gopls`,
`clangd`); if a `tower`-shaped middleware model is preferred at that point,
`tower-lsp-server` 0.23.0 + `ls-types` is the living alternative. Do not
depend on `tower-lsp` or bare `lsp-types` directly.

## Alternatives considered

`tower-lsp` + `lsp-types` — rejected, both confirmed stalled/abandoned.
Writing a from-scratch JSON-RPC/LSP layer — rejected as unnecessary; the
protocol surface Phase 7 requires (server lifecycle, `$/progress`,
incremental `didChange`, `publishDiagnostics`, code actions with
`WorkspaceEdit`, cross-file rename) is exactly what these maintained crates
already model.

## Consequences

The final choice between `async-lsp`/`lsp-server` and `tower-lsp-server` is
deferred to Phase 7 rather than locked here, since it depends on integration
ergonomics that are only visible once real server sessions are being
managed. Phase 7's test plan already requires a fuzzer against the JSON-RPC
stream parser (something v0 never had) regardless of which foundation is
chosen.
