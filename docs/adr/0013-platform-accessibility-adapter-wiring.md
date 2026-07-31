# 0013. `accesskit_winit` owned inside `lux-platform`'s desktop backend, exposed as plain `PlatformEvent`/`PlatformWindow` surface

## Status

Accepted.

## Context

ADR 0005 commits to wiring `accesskit` in starting with Phase 2's first
working render, not deferring it. Standing up the first unified `lux`
binary (this phase's closing piece) is that first working render, so it
must expose a real OS accessibility tree, not defer the question again.

`accesskit_winit::Adapter` is the sanctioned way to attach a real platform
accessibility adapter to a `winit` window (a raw `accesskit_macos::Adapter`
alone doesn't receive OS queries without NSView subclassing, which
`accesskit_winit` already does internally). But its constructors
(`with_direct_handlers`, `with_event_loop_proxy`) take concrete
`winit::event_loop::ActiveEventLoop` and `winit::window::Window` — and
`lux-ui` and everything above `lux-platform` must never depend on `winit`
directly (golden rule #2's sibling boundary for this crate, documented in
`lux-platform/src/lib.rs`). Something has to own the concrete `winit` types
and translate.

Two shapes were possible: expose enough raw `winit`/`accesskit_winit` types
through the trait for a caller to build its own adapter (leaks the
boundary), or have the desktop backend build and drive the adapter itself,
surfacing only `accesskit`-typed events and one new window method (keeps the
boundary, costs one more internal subsystem in `desktop/`).

## Decision

`lux-platform`'s desktop backend owns one `accesskit_winit::Adapter` per
window, built via `Adapter::with_event_loop_proxy` at window-creation time
(before the window is shown, per that constructor's own requirement — every
window is now created invisible internally and made visible immediately
after, regardless of what the caller's `WindowAttributes::visible` was
already going to do). The event loop's user-event channel — previously
unused (`EventLoop<()>`) — now carries `accesskit_winit::Event`, decoded in
`ApplicationHandler::user_event` into three new backend-agnostic
`PlatformEvent` variants (`AccessibilityInitialTreeRequested`,
`AccessibilityActionRequested(accesskit::ActionRequest)`,
`AccessibilityDeactivated`), delivered through the exact same
`PlatformEventHandler::window_event` callback every other input event
already uses — no new handler trait method needed. `PlatformWindow` gains
one new method, `update_accessibility_tree(&self, update:
accesskit::TreeUpdate)`, which the handler calls in response to
`AccessibilityInitialTreeRequested` (and again on every content/cursor
change, per ADR 0005's "kept in sync as part of the same change"
consequence).

`lux-platform` takes `accesskit` (the core, backend-agnostic types crate,
already a `lux-ui` dependency at the same 0.24.1) as a real dependency —
these are plain data types, not a `winit`-shaped API, so this doesn't
reopen the boundary the way exposing `accesskit_winit`'s own types would.
`accesskit_winit` itself stays internal to `desktop/`, never re-exported.

`with_event_loop_proxy` (not `with_direct_handlers`) specifically because
its handlers run through the same single-threaded event-loop dispatch every
other `PlatformEvent` already goes through — `with_direct_handlers`'
callbacks "may be called on any thread, depending on the underlying
platform adapter," which would require the handler to be `Send` and
thread-safe for no reason every other part of this trait doesn't already
need. The one real cost: `request_initial_tree` can't return synchronously
over this channel, so accesskit's own platform adapters show a temporary
placeholder tree until the first `update_accessibility_tree` call lands —
an inherent, documented characteristic of the proxy approach, not a bug to
route around.

## Alternatives considered

`with_direct_handlers`, called directly from `lux-ui` or the application
crate bypassing `lux-platform` (what the `accessibility_smoke` example
already does, deliberately, as an example-only shortcut) — rejected for the
actual product binary because it would mean the primary `lux` binary reaches
past the platform boundary that exists specifically so a future UIKit/JNI
backend doesn't have to re-implement this from scratch; an example
bypassing the boundary to test `lux-ui::accessibility::build_tree` in
isolation is a different, narrower claim than the shipping app doing it.

## Consequences

Every `lux-platform` desktop window is accessible by default, matching ADR
0005's "not opt-in" spirit — no capability flag turns this off today.
Mobile backends (Phase 12/13) must each do their own equivalent wiring
(`accesskit_ios`/`accesskit_android`), already anticipated by ADR 0005's own
platform list; this ADR only settles the desktop shape. The placeholder-tree
window between adapter construction and the first real `update_accessibility_tree`
call is a real, small window every caller must close promptly (build and
send the real tree as soon as `AccessibilityInitialTreeRequested` arrives),
not something this ADR papers over.
