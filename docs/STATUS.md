# Status

Source of truth for what actually works, per platform (golden rule #9).
Updated in the same PR that changes behavior. The README promises nothing
this file does not confirm.

Legend: `not-started` · `partial` (behind an `experimental.*` flag, listed
under Experimental below) · `implemented` · `excluded` (permanently out of
scope for that platform by design, not a gap to fill — see the linked ADR).

As of Phase 0, nothing is implemented. No UI exists yet by design (the master
prompt is explicit: "if there's a window at the end of Phase 0, you moved too
fast").

## Core editing (Phase 1)

| Capability | macOS | Windows | Linux | iPadOS/iOS | Android |
|---|---|---|---|---|---|
| Buffer load/save (atomic + backup) | not-started | not-started | not-started | not-started | not-started |
| EOL / indent / encoding detection | not-started | not-started | not-started | not-started | not-started |
| Multicursor + multiselection | not-started | not-started | not-started | not-started | not-started |
| Transactional undo/redo | not-started | not-started | not-started | not-started | not-started |
| Edit operations (indent, comment, duplicate, sort, case) | not-started | not-started | not-started | not-started | not-started |
| Movement (word/line/paragraph/bracket-match) | not-started | not-started | not-started | not-started | not-started |
| Sublime-style selections (expand, next occurrence, box) | not-started | not-started | not-started | not-started | not-started |
| Find/replace in buffer | not-started | not-started | not-started | not-started | not-started |
| Snippets with navigable placeholders | not-started | not-started | not-started | not-started | not-started |

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
