# Lux — Integration TODO (agent `/goal` backlog)

This is the **closure backlog**. Domain crates for phases 1–10 largely
exist; the product does not. Read first:

- [`CLAUDE.md`](CLAUDE.md) — non-negotiable rules
- [`docs/STATUS.md`](docs/STATUS.md) — ground truth (trust over this file)
- [`docs/NON-GOALS.md`](docs/NON-GOALS.md) — hard exclusions
- [`docs/adr/0017-crate-complete-is-not-product-complete.md`](docs/adr/0017-crate-complete-is-not-product-complete.md)
- [`GOALS.md`](GOALS.md) — phase intent (often `library-done` only)

**How to use:** pick the lowest open `GNN` whose dependencies are done.
Run exactly:

```text
/goal <paste the Goal block for that item>
```

Do not skip ahead to Git/LSP/terminal/plugins before the editor chrome and
workspace spine work. Do not implement anything in NON-GOALS.

**Done means STATUS moves.** Library tests alone are not enough.

Legend: `OPEN` · `BLOCKED` · `DONE`

---

## Track 0 — Honesty & gates

### G00 — Keep roadmap honest with STATUS
- Status: `DONE` (ADR 0017 + this file + GOALS/README sync in the same change)
- Depends: —
- Goal: Roadmap language distinguishes `library-done` from STATUS
  `implemented`; agents have a single integration backlog.
- Verify: `test -f TODO.md && test -f docs/adr/0017-crate-complete-is-not-product-complete.md`

---

## Track 1 — Desktop boot & real documents

### G01 — System monospace fonts on Linux and Windows
- Status: `DONE`
- Depends: G00
- Goal: `cargo run -p lux` finds a real system monospace font on Linux and
  Windows (not only macOS paths) and reaches the first frame without panic.
- Done when:
  - `crates/lux/src/font.rs` (or successor) probes documented Linux/Windows
    candidates and/or fontconfig where appropriate
  - Failure mode remains honest (clear error, no silent blank render)
  - `docs/STATUS.md` notes boot-on-Linux/Windows for the shell rows only if
    actually exercised; otherwise receipt records the blocker
- Verify:
  - `cargo test -p lux`
  - Linux: `timeout 5 env WGPU_BACKEND=gl xvfb-run -a cargo run -p lux`
    prints `Lux: using system font` and does not panic in `font::`
  - Receipt: `docs/receipts/g01-system-monospace-fonts-linux-windows.md`
- Out of scope: custom bundled font product decision (may be a follow-up ADR)
- Closed: 2026-09-01 — Linux font + xvfb event-loop smoke verified; Windows
  loader/tests in tree, first-frame STATUS left `not-started` (see receipt)

### G02 — Open and save real files in `lux`
- Status: `DONE`
- Depends: G01
- Goal: The desktop binary edits a path from argv and/or a native open
  dialog; save uses `lux-core` atomic save+backup; the embedded
  `sample.txt` is no longer the only document.
- Done when:
  - Opening a UTF-8 file shows its contents in `EditorView`
  - Save / Save As persist bytes; dirty state is visible in chrome or title
  - Huge/binary policy from `lux-core` is honored (refuse or gated path —
    no fake preview)
  - Matching STATUS rows move only on platforms where this was run
- Verify:
  - `cargo test -p lux`
  - Linux: `timeout 5 env WGPU_BACKEND=gl xvfb-run -a ./target/debug/lux <file>`
  - Receipt: `docs/receipts/g02-open-save-real-files.md`
- Out of scope: multi-root workspace (G08)
- Closed: 2026-09-01 — Linux argv open + binary refuse + huge read-only
  smoke; save round-trip via `document` tests (same path as Ctrl/Cmd+S).
  Windows STATUS left `not-started` (see receipt)

---

## Track 2 — Config drives the app

### G03 — Wire `lux-config` into `lux` (settings + keymaps)
- Status: `DONE`
- Depends: G02
- Goal: Layered JSONC settings and keymap presets actually drive input and
  editor options in the running binary (hot reload via existing watchers,
  no polling).
