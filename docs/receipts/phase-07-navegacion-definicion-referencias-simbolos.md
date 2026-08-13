# Receipt: Navegación (definición, referencias, símbolos) (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: The "Navegación (definición, referencias, símbolos)" deliverable
  of Phase 7 ("Cliente LSP real") from `LUX_PROMPT_MAESTRO.md` is complete
  and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-lsp navigation::` (unit tests plus real
  `rust-analyzer` end-to-end tests)
- Iterations: 3 / 10
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** Adds `lux-lsp::navigation` (`Navigation`, `NavigationError`,
  `SymbolEntry`, `WorkspaceSymbolEntry`) on top of the prior four Phase 7
  deliverables. The master prompt's ask for this deliverable is
  "definición, declaración, tipo, implementaciones, referencias,
  jerarquía de llamadas, símbolos de documento y de workspace" — all nine
  of those LSP requests are covered here, not a subset; no `lux-ui`
  wiring (jump-to-definition keybinding, problems/outline panels), same
  headless-library scope every prior Phase 7 receipt has followed.
  `docs/STATUS.md`'s "Navigation" row stays `not-started`.

  **Read the `lsp_types` source before designing the API, not after
  hitting a type error.** Before writing `navigation.rs`, checked
  `lsp-types-0.95.1/src/request.rs` directly: `GotoDeclarationResponse`,
  `GotoTypeDefinitionResponse`, and `GotoImplementationResponse` are all
  literal `pub type X = GotoDefinitionResponse;` aliases, not just
  similarly-shaped types. That confirmed one normalizer
  (`locations_from_goto`, handling the `Scalar`/`Array`/`Link` response
  variants) could correctly serve all four goto-style requests instead of
  four near-duplicate ones — collapsing what could have been ~80 lines of
  repetition into one function, unit-tested once against all three
  variants (plus `None`). Same approach for `documentSymbol` and
  `workspace/symbol`: read `document_symbols.rs`/`workspace_symbols.rs`
  directly, confirmed both return an `untagged` enum of either the modern
  nested shape (`DocumentSymbol`/`WorkspaceSymbol`, hierarchical, with
  `range`/`selection_range`) or the older flat `SymbolInformation` list,
  and normalized both into one `SymbolEntry`/`WorkspaceSymbolEntry` shape
  so callers don't need to branch on which one a given server sent.

  **`LocationLink` resolves to its `target_selection_range`, not
  `target_range`.** A `GotoDefinitionResponse::Link` variant (used by
  servers with "location link" client-capability support) carries both a
  coarse `target_range` (the whole enclosing construct) and a precise
  `target_selection_range` (what should actually be revealed/selected --
  e.g. just the function name, not its whole body). `locations_from_goto`
  uses the latter, matching what a real "jump to definition" should land
  the cursor on; a unit test pins this down explicitly so a future change
  can't silently regress to the coarser range.

  **One clean root-cause fix, found immediately and applied everywhere it
  applied, not per-test:** the first real-server run failed two of three
  tests with `content modified (jsonrpc error -32801)` -- rust-analyzer's
  well-known "not ready yet, ask again" reply (the same one `async-lsp`'s
  own reference client retries on for `hover`, already known from the
  document-sync deliverable's work). The retry helper
  (`retry_until`) only tolerated an empty/incomplete *result*, not this
  specific *error*; added the same `CONTENT_MODIFIED` tolerance
  `sync`/`completion`'s real-server tests already use. A second full run
  still had one flaky failure: `incoming_calls`/`outgoing_calls` were
  called directly (`.await.expect(...)`) rather than through
  `retry_until`, so they weren't covered by the same fix yet. Routed both
  through `retry_until` too (predicate: non-empty). Five consecutive full
  runs after that, zero failures — the fix was structural (every
  request that talks to a still-indexing server needs the same
  tolerance), not a one-off patch.

  **Real-server tests use one fixture doing real work, not one test per
  request.** A single small crate — a `Point` struct with a `new`
  constructor, `make()` constructing one, and `use_it()` calling `make()`
  — gives a genuine caller/callee relationship, a genuine type usage, and
  a genuine `impl` block, letting `definition`, `references`,
  `implementations`, `type_definition`, `document_symbols`,
  `workspace_symbols`, and the full call-hierarchy flow all assert real
  semantic outcomes (a specific definition line, a minimum reference
  count, a specific `impl` line, real symbol names, a real caller and a
  real callee) rather than "the request returned something." `declaration`
  is the one exception: Rust has no header/source split for LSP
  `textDocument/declaration` to meaningfully distinguish from
  `definition`, so that call is only checked for a clean round-trip
  (`Ok`, tolerating an empty result), not specific content — asserting
  more there would have been asserting rust-analyzer implementation
  details this deliverable doesn't depend on.

  **Verified on macOS (aarch64-apple-darwin)**: `cargo test -p lux-lsp
  navigation::` — 9 tests: 6 pure unit tests (all three
  `GotoDefinitionResponse` variants plus `None`, nested-vs-flat document
  symbol normalization, no process spawned) and 3 tests against a real
  `rust-analyzer` covering all nine navigation requests against the
  `Point`/`make`/`use_it` fixture described above. Re-ran 5 times after
  the `CONTENT_MODIFIED`/retry fix to specifically rule out the flakiness
  the first two runs showed — 0 failures across all 5, ~0.9-2.9s each
  run (call latency varies with whether rust-analyzer's still warming up
  from a fresh process). Also verified: the full crate suite (`cargo test
  -p lux-lsp`, 43 tests -- the prior four deliverables' gates plus this
  one, all still green; no shared infrastructure changed this loop, so no
  regression risk to them), `cargo clippy -p lux-lsp --all-targets -- -D
  warnings` (clean), `cargo fmt --check -p lux-lsp` (clean), `cargo check
  --workspace` (no regressions elsewhere), `./scripts/check-limits.sh`
  (clean -- `navigation.rs` 417 lines, `navigation/real_server.rs` 313
  lines, both well under 600).
