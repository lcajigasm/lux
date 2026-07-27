# Status

Source of truth for what actually works, per platform (golden rule #9).
Updated in the same PR that changes behavior. The README promises nothing
this file does not confirm.

Legend: `not-started` · `partial` (behind an `experimental.*` flag, listed
under Experimental below) · `implemented` · `excluded` (permanently out of
scope for that platform by design, not a gap to fill — see the linked ADR).

Phase 0 shipped no UI by design (the master prompt is explicit: "if there's a
window at the end of Phase 0, you moved too fast"). Phase 1 shipped a
complete, headless `lux-core` — see below.

## Core editing (Phase 1)

`lux-core` has no platform-specific code (that's the whole point of golden
rule #2); the "implemented" mark below reflects where it's actually been
*run* — the full test suite (235 tests + a proptest + 30 minutes of
`cargo-fuzz`), all on macOS (aarch64-apple-darwin). It's confirmed to
**cross-compile cleanly for `aarch64-apple-ios` and `aarch64-linux-android`**
(`cargo build -p lux-core --target ...`, no linker needed — it's an `rlib`,
no final binary) — Phase 1's specific portability check — but that's a build
check, not a behavioral one, so those columns stay `not-started` until the
suite actually runs there. Windows/Linux are untested for the same reason
(no non-macOS dev environment available yet); the code itself uses only
`std::fs`/`std::path`, nothing OS-specific.

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Buffer load/save (atomic + backup) | implemented | not-started | not-started | not-started | not-started |
| EOL / indent / encoding detection | implemented | not-started | not-started | not-started | not-started |
| Positions/selections (byte/char/column, grapheme-aware) | implemented | not-started | not-started | not-started | not-started |
| Multicursor + multiselection | implemented | not-started | not-started | not-started | not-started |
| Transactional undo/redo | implemented | not-started | not-started | not-started | not-started |
| Edit operations (indent, comment, duplicate, sort, case) | implemented | not-started | not-started | not-started | not-started |
| Movement (word/line/paragraph/bracket-match) | implemented | not-started | not-started | not-started | not-started |
| Sublime-style selections (expand, next occurrence, box) | implemented | not-started | not-started | not-started | not-started |
| Find/replace in buffer | implemented | not-started | not-started | not-started | not-started |
| Snippets with navigable placeholders | implemented | not-started | not-started | not-started | not-started |
| `lux-core-cli` headless test harness | implemented | not-started | not-started | not-started | not-started |

**Known gap**: the "open a 100 MB file" performance budget's RSS half (< 3×
file size) measures 3.16×, over budget — see `docs/PERFORMANCE.md`'s
measurement history for the root cause (three simultaneous copies of the
file's bytes during `Buffer::open`) and why it wasn't fixed in the same pass
that found it.

## Rendering & desktop shell (Phase 2, `v0.1.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Window/input/IME platform trait | not-started | not-started | not-started | not-started | not-started |
| GPU text rendering (glyph atlas, subpixel AA) | not-started | not-started | not-started | not-started | not-started |
| Desktop IME (preedit, cursor area) | not-started | not-started | not-started | n/a | n/a |
| Accessibility tree (`accesskit`) | not-started | not-started | not-started | not-started | not-started |
| Lux Dark theme | not-started | not-started | not-started | not-started | not-started |
| Virtualized editor view | not-started | not-started | not-started | not-started | not-started |

## Syntax & navigation (Phase 3, `v0.2.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Tree-sitter incremental highlighting | not-started | not-started | not-started | not-started | not-started |
| Token-based themes | not-started | not-started | not-started | not-started | not-started |
| Bracket pair colorization | not-started | not-started | not-started | not-started | not-started |
| Syntax-based code folding | not-started | not-started | not-started | not-started | not-started |
| Auto-indent by language | not-started | not-started | not-started | not-started | not-started |
| Goto Anything | not-started | not-started | not-started | not-started | not-started |
| Command palette | not-started | not-started | not-started | not-started | not-started |
| Search/replace bar | not-started | not-started | not-started | not-started | not-started |
| Tabs + status bar | not-started | not-started | not-started | not-started | not-started |
| Minimap | not-started | not-started | not-started | not-started | not-started |

## Projects, files & search (Phase 4, `v0.3.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Multi-root workspaces | not-started | not-started | not-started | not-started | not-started |
| File tree explorer | not-started | not-started | not-started | not-started | not-started |
| Native FS watchers | not-started | not-started | not-started | not-started | not-started |
| Find in files (ripgrep engine) | not-started | not-started | not-started | not-started | not-started |
| Replace in project with diff preview | not-started | not-started | not-started | not-started | not-started |
| Sessions & crash recovery | not-started | not-started | not-started | not-started | not-started |
| Huge/binary file handling | not-started | not-started | not-started | not-started | not-started |

## Configuration & personalization (Phase 5, `v0.4.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| JSONC config with layered precedence | not-started | not-started | not-started | not-started | not-started |
| Keymap presets (Sublime/VS Code/JetBrains) | not-started | not-started | not-started | not-started | not-started |
| `.editorconfig` / `.gitattributes` | not-started | not-started | not-started | not-started | not-started |
| Settings importers | not-started | not-started | not-started | not-started | not-started |
| Font settings + overrides | not-started | not-started | not-started | not-started | not-started |

## Git integration (Phase 6, `v0.5.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Gutter diff indicators | not-started | not-started | not-started | not-started | not-started |
| Diff panel (side-by-side/inline) | not-started | not-started | not-started | not-started | not-started |
| Stage/unstage/commit/amend | not-started | not-started | not-started | not-started | not-started |
| History & blame | not-started | not-started | not-started | not-started | not-started |
| Branches & stash | not-started | not-started | not-started | not-started | not-started |
| Merge conflict resolution | not-started | not-started | not-started | not-started | not-started |

## LSP client (Phase 7, `v0.6.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Server lifecycle management | not-started | not-started | not-started | excluded (no child processes on iOS — ADR 0006) | not-started |
| Incremental document sync | not-started | not-started | not-started | excluded | not-started |
| Diagnostics panel | not-started | not-started | not-started | excluded | not-started |
| Autocomplete | not-started | not-started | not-started | excluded | not-started |
| Navigation (definition/references/symbols) | not-started | not-started | not-started | excluded | not-started |
| Hover / signature help / inlay hints | not-started | not-started | not-started | excluded | not-started |
| Code actions + `WorkspaceEdit` | not-started | not-started | not-started | excluded | not-started |
| Cross-file rename | not-started | not-started | not-started | excluded | not-started |
| Formatting | not-started | not-started | not-started | excluded | not-started |

## Terminal (Phase 8, `v0.7.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Real PTY terminal | not-started | not-started | not-started | excluded (no child processes on iOS — ADR 0006) | not-started |
| ConPTY | n/a | not-started | n/a | n/a | n/a |
| Scrollback, search, selection | not-started | not-started | not-started | excluded | not-started |
| Multiple named terminals / shell profiles | not-started | not-started | not-started | excluded | not-started |
| Build-error link detection | not-started | not-started | not-started | excluded | not-started |
| Task panel with cancellation | not-started | not-started | not-started | excluded | not-started |

## Plugins (Phase 9, `v0.8.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| `wasmtime` plugin host | not-started | not-started | not-started | not-started (subject to App Review — ADR 0006) | not-started |
| Capability-scoped permissions | not-started | not-started | not-started | not-started | not-started |
| Sandboxed CPU/memory limits | not-started | not-started | not-started | not-started | not-started |
| Extension points (commands, grammars, themes...) | not-started | not-started | not-started | not-started | not-started |
| Local plugin install | not-started | not-started | not-started | not-started | not-started |

## Accessibility, i18n & polish (Phase 10, `v0.9.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Full keyboard navigation | not-started | not-started | not-started | not-started | not-started |
| Screen reader support | not-started (VoiceOver) | not-started (NVDA) | not-started (Orca) | not-started | not-started (TalkBack) |
| High contrast / UI scaling / reduced motion | not-started | not-started | not-started | not-started | not-started |
| i18n (es/en) | not-started | not-started | not-started | not-started | not-started |
| RTL layout | not-started | not-started | not-started | not-started | not-started |

## Packaging & distribution (Phase 11, `v1.0.0`)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Signed, notarized installer | not-started (.dmg) | not-started (.msi + winget) | not-started (.deb/.rpm/AppImage/Flatpak) | n/a (App Store, Phase 12) | n/a (Play Store, Phase 13) |
| Auto-update | not-started | not-started | not-started | n/a | n/a |
| Local crash reports (no auto-send) | not-started | not-started | not-started | not-started | not-started |

## Mobile shells (Phase 12 `v1.1.0`, Phase 13 `v1.2.0`)

| Capability | iPadOS/iOS | Android |
|---|---|---|
| Native platform-trait backend (UIKit / Kotlin+JNI) | not-started | not-started |
| Mobile text input (IME, selection handles, system menu) | not-started | not-started |
| External keyboard / trackpad / Stage Manager (iPad) | not-started | n/a |
| Sandboxed file access (`UIDocumentPicker` / SAF) | not-started | not-started |
| `accesskit_ios` / `accesskit_android` | not-started | not-started |
| App Store / Play Store signed build | not-started | not-started |

## Experimental

_Nothing is behind an `experimental.*` flag yet._
