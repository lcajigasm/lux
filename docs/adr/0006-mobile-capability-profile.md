# 0006. Mobile capability profile: no terminal UI, no LSP UI on iOS/iPadOS

## Status

Accepted. Re-confirm the App Store policy basis at the start of Phase 12 (see
Consequences) — the original research could not reach `developer.apple.com`.

## Context

iOS and iPadOS do not allow a sandboxed App Store app to create child
processes: `fork`, `exec`, and `posix_spawn` are unavailable. This has
direct, unavoidable consequences:

- **No LSP servers on iOS/iPadOS.** A language server is an external
  process. It cannot be launched.
- **No PTY terminal on iOS/iPadOS.** A PTY implies a child shell.
- **Git CLI is not needed anyway** — `gix` is an in-process library, and Git
  over HTTPS works fine (see ADR 0010).
- File access goes through `UIDocumentPicker` and security-scoped bookmarks,
  not free directory traversal.
- A plugin system that downloads and executes arbitrary code is legally
  fraught under App Store review; sandboxed WASM plugins are defensible,
  arbitrary script execution is not.

## Decision

Lux has a capability profile per platform, declared as data in code and
mirrored honestly in `docs/STATUS.md`. Desktop (v1.0) ships full terminal,
LSP, and Git. Mobile ships the full editor, Git, and syntax highlighting —
and explicitly **no terminal UI and no LSP UI at all**. Per golden rule #1,
if a capability doesn't exist on a platform, there is no button, menu item,
or panel for it on that platform — not a grayed-out one, not one that shows
an error when pressed.

If code intelligence on iPad becomes a goal later, the legitimate paths are
a remote LSP over the network or a WASM-compiled language server run
in-process — both are explicitly post-1.0 projects, not v1.1 scope.

## Alternatives considered

Shipping a degraded/simulated terminal or LSP indicator on mobile so the UI
"looks complete" across platforms — rejected outright; this is exactly the
failure mode `docs/adr/0001-lessons-from-v0.md` documents (UI implying
capability that isn't real).

## Consequences

`docs/STATUS.md` marks terminal and LSP UI as `excluded` (not `not-started`)
on iPadOS/iOS — this is a permanent platform constraint, not a queued task.
**Action required at the start of Phase 12**: re-verify current App Store
review rules before investing in that phase, since the assumption above is
based on stable, long-documented platform behavior but was not confirmed
against live Apple documentation during this research window.