- Done when:
  - Default keymap performs documented editing commands
  - Changing a watched settings file updates behavior without restart
  - No settings UI for unimplemented controls (rule #1)
  - STATUS rows for config/keymaps move on verified platforms
- Verify: `cargo test -p lux-config && cargo test -p lux` + receipt with
  reload scenario
- Out of scope: preferences GUI beyond what is needed to prove layering;
  importers can stay library-only until a later goal if STATUS already
  separates them
- Closed: 2026-09-01 — Linux: Sublime keymap dispatches save/open/save_as;
  `editor.fontSize` hot-reload via `ConfigWatcher` (unit + xvfb smoke).
  Windows STATUS left `not-started` (see receipt)

---

## Track 3 — Syntax on screen

### G04 — Wire `lux-syntax` highlights into the editor paint path
- Status: `DONE`
- Depends: G02 (G03 optional but preferred for theme choice)
- Goal: Tree-sitter tokens from `lux-syntax` color glyphs in `lux`'s
  render path for at least one opened buffer language; incremental edit
  updates highlights without blocking the UI thread.
- Done when:
  - `lux` depends on `lux-syntax`
  - Visible syntax colors for a baseline language (e.g. Rust)
  - Highlight work stays off the UI thread (channels/worker already in crate)
  - STATUS "Tree-sitter incremental highlighting" / token themes reflect
    **on-screen** reality, not library-only
- Verify: `cargo test -p lux-syntax && cargo test -p lux` + receipt
  screenshot or deterministic render test if one exists
- Out of scope: all 18 languages polished in UI chrome; folding UI (G07+)
- Closed: 2026-09-01 — Linux: Rust files get `LUX_DARK` token colors via
  `BackgroundHighlighter` + paint-path `color_at`; edits resubmit off the
  UI thread (`try_recv` drain). Windows/macOS on-screen left unverified.

---

## Track 4 — Editor chrome & navigation

### G05 — Tabs + status bar wired and painted
- Status: `OPEN`
- Depends: G02, G03
- Goal: Multi-buffer tabs and a status bar are visible and operable in
  `lux` using existing `lux-ui` modules (not new parallel widgets).
- Done when: open multiple files into tabs; close/switch; status shows
  cursor/encoding/EOL or honest subset; STATUS rows move
- Verify: `cargo test -p lux-ui` + `cargo test -p lux` + receipt
- Out of scope: custom drag-drop tab DnD polish beyond existing module

### G06 — Command palette + Goto Anything wired
- Status: `OPEN`
- Depends: G05, G03
- Goal: Unified command palette and Goto Anything (`lux-ui` + `lux-search`
  fuzzy) are reachable from real keybindings and perform real commands /
  file jumps.
- Done when: palette runs at least open-file / save / basic edit commands;
  Goto opens files from the current folder or workspace root when present;
  no fake result lists; STATUS rows move
- Verify: receipt with keybinding → action; `cargo test -p lux-ui`
- Out of scope: `#symbol` / `#text` suffixes (ADR 0014 deferred)

### G07 — Find/replace bar + minimap + folding/brackets UI
- Status: `OPEN`
- Depends: G04, G05
- Goal: In-buffer find/replace UI, minimap, and syntax folding/bracket
  emphasis use existing `lux-ui` / `lux-syntax` APIs from the binary.
- Done when: STATUS rows for search bar, minimap, folding, bracket colors
  are `implemented` on at least one desktop OS where verified
- Verify: `cargo test -p lux-ui && cargo test -p lux-syntax` + receipt
- Out of scope: project-wide replace (G09)

---

## Track 5 — Workspace & project search

### G08 — Wire `lux-workspace` + file tree + native watchers
- Status: `OPEN`
- Depends: G05
- Goal: Open a folder / `.lux-project`, show virtualized file tree, keep
  index current via native FS watchers (no polling).
- Done when: tree reflects add/delete/rename; multi-root if library already
  supports it; STATUS rows move; UI thread does no tree I/O
- Verify: `cargo test -p lux-workspace && cargo test -p lux-ui` + receipt
- Out of scope: remote FS / SSH

### G09 — Find in files + project replace with diff preview
- Status: `OPEN`
- Depends: G08
- Goal: Embedded ripgrep search and project replace from `lux-search` are
  operable from the UI with real progressive results and diff preview
  before apply.
- Done when: matching STATUS rows move; cancelation works; no simulated hits
- Verify: `cargo test -p lux-search` + receipt on a real tree
- Out of scope: cloud search

### G10 — Sessions & crash recovery wired
- Status: `OPEN`
- Depends: G08, G02
- Goal: Session restore and crash recovery from `lux-workspace` actually
  reopen the prior workspace/tabs after restart.
- Done when: kill -9 / panic recovery path documented and STATUS row moves
- Verify: receipt with before/after restart; `cargo test -p lux-workspace`
- Out of scope: cloud session sync (NON-GOALS)

---

## Track 6 — Git, LSP, terminal, plugins (UI)

### G11 — Wire `lux-git` into editor surfaces
- Status: `OPEN`
- Depends: G08
- Goal: Gutter hunks + at least one real changes/diff/commit flow visible
  in `lux`, backed by in-process `gix` (no shell-out).
- Done when: STATUS git rows that you claim move only after UI proof
- Verify: `cargo test -p lux-git` + receipt on a real repo
- Out of scope: hosting remotes / PR UI / GitHub app

### G12 — Wire `lux-lsp` client UI
- Status: `OPEN`
- Depends: G04, G03, G05
- Goal: Server lifecycle, diagnostics, completion, and go-to-definition
  work in the binary against at least one real server (prefer
  `rust-analyzer`).
- Done when: STATUS LSP rows you claim are `implemented` on a verified OS;
  failures are visible, not silent
- Verify: `cargo test -p lux-lsp` + receipt with real server
- Out of scope: DAP/debugger (NON-GOALS); every language server

### G13 — Wire `lux-term` PTY panel
- Status: `OPEN`
- Depends: G05
- Goal: A real terminal panel in `lux` using `lux-term` (PTY, scrollback,
  resize); excluded platforms stay excluded (ADR 0006 / 0012).
- Done when: STATUS terminal rows move on macOS/Windows/Linux as verified;
  iOS remains `excluded`
- Verify: `cargo test -p lux-term` + receipt (`vim` or shell smoke)
- Out of scope: full task runner IDE; Android term if STATUS/ADR exclude

### G14 — Wire `lux-plugin` install + command contribution
- Status: `OPEN`
- Depends: G06, G03
- Goal: Local install + capability grants + at least one example plugin
  contributes a command reachable from the palette; sandbox limits enforced.
- Done when: STATUS plugin rows move on verified desktop OS; App Review
  caveat retained for iOS
- Verify: `cargo test -p lux-plugin` + receipt installing example plugin
- Out of scope: marketplace (NON-GOALS); network/process host backends may
  remain honest `Unimplemented` until a dedicated goal

---

## Track 7 — Desktop STATUS parity & a11y proof

### G15 — Prove wired desktop features on Linux and Windows
- Status: `OPEN`
- Depends: G01–G07 minimum; re-run after each later track as needed
- Goal: For every STATUS row marked `implemented` on macOS for a wired
  feature, either mark the same on Linux/Windows after real runs, or
  document a concrete platform blocker in STATUS (not silence).
- Done when: Linux and Windows columns stop being wholesale `not-started`
  for tracks already wired; CI alone does not flip UI rows
- Verify: receipts per OS + STATUS diff in the same PR
- Out of scope: claiming mobile

### G16 — Screen-reader / keyboard passes on real AT
- Status: `OPEN`
- Depends: G05–G06; accesskit tree already present from phase 2
- Goal: Exercise VoiceOver (macOS), NVDA (Windows), Orca (Linux) against
  the wired UI; fix blockers that are in our tree; leave STATUS honest if
  AT is unavailable.
- Done when: STATUS screen-reader cells reflect real runs or explicit
  `not-started` with reason
- Verify: receipt per AT; no fake "compatible" claims
- Out of scope: inventing a full a11y rewrite unrelated to accesskit ADR

---

## Track 8 — Packaging & mobile hosts

### G17 — Packaging v1.0 remaining work
- Status: `OPEN` / partially `BLOCKED` on certificates
- Depends: G15 for "clean install" claims
- Goal: Finish unsigned channel layout, crash log paths per OS, update
  verify docs; signed/notarized artifacts when certs exist.
- Done when: STATUS packaging rows match reality (unsigned vs signed)
- Verify: `scripts/package.sh` (or successor) + `cargo test -p lux-pack`
- Out of scope: pretending notarization without Apple/Microsoft certs

### G18 — iPadOS/iOS host beyond skeleton (v1.1)
- Status: `OPEN`
- Depends: G02–G07 desktop spine preferred for shared Rust
- Goal: UIKit backend `available() == true` on device; IME + document
  picker paths real; capability profile enforced (no PTY).
- Done when: STATUS Phase 12 rows move from `not-started` only after
  device/simulator evidence
- Verify: receipt on simulator/device; `host-ios/` no longer empty stubs
- Out of scope: App Store submission if certs/profile missing (record
  BLOCKED)

### G19 — Android host beyond skeleton (v1.2)
- Status: `OPEN`
- Depends: G18 pattern / shared FFI lessons
- Goal: Kotlin/JNI backend real on device; IME `InputConnection`; SAF open;
  NDK builds for crates that need clang (`lux-syntax`, etc.).
- Done when: STATUS Phase 13 rows move after device evidence
- Verify: receipt on emulator/device; NDK CI job or documented local script
- Out of scope: Play Store publishing without keys (BLOCKED, honest)

---

## Suggested `/goal` paste template

```text
/goal Execute TODO.md item GNN (<title>). Follow CLAUDE.md and ADR 0017.
Do not mark docs/STATUS.md implemented without running the capability in
the lux binary on that platform. Leave a docs/receipts/ entry with the
verify commands. Do not implement NON-GOALS. Commit on a cursor/* branch
and open a PR when the item's Done when is met.
```

## What not to do

- Do not add debugger, collab, AI, marketplace, telemetry, or cloud sync.
- Do not add UI chrome for capabilities not wired (rule #1).
- Do not start G11–G14 before G02–G08 unless unblocking a proven dependency.
- Do not flip Windows/Linux/iOS/Android STATUS cells from macOS CI alone.
