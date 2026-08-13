# Receipt: Code actions + WorkspaceEdit transaccional (Phase 7)

- Phase: 07
- Crate(s): lux-lsp
- Goal: The "Code actions + WorkspaceEdit transaccional" deliverable of
  Phase 7 ("Cliente LSP real") from `LUX_PROMPT_MAESTRO.md` is complete
  and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-lsp -- code_actions::` (unit tests plus real
  `rust-analyzer` end-to-end tests)
- Iterations: 4 / 12
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Scope.** Adds `lux-lsp::code_actions` (`CodeActions`,
  `CodeActionsError`, `ApplyError`, `FileChange`, `preview`) plus a new
  `DocumentSync::apply_all` on top of the prior six Phase 7 deliverables.
  The master prompt's ask is "code actions y quick fixes reales aplicando
  `WorkspaceEdit`, incluidos los que tocan varios ficheros, con
  previsualización y aplicación transaccional (todo o nada)" — all four
  pieces (real actions/quick fixes, multi-file edits, preview, atomic
  apply) are covered. No `lux-ui` wiring (lightbulb menu, quick-fix
  popup) — same headless-library scope every prior Phase 7 receipt has
  followed.

  **"Aplicación transaccional" needed a real commit primitive, not just a
  loop over `sync.change()`.** Calling the existing per-file `change()`
  method once per targeted file would NOT be atomic: if file 3 of 5
  turned out not to be open, files 1 and 2 would already have been
  mutated and notified to the server before the failure surfaced. Added
  `DocumentSync::apply_all(&self, edits_by_uri: &BTreeMap<Url,
  Vec<TextEdit>>)` instead, structured as two phases under one lock
  acquisition: phase 1 validates every target URI is open and computes
  each one's prospective new text *without mutating anything*; phase 2
  commits only once phase 1 has fully succeeded, so by the time any
  document is actually mutated, every target is already guaranteed
  present. This lives in `sync.rs` (extending the document-sync
  deliverable's module) rather than `code_actions.rs`, since it needs
  direct access to `DocumentSync`'s private per-document map — the same
  category of necessary cross-deliverable extension `session.rs` has
  needed once per prior loop. Refactored the `didChange` content-change
  construction (`TextDocumentSyncKind::INCREMENTAL` vs `FULL`) out of
  `change()` into a shared `content_changes_for` free function so
  `apply_all` reuses the exact same negotiated-sync-kind logic rather
  than duplicating it.

  **A `WorkspaceEdit` that creates/renames/deletes a file is rejected
  outright, not partially honored.** This client's authority stops at
  already-open, in-memory documents — it doesn't own the filesystem.
  `preview` normalizes all three `WorkspaceEdit` shapes (the legacy
  `changes` map; `document_changes` as a flat `Vec<TextDocumentEdit>`;
  the full `DocumentChangeOperation` mix including `ResourceOp::
  {Create,Rename,Delete}`) into one ordered `FileChange` list, and
  `text_edits_only` converts that to the edit map `apply_all` needs,
  returning `Err(ApplyError::UnsupportedFileOperation)` -- converting
  nothing -- the moment any resource operation appears. Silently applying
  just the text-edit portion of a code action that also, say, creates a
  new file would leave the workspace in a state the server never
  intended and the user never approved; refusing the whole thing is the
  honest "all or nothing" reading of the deliverable, not a shortcut.

  **What got stuck, in order, and the actual fix for each (no repeated
  guesses at the same failure):**
  1. The real "Extract into variable" code-action test returned an empty
     action list from rust-analyzer for a full 60s, every attempt.
     Root-caused by inspecting `session.rs`'s existing pattern from the
     completion deliverable (client capabilities gating server features)
     rather than assuming rust-analyzer simply doesn't offer that
     refactor: added `textDocument.codeAction.codeActionLiteralSupport`
     (plus `resolveSupport`/`dataSupport`/`isPreferredSupport`) to the
     `initialize` handshake's `ClientCapabilities`. The very next run
     found the action immediately.
  2. With actions flowing, a `struct DeleteFile has no field named
     annotation_id` compile error in a unit test -- a wrong guess at
     `DeleteFile`'s shape (unlike `CreateFile`/`RenameFile`, it has no
     annotation field). Checked the real struct definition in the
     vendored `lsp-types` source and fixed the test to match.
  3. The extract-action real-server test then passed 6 of 8 repeated
     runs but failed 2 with "expected the resolved action to carry a
     WorkspaceEdit" -- a genuine timing flake, not a capability gap: the
     test grabbed the *first* non-empty code-action response and
     resolved whatever "extract"-titled action was in it, but an early
     response (while rust-analyzer is still finishing its own indexing)
     can list the action without it being resolvable to a real edit yet.
     Rewrote the retry loop to keep re-requesting *and* re-resolving
     until an action actually yields a `WorkspaceEdit`, rather than
     accepting the first list that merely contains a matching title. 8
     consecutive full runs afterward, 0 failures.

  **Verified on macOS (aarch64-apple-darwin)**: `cargo test -p lux-lsp --
  code_actions::` -- 8 tests: 6 pure unit tests (`WorkspaceEdit`
  normalization for all three response shapes, `document_changes` taking
  precedence over the legacy `changes` map per spec, a resource operation
  rejecting the whole apply with nothing converted, edits for the same
  file merging correctly) and 2 tests against a real `rust-analyzer`: a
  genuine "Extract into variable" action requested, resolved, and
  applied, with the open document's text confirmed to actually contain
  the new `let` binding afterward; and the multi-file transactional
  guarantee itself -- two open documents edited together in one call
  (success case), then a second call mixing one of those same documents
  with a third, never-opened target, confirming the call fails *and*
  neither already-open document was touched (the core "todo o nada"
  proof). Re-ran 8 times after the final fix specifically to rule out
  the timing flake seen mid-loop -- 0 failures across all 8. Also
  verified: the full crate suite (`cargo test -p lux-lsp`, 71 tests --
  all six prior deliverables' gates plus this one, all still green),
  the `session::` and `sync::` gates specifically (deliverables #1 and
  #2, whose modules this loop extended), `cargo clippy -p lux-lsp
  --all-targets -- -D warnings` (clean), `cargo fmt --check -p lux-lsp`
  (clean), `cargo check --workspace` (no regressions elsewhere),
  `./scripts/check-limits.sh` (clean -- largest touched file is `sync.rs`
  at 500 lines, still under the 600-line limit but the closest of any
  Phase 7 file so far; worth watching in the next deliverable that
  touches it).
