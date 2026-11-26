# 0020. The tab row is the macOS title bar; fullscreen is native

## Status

Accepted, 2026-11-26.

## Context

macOS windows used the standard title bar: a 28 pt strip with the
traffic lights and the title, then Lux's 32 pt tab row under it. Sublime
Text puts the tabs in the title bar row with the traffic lights at their
left. Distraction-free mode asked winit for borderless fullscreen; on
macOS winit implements that with `toggleFullScreen:`, the native
fullscreen Space, but nothing in Lux offered plain fullscreen, and Lux
did not learn when the green button changed the state.

winit 0.30.13 can make the title bar transparent, extend the content
under it, and hide the title text. It has no API for the traffic lights'
frames or position, the user's title-bar double-click setting, or the end
of a fullscreen transition.

## Decision

- **Unified title bar.** On macOS every window is created with
  `titlebar_transparent`, `fullsize_content_view` and `title_hidden`. The
  tab row is drawn at y = 0 of the content. The title is still set, so
  Mission Control and the Window menu (now `NSApp.windowsMenu`) list it.
- **Traffic lights.** `lux-platform` reads the zoom button's frame and
  reports `PlatformWindow::title_bar_leading_inset`: its right edge plus
  12 pt, or 78 pt when AppKit returns no button, or 0 in fullscreen. The
  tab strip starts at `max(sidebar width, inset)`. The platform resizes
  the title-bar container to the tab row's height and centers the buttons
  on it, again on every `NSWindowDidResizeNotification`, fullscreen exit,
  and title change, which are when AppKit lays the title bar out anew.
- **Empty tab-row space** calls `PlatformWindow::title_bar_pressed`. On
  macOS a double click (the current `NSEvent`'s `clickCount`) zooms,
  minimizes, or does nothing per `AppleActionOnDoubleClick` (with the
  older `AppleMiniaturizeOnDoubleClick` as fallback; Zoom when neither is
  set). The defaults are read on a worker thread at window creation and
  whenever the app regains focus. Any other press starts winit's
  `drag_window`.
- **Fullscreen** is AppKit's: View > Enter Full Screen is a
  `toggleFullScreen:` item (Control-Command-F) with no target, so AppKit
  retitles it and does not add a second one. The `toggle_full_screen`
  keymap command and palette entry call the same winit fullscreen.
  `NSWindowDidEnter/ExitFullScreenNotification` become
  `PlatformEvent::FullscreenChanged`, sent once the transition settles,
  whatever started it. Distraction-free mode stays in native fullscreen,
  as in Sublime Text; leaving fullscreen by any route leaves it too.
- **Color.** The window background is the theme's tab-bar color and its
  appearance is Aqua or Dark Aqua by that color's contrast, so the
  traffic lights are drawn for the color behind them.
- All AppKit calls are in `lux-platform` (`desktop/macos_title_bar.rs`),
  which now depends on `objc2`, `objc2-foundation` and `objc2-app-kit`
  on macOS. Other platforms keep their system title bar; the new trait
  methods default to no-ops and a zero inset.

## Alternatives considered

A 28 pt tab row, so AppKit's own button position would be centered —
rejected: it changes the tab row on every platform and every layout test.
An `NSToolbar` to make the title bar taller — rejected: the unified
compact toolbar is not 32 pt, and a toolbar brings its own chrome.
Polling `is_fullscreen` to catch the green button — rejected by rule 5;
AppKit posts notifications.

## Consequences

A session saved before this change stored the content size below a
28 pt title bar; restoring it makes the window 28 pt shorter once. Moving
the buttons depends on AppKit's private view hierarchy (button →
`NSTitlebarView` → `NSTitlebarContainerView` → theme frame); if a macOS
release changes it, the lookups return `None` and the buttons stay where
AppKit puts them, with the inset still computed from their real frame.
`scripts/unified-title-bar.sh` checks the position on every macOS UI run.
