---
name: add-platform-backend
description: Checklist for implementing a new lux-platform backend (relevant to Phase 12 UIKit and Phase 13 Kotlin/JNI) — trait coverage, IME verification, accesskit wiring, and mobile process-model constraints.
---

# Add a `lux-platform` backend

Checklist for implementing the `lux-platform` trait for a new platform (see
`docs/adr/0004-platform-trait-and-mobile-backends.md`). `winit` already
covers desktop; this applies to UIKit (Phase 12) and Kotlin/JNI (Phase 13),
where the trait is implemented from scratch because `winit` cannot be
embedded in a `UIView` or `SurfaceView`.

1. **Cover every trait method** — window/surface lifecycle, keyboard/mouse/
   touch input, IME (preedit + cursor area, not just show/hide), clipboard,
   file dialogs, DPI/scaling, safe area. A partial implementation that
   compiles but silently no-ops a method is exactly the kind of "looks done"
   result golden rule #1 forbids — every method needs its own check, not just
   a build pass.
2. **Verify IME properly**, not just that the keyboard appears. Desktop
   already had this bar (Phase 2); mobile is harder because `winit`'s mobile
   IME being show/hide-only is the whole reason this backend exists. Test
   composition and confirmation with a real IME (e.g. an East Asian input
   method), not just Latin input.
3. **Wire `accesskit`** for the platform (`accesskit_ios` / `accesskit_android`)
   from the same PR that lands the backend, not after — retrofitting
   accessibility is expensive, building it in from the first render is not
   (see `docs/adr/0005-accessibility-from-phase-2.md`).
4. **Remember the mobile process-model constraint**: no `fork`/`exec`/
   `posix_spawn` on iOS/iPadOS. Do not let a platform backend PR add a
   command-palette entry, menu item, or code path that tries to launch an LSP
   server or a PTY on a platform where it's `excluded` in `docs/STATUS.md`
   (see `docs/adr/0006-mobile-capability-profile.md`). If you find yourself
   writing that code, stop — it belongs nowhere until it can actually run.
5. **Gate:** the backend's own crate tests, plus a manual test pass on real
   hardware for IME and accessibility (both require a human — document the
   result in the deliverable's receipt, per `docs/receipts/TEMPLATE.md`).

This skill is intentionally thin — expand it with real gotchas the first
time Phase 12/13 work actually starts.
