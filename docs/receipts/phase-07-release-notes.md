# Phase 7 release notes — Cliente LSP real

Commits from `de61bff` (end of Phase 6) to `HEAD`. Phase 7's own work is
two commits (`0f7dcec`, `1c769c3`) — the crate itself, then its docs/
receipts. The other four (`26df33a`..`88a1582`) are pre-existing
maintenance fixes to `lux-git`/`lux-core`/`lux-config` that landed on this
branch before Phase 7 started; unrelated to the LSP client, listed below
for completeness.

## Highlights

- **`lux-lsp`** (new crate, thirteen modules), a real LSP client with a
  full lifecycle — not the polling `collect_snapshot` v0 had:
  - **`session`**: discovery, launch, capability handshake, clean
    shutdown, crash-triggered restart (up to 3 attempts), one server per
    `(language, root)`, and real per-server status with `$/progress` —
    all event-driven via `tokio::watch`, never polled. Built on
    `async-lsp` 0.2.4 (ADR 0011's final pick) + `async-process` (no
    `tokio-util::compat` shim needed).
  - **`sync`**: incremental `didChange` (server-negotiated
    `INCREMENTAL`/`FULL`/default, never assumed), correct
    `didOpen`/`didSave`/`didClose`, UTF-16-code-unit-aware position math,
    and automatic `didOpen` replay after a crash restart.
  - **`diagnostics`**: continuous `publishDiagnostics` reception,
    severity filtering, per-document summaries, and stable
    next/previous navigation with wraparound.
  - **`completion`**: `resolve`, server-sort-order compliance, snippet
    items surfaced faithfully, and staleness rejection via a generation
    counter (no client-side cancellation in `async-lsp`, so a fast
    typist's screen can't show a stale result over a newer one).
  - **`navigation`**: definition/declaration/type-definition/
    implementations, references, call hierarchy, document and workspace
    symbols — nine LSP requests behind one normalized API.
  - **`hover`/`signature`/`inlay_hints`/`semantic_tokens`**: all three
    `MarkedString`/`MarkupContent` hover shapes flattened; signature
    help's UTF-16 label offsets; inlay hints for a visible range;
    delta-encoded semantic tokens decoded to absolute, legend-resolved
    spans.
  - **`code_actions`**: `WorkspaceEdit` normalized from any of its three
    wire shapes into one ordered `Vec<FileChange>`, applied
    transactionally (validate every target is open, then mutate all or
    nothing) via a new `DocumentSync::apply_all`.
  - **`rename`**: cross-file rename via real `textDocument/rename` (never
    string replacement), multi-file preview, and genuine per-hunk
    accept/reject — reusing `code_actions`' `WorkspaceEdit` normalization
    rather than re-deriving it.
  - **`formatting`**: whole-document and range formatting,
    `format_and_save` (formats before saving, not after), and
    `FormattingConfig`/`LanguageFormattingConfig` with real per-language-
    over-workspace-default precedence.
  - **`framing`**: a defensive Content-Length cap this phase's own
    fuzzer found it needed — see Findings.
