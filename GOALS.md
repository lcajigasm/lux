# Lux — Project Goals

This is the full goal map, phase by phase, from an empty repository to
`v1.2.0`. It exists so an IDE agent (Cursor or otherwise) can pick up this
project and drive it to completion without re-deriving the plan.

**Read these first, they are authoritative and this file does not repeat
them:**

- [`CLAUDE.md`](CLAUDE.md) — the ten non-negotiable rules, commands, and why
  they exist (post-mortem of the failed `lux-edit` v0 attempt).
- [`docs/STATUS.md`](docs/STATUS.md) — what is *actually* implemented right
  now, per platform. This is ground truth; if it disagrees with the
  checklist below, trust `STATUS.md`.
- [`docs/NON-GOALS.md`](docs/NON-GOALS.md) — explicitly out of scope until
  after `v1.0.0`. Do not implement these no matter how small the ask seems.
- [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) — the measured performance
  budgets and benchmark history.
- [`docs/adr/`](docs/adr/) — every architectural decision already made
  (UI stack, text rendering stack, platform trait, dependency choices).
  Do not reopen these without new evidence; supersede with a new ADR.
- `LUX_PROMPT_MAESTRO.md` (repo root) — the original build brief this file
  distills, with full rationale and acceptance criteria per phase.

## Product vision

Lux is a free, cross-platform text and code editor with the feel of
Sublime Text 3: instant startup, imperceptible input latency, flawless
handling of huge files, first-class multicursor, and a UI that never gets
in the way. Scope for v1.0 also includes a real terminal, a real LSP
client, and integrated Git. Not an IDE; does not chase VS Code's breadth.

**Platform order:** macOS, Windows, Linux (v1.0) → iPadOS/iOS (v1.1) →
Android (v1.2).

**License:** MIT or Apache-2.0 dual. Free and open source, no paid tier,
no telemetry by default.

## Roadmap

| Phase | What | Release | Status |
|---|---|---|---|
| 0 | Foundations & quality gate | — | done |
| 1 | `lux-core`: headless text engine | — | done |
| 2 | Renderer & desktop shell | `v0.1.0` | done |
| 3 | Syntax, navigation, command palette | `v0.2.0` | done |
| 4 | Projects, files, global search | `v0.3.0` | done |
| 5 | Config, keymaps, personalization | `v0.4.0` | done |
| 6 | Git integration (`gix`) | `v0.5.0` | done |
| 7 | Real LSP client | `v0.6.0` | done |
| 8 | Real PTY terminal | `v0.7.0` | done |
| 9 | WASM plugins with sandbox | `v0.8.0` | done |
| 10 | Accessibility, i18n, polish | `v0.9.0` | done |
| 11 | Packaging & distribution | `v1.0.0` | in progress (unsigned pack + crash/update verify; signing blocked on certs) |
| 12 | iPadOS & iOS | `v1.1.0` | in progress (Swift/MTKView host; no App Store build) |
| 13 | Android | `v1.2.0` | in progress (Kotlin SurfaceView+IME+SAF; no Play build) |

---

## Phase 0 — Foundations & quality gate

*Done.* Cargo workspace, `CLAUDE.md`, ADRs 0001–0015, CI (fmt/clippy/test/
deny/limits/benches), `scripts/check-limits.sh`, core-isolation CI job,
Criterion baseline, `docs/STATUS.md` skeleton, `deny.toml`.

## Phase 1 — `lux-core`: headless text engine

*Done.* `Buffer` on `ropey` with atomic save+backup and EOL/indent/encoding
detection; grapheme-aware positions/selections; first-class multicursor
with merge-on-overlap; transactional undo/redo; Sublime-style edit
operations and selections; buffer find/replace; navigable snippets;
`lux-core-cli` headless harness; ≥85% coverage + 30 min fuzzing.

## Phase 2 — Renderer & desktop shell (`v0.1.0`)

*Done.* `lux-platform` trait + `winit` backend; `lux-render` wgpu context
and own glyph atlas (`etagere` + `swash`, 4×1 subpixel); `parley` +
`fontique` text layout; `lux-ui` virtualized editor view; working desktop
IME (preedit + cursor area); `accesskit` wired from first render; Lux Dark
theme; event-driven render loop. Released as `v0.1.0`.

## Phase 3 — Syntax, navigation, command palette (`v0.2.0`)

*Done.* `lux-syntax` incremental tree-sitter across the 15 baseline
languages (grammars compiled into the binary); token/scope-based themes;
bracket pair colorization + real syntax-driven folding; per-language
auto-indent; Goto Anything (`Cmd+P`) with Sublime-style suffixes; unified
command palette; find/replace bar; tabs + status bar; minimap. Released as
`v0.2.0`.

## Phase 4 — Projects, files, global search (`v0.3.0`)

*Done.* `lux-workspace` with real multi-root and `.lux-project`; virtualized
tree file explorer; native FS watchers (no polling); Find in Files on an
embedded ripgrep engine; project-wide replace with diff preview; workspace
index fed by watchers; sessions/recovery; explicit huge-file and binary-file
handling. Released as `v0.3.0`.

## Phase 5 — Config, keymaps, personalization (`v0.4.0`)

*Done.* `lux-config` JSONC with layered precedence and hot reload; keymaps
as data with Sublime/VS Code/JetBrains presets; `.editorconfig` and
`.gitattributes` support; schema-aware settings importers; typography
settings with per-project/folder overrides; preferences UI. Released as
`v0.4.0`.

## Phase 6 — Git integration with `gix` (`v0.5.0`)

