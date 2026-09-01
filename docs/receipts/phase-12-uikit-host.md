# Receipt: iOS UIKit host (Phase 12)

- Phase: 12
- Crate(s): lux-platform
- Goal: Swift host owns MTKView, UIKeyInput, UIKeyCommand, document picker;
  Rust `uikit` module exposes iOS-only FFI; ADR 0006 capability helpers.
- Verify: `cargo test -p lux-platform uikit::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  Not an App Store build. `uikit::available()` is false on macOS. STATUS
  store row stays `not-started`.
