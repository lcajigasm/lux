# Receipt: Rename cross-file con preview multi-fichero (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: The "Rename cross-file con preview multi-fichero" deliverable of
  Phase 7 ("Cliente LSP real") from `LUX_PROMPT_MAESTRO.md` is complete
  and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-lsp -- rename::` (unit tests plus real
  `rust-analyzer` end-to-end tests)
- Iterations: 3 / 10
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** Adds `lux-lsp::rename` (`Rename`, `RenameError`,
  `RenameApplyError`, `RenameHunk`, `RenamePreview`, `preview`) on top of
  the prior seven Phase 7 deliverables, plus a correctness fix to
  `sync.rs` discovered while building it (below). The master prompt's ask
  is "rename cross-file vía LSP, con previsualización multi-fichero y
  aplicación o rechazo por hunk. Nada de reemplazo de cadenas" — all four
  pieces are covered: real `textDocument/rename` (never a text-search-
  and-replace), multi-file preview, and genuine per-hunk (not per-file)
  accept/reject. No `lux-ui` wiring (rename dialog, diff preview panel) —
  same headless-library scope every prior Phase 7 receipt has followed.

  **Reused `code_actions::preview` rather than re-deriving `WorkspaceEdit`
  normalization.** A rename's result is the same `WorkspaceEdit` shape a
  code action's `edit` field carries, so `rename::preview` calls
  `code_actions::preview` and flattens its `FileChange::Edit` entries into
  individually addressable `RenameHunk`s (stable `id`, `uri`, the raw
  `lsp_types::TextEdit`) while passing file operations through
  separately. No duplicated `document_changes`/`changes`-map-precedence
  logic.

  **"Aplicación o rechazo por hunk" is a real selection primitive, not a
  UI-layer filter.** `Rename::apply` takes a `HashSet<usize>` of accepted
  hunk ids; `hunks_to_edit_map` (the pure half, unit-tested independent
  of any session) builds the edit-by-file map from *only* those hunks,
  then hands it to `DocumentSync::apply_all` for the same atomic,
  validate-before-mutate guarantee the code-actions deliverable already
  built. Rejected hunks are never included at all — not applied and
  reverted, simply never sent. A rename whose `WorkspaceEdit` also
  requests a file create/rename/delete is refused outright (same
  reasoning as code actions): this client owns already-open in-memory
  documents, not the filesystem.

  **Found and fixed a latent multi-edit ordering bug in `sync.rs` before
  writing any rename-specific code.** Reasoning through what "aplicación
  o rechazo por hunk" needs to actually apply correctly surfaced a gap in
  the existing `DocumentSync::change`/`apply_all`: both applied a file's
  multiple edits via naive sequential folding in array order, but LSP
  `TextEdit` ranges are always relative to the *original* document, not
  to whatever an earlier edit in the same batch already produced. A
  rename touching the same file more than once (the common case — the
  symbol appears several times) routinely shifts line/column length,
  which silently invalidates every later edit's coordinates once the
  earlier one has been folded in. Neither the document-sync nor the
  code-actions deliverable's tests ever exercised multiple edits landing
  in one file, so this went unnoticed until reasoning about rename's
  primary use case surfaced it — found by design review, not by a failing
  test. Fixed with `sort_edits_for_application` (descending document
  position, so each not-yet-applied edit's coordinates are still valid
  the moment it's applied) used consistently for both the local fold and
  the `didChange` array sent to the server (same order both sides, so
  client and server converge on the same final text). Two new pure unit
  tests in `sync.rs` pin this down directly (`let x = x + 1;`, renaming
  both `x`s to the longer `index`, in both array orders) — this fix
  doesn't show up under the `rename::` gate filter since it lives in
  `sync.rs`, but `cargo test -p lux-lsp -- sync::` (13 tests, up from 11)
  and the full crate suite both stayed green throughout.

  **What got stuck, in order, and the actual fix for each:**
  1. The cross-file real-server tests failed immediately (not a 60s
     timeout — a hard error in under a second) with `"No references
     found at position" (jsonrpc error -32602)`. This is a genuine
     "nothing there" response, not rust-analyzer's usual "still
     computing" `CONTENT_MODIFIED` signal, so the existing
     `CONTENT_MODIFIED`-only retry tolerance didn't cover it. Root cause:
     a freshly loaded two-file crate can answer this way for a moment
     while rust-analyzer is still building its crate graph, before it's
     ready to resolve references at all. Broadened the retry loops (both
     `prepareRename` and `rename`) to tolerate any JSON-RPC response
     error, not just `CONTENT_MODIFIED`, within the same bounded 60s
     budget — a genuine bug would still surface as a timeout, not hang
     forever. Passed on the very next run.
  2. A related consideration, resolved before it became a bug rather
     than after: the fixture's `pub mod other;` needed `src/other.rs` to
     genuinely exist on disk (not just be opened via `didOpen` overlay
     content), since rust-analyzer resolves a module declaration to a
     file path as part of loading the crate graph. Wrote both fixture
     files to disk from the start rather than discovering this via a
     failure.

  **Verified on macOS (aarch64-apple-darwin)**: `cargo test -p lux-lsp --
  rename::` -- 7 tests: 5 pure unit tests (hunk flattening with stable,
  distinct ids across files; file operations kept separate from edit
  hunks; a file operation rejecting the whole apply; only-accepted-ids
  included when building the edit map; an empty accepted set applying
  nothing) and 2 tests against a real `rust-analyzer`: a genuine
  `prepareRename` response with the real current name (`"Point"`) as the
  placeholder, and the full cross-file rename flow -- a struct defined in
  one file and used (return type, associated-function call, struct
  literal) across both that file and a second one, previewed into hunks
  spanning both files, one file's hunks applied while confirming the
  other file is byte-for-byte unchanged, then the remaining hunks applied
  in an independent second call, with both files finally showing the
  renamed symbol and zero leftover occurrences of the old name. Re-ran 5
  times after the retry-tolerance fix -- 0 failures across all 5. Also
  verified: the full crate suite (`cargo test -p lux-lsp`, 80 tests --
  all seven prior deliverables' gates plus this one, all still green),
  the `session::`, `sync::` (now 13 tests, covering the ordering fix),
  and `code_actions::` gates specifically (deliverables #1, #2, and #7,
  whose modules this loop touched), `cargo clippy -p lux-lsp
  --all-targets -- -D warnings` (clean after one fix: `sort_by` →
  `sort_by_key` per clippy's own suggestion), `cargo fmt --check -p
  lux-lsp` (clean), `cargo check --workspace` (no regressions elsewhere),
  `./scripts/check-limits.sh` (clean -- `sync.rs` is now 580 of the
  600-line limit, the closest any Phase 7 file has come; worth watching,
  possibly splitting, in whichever future deliverable touches it next).