*Done.* In-process Git, no shell-out: gutter change indicators + hunk
navigation; tree changes panel + side-by-side diff; stage/unstage/commit/
amend; history + inline/commit blame; branches + stash; merge conflict
resolution. Released as `v0.5.0`.

## Phase 7 — Real LSP client (`v0.6.0`)

*Done.* Full server lifecycle management with visible per-server status;
incremental document sync; continuous diagnostics + problems panel;
completion with resolve; full navigation (definition/references/symbols);
hover/signature/inlay hints/semantic tokens; transactional code actions and
cross-file rename via `WorkspaceEdit`; formatting (document/range/
on-save); JSON-RPC stream fuzzer. Verified against `rust-analyzer`,
`typescript-language-server`, `pyright`, `gopls`, `clangd`. Released as
`v0.6.0`.

## Phase 8 — Real PTY terminal (`v0.7.0`)

*Done.* `lux-term` on `portable-pty` + `alacritty_terminal`.

- [x] Real PTY: persistent processes, signals, correct resize
      (`docs/receipts/phase-08-pty-real-portablepty-alacrittyterminal-senales.md`)
- [x] ConPTY on Windows
      (`docs/receipts/phase-08-conpty-windows.md`)
- [x] Scrollback, search, selection/copy
      (`crates/lux-term/src/scrollback.rs`,
      `docs/receipts/phase-08-scrollback-busqueda-seleccion-copia.md`)
- [x] Multiple named terminals + shell profiles
      (`crates/lux-term/src/sessions.rs`,
      `docs/receipts/phase-08-terminales-nombrados-perfiles-shell.md`)
- [x] Build-tool error-location link detection, with jump to file:line
      (`crates/lux-term/src/links.rs`,
      `docs/receipts/phase-08-deteccion-enlaces-file-line.md`)
- [x] Project task panel with real cancellation
      (`crates/lux-term/src/tasks.rs`,
      `docs/receipts/phase-08-panel-tareas-cancelacion.md`)
- [x] Phase close: VT conformance suite; 100 MB/s output without frame-time
      degradation; killing a terminal leaves no orphan processes; document
      explicitly that this capability will not exist on iOS/iPadOS
      (`docs/receipts/phase-08-cierre-vt-throughput-orphans.md`)

Released as `v0.7.0` (crate-complete; UI wiring is Phase 10).

## Phase 9 — WASM plugins with sandbox (`v0.8.0`)

*Done.* See ADR 0016 and `docs/plugins/API.md`.

- [x] `wasmtime` host with Component Model
- [x] Versioned extension API with explicit per-capability permissions
- [x] Real isolation: per-plugin CPU/memory limits and cancellation (fuel)
- [x] Concrete extension points (commands, grammars, themes, formatters,
      LSP server definitions, decorations)
- [x] Local install from file/folder + per-folder trust model
- [x] 2–3 example plugins
- [x] Phase close: sandbox-escape tests pass; API reference docs published

Release `v0.8.0`.

## Phase 10 — Accessibility, i18n, polish (`v0.9.0`)

*Done as code; screen-reader hardware passes remain STATUS `not-started`.*

- [x] Full keyboard navigation across all panels (F6 ring, Escape untrap)
- [ ] Complete `accesskit` verified with VoiceOver, NVDA, and Orca
      (tree exists since Phase 2; no VoiceOver/NVDA/Orca run here)
- [x] High-contrast themes + OS-respecting UI scaling
- [x] Reduced motion
- [x] Real i18n: string catalog + locale date/number formats, full
      Spanish and English
- [x] Real RTL verification in layout helpers (`direction_for`, start edge)
- [x] Phase close: WCAG AA on shipped themes; `docs/STATUS.md` honest
      (`docs/receipts/phase-10-a11y-i18n-chrome.md`)

Release `v0.9.0` (screen-reader verification is a follow-up on real AT).

## Phase 11 — Packaging & distribution (`v1.0.0`)

- [ ] Real macOS signing + notarization, universal `.dmg`
- [ ] Signed `.msi` + `winget` manifest (unsigned YAML stub in `packaging/winget/`)
- [ ] `.deb`, `.rpm`, AppImage, Flatpak; Homebrew formula
- [x] Auto-update **verify** (Ed25519); channel/server not hosted
      (`crates/lux-pack`, `docs/receipts/phase-11-pack-crash-update.md`)
- [ ] Reproducible CI releases with generated release notes
- [x] Local crash reports, **no automatic upload**
- [x] Simple website stub; `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, issue
      template
- [ ] Phase close: clean install on all desktop OSes; live auto-update;
      all PERFORMANCE.md budgets republished

`scripts/package.sh` lays out an **unsigned** tree.

## Phase 12 — iPadOS & iOS (`v1.1.0`)

- [x] Swift host owning `MTKView`, FFI names into Rust (`host-ios/`,
      `lux-platform::uikit`)
- [x] `UIKeyInput` + `UIKeyCommand` + `UIDocumentPicker` in the host
- [ ] IME candidate window, selection handles, Stage Manager, accesskit_ios
      on device
- [x] Capability profile helpers (`child_processes_allowed` is false on iOS)
- [ ] CI sign + App Store submission
- [ ] Phase close: App Store review, budgets on a real iPad

## Phase 13 — Android (`v1.2.0`)

- [x] Kotlin host with `SurfaceView` + JNI (`host-android/LuxActivity.kt`)
- [x] `onCreateInputConnection` (not keyboard-visibility)
- [x] SAF `ACTION_OPEN_DOCUMENT`
- [ ] `accesskit_android` on device; Play Store signed build
- [ ] Phase close: Android 8+ devices including Mali

Terminal/LSP UI are **not** in this host (GOALS / ADR 0006).