- **A real fuzzer for the JSON-RPC stream parser** (`fuzz/
  lsp_stream_fuzz`, the phase's own acceptance criterion, "v0 no tenía
  ninguno") — drives the actual production path (`guard_stream` wrapping
  `async-lsp`'s `Message::read`) with raw bytes. Final clean run:
  **53,608,974 executions in 1801 seconds, zero crashes.**
- **Verified against all five required servers** — `rust-analyzer`,
  `typescript-language-server`, `pyright`, `gopls`, `clangd` — not just
  `rust-analyzer`, which every one of the nine deliverables' own tests
  had used exclusively until this phase's closing loop.
- **A fake-server protocol test suite** (`lux-lsp-fake-server`, a
  test-only binary speaking the real wire protocol with scripted
  responses) — the phase's other named acceptance criterion, distinct
  from the fuzzer: proves exactly what this client sends during
  `initialize`, an exact `$/progress` sequence, and restart behavior for
  a server that exits on its own rather than being killed.
- **A no-thread-blocking proxy check** for "ninguna operación LSP
  bloquea el hilo de UI" — no `lux-ui` exists yet to measure a real
  frame, so this proves the mechanism instead: a slow, in-flight hover
  request never stalls a concurrent tick loop sharing its executor
  thread.

## Findings during this phase, not just deliverables landing on schedule

- **A real, fuzzer-found unbounded-allocation crash in the `async-lsp`
  dependency, fixed rather than routed around.** `async-lsp` 0.2.4's
  `Message::read` allocates `vec![0u8; content_len]` straight from a
  server-sent `Content-Length` header with no upper bound — a single
  short, hostile or buggy header line aborts the whole client process.
  `Message::read` is private with no hook to patch from outside, so
  fixed in `lux-lsp` itself: `framing::guard_stream`, a transparent
  wire-framing pass-through that rejects any claimed length over 64 MiB
  before an allocation is attempted, now sitting between every real
  child process's stdout and `async-lsp`. Re-fuzzing after that fix
  found a second, subtler bug — the guard's own header matching was
  looser than `async-lsp`'s, so a malformed second header line could
  mask a real, huge value read from an earlier well-formed one — fixed
  by matching `async-lsp`'s exact header syntax byte-for-byte.
- **A real client bug found while testing against a second server**:
  `session.rs`'s `initialize` call only ever sent `workspace_folders`,
  never the (spec-deprecated, but still practically required)
  `root_uri`. `typescript-language-server` 5.3.0 can't resolve its own
  `typescript` dependency without it and fails the whole handshake.
  Found by manually driving the server's raw stdio to isolate the
  client's exact contribution before touching any code. Fixed by sending
  both.
- **Getting real capability-gated behavior out of `rust-analyzer`
  needed three separate client-capability declarations**, discovered by
  inspecting the real capability payload rather than assuming a gap:
  `completionItem.resolveSupport`/`snippetSupport` (completion),
  `codeAction.codeActionLiteralSupport`/`resolveSupport`/`dataSupport`
  (code actions), and sending `root_uri` (above).
- **A latent multi-edit ordering bug in `DocumentSync`, found by design
  review before it became a test failure**: `change`/`apply_all` applied
  a file's multiple edits in array order, but LSP `TextEdit` ranges are
  relative to the *original* document — an earlier edit that shifts
  length silently invalidated later ones in the same file. Fixed by
  sorting to descending document position before folding, on both the
  local apply and the `didChange` sent to the server.
- **A genuine, confirmed-not-assumed upstream limitation, left
  unforced**: rust-analyzer's range formatting requires a nightly
  `rustfmt` opt-in this client correctly doesn't assume; the range-
  formatting test reads the server's real advertised capability and
  asserts the honest outcome either way, rather than hard-coding success.

## Not yet built or verified, honestly

- **No `lux-ui`/`lux` wiring for any of it** — every deliverable's own
  scope note says so; `lux-lsp` is a complete, tested, headless library
  with no diagnostics panel, completion popup, hover tooltip, code-action
  lightbulb, rename dialog, or format command anywhere a user of the
  running app could reach. This is the same-shaped gap every phase since
  Phase 3 has had for its own backend work.
- **Combining semantic tokens with tree-sitter's own highlighting**
  (the master prompt's literal phrase) needs both `lux-syntax` and
  `lux-lsp` wired into `lux-ui`'s renderer — this phase delivers the
  necessary real, decoded, legend-resolved spans, not the combining
  itself.
- **"Un servidor que se cuelga o muere no arrastra al editor"** is
  covered at the `lux-lsp` layer (both a `SIGKILL` test and a
  self-exiting-server test restart cleanly), but "el editor" as a full
  application doesn't exist yet with `lux-lsp` wired in, so the
  full-system claim can't be verified end-to-end.
- Windows and Linux remain `not-started` — no non-macOS dev environment
  available, same constraint as every phase before it.
- The fuzzer and multi-server/fake-server suites are real, run manually
  this phase, not part of routine `cargo test --workspace` (the fuzzer
  needs a nightly toolchain outside this repo's stable pin; the
  multi-server suite needs five real server binaries on `PATH`) —
  matching how every other phase's large-scale verification pass has
  been described, not a promise that the exact commands run in CI.

## Commits

```
0f7dcec feat(lux-lsp): add a real LSP client for Phase 7
1c769c3 docs: record Phase 7 LSP client deliverables and scope
```

Pre-existing, unrelated maintenance fixes also in this range (not Phase
7 work):

```
26df33a fix(lux-git): stop relying on ambient git identity for ref-log writes
50a8fb8 fix(lux-config): avoid non-char-boundary panic, import fractional font settings
71885b1 fix(lux-core): close the 100 MB open RSS budget gap left open at Phase 1
88a1582 fix(lux-config): fix flaky Linux hot-reload test on file creation
```
