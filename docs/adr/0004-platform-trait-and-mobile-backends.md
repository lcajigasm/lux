# 0004. A `lux-platform` trait from day one; `winit` on desktop, hand-written backends on mobile

## Status

Accepted.

## Context

`winit` 0.30.13 is the solid desktop standard, but two verified facts rule it
out for mobile:

1. **`winit` cannot be embedded inside a `UIView` or a `SurfaceView`.** Its
   own maintainers say so explicitly — wrapping an arbitrary `Surface` "is
   not possible without completely rewriting the Android backend" — because
   `winit` owns `UIApplicationMain` and the `Activity` lifecycle itself.
2. **Its mobile IME support is shows/hide-only.** On Android, `Update` is a
   no-op; cursor-area and purpose hints are no-ops; there are long-standing
   open issues (#1823, #4126) about the keyboard not appearing and accented
   characters misbehaving. That is not sufficient for a text editor.

`winit` 0.31 has been in beta since November 2025 (still unreleased as of
2026-07-26) and includes changes aimed precisely at this problem: `Window`
becomes a trait, the `Event` enum disappears, and `resumed`/`suspended` are
split from `can_create_surfaces()` for portability to Android.

## Decision

Define `lux-platform` as a trait — window, surface, keyboard/mouse/touch
input, IME with preedit and cursor areas, clipboard, file dialogs, DPI/safe
area, lifecycle — from Phase 2, before any rendering code exists. Implement
it against `winit` 0.30.13 for desktop, but shape the trait's signatures like
`winit` 0.31 so that migrating later is a matter of swapping one crate. Write
hand-written implementations for UIKit (Phase 12) and Android/JNI (Phase 13);
do not attempt to force `winit` onto either.

## Alternatives considered

Waiting for `winit` 0.31 to stabilize before writing `lux-platform` — rejected
because 0.31 has had no release date for nine months as of this writing, and
even once stable it doesn't solve the mobile embedding problem, only the API
shape. Using `winit` unofficial mobile patches — rejected, no such patches
address the `UIView`/`SurfaceView` ownership issue, which is structural.

## Consequences

Desktop ships on a true, well-supported dependency. Mobile ships on code we
own and must maintain, but that code is the only way to keep this project's
promise of five platforms without being blocked on someone else's roadmap —
consistent with ADR 0002's reasoning about `gpui`. The trait boundary is also
what makes the winit→0.31 migration, whenever it lands, a single-crate change
instead of a rewrite.
