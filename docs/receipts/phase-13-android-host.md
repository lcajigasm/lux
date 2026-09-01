# Receipt: Android Kotlin/JNI host (Phase 13)

- Phase: 13
- Crate(s): lux-platform
- Goal: Kotlin `Activity` with SurfaceView, real InputConnection, SAF
  `ACTION_OPEN_DOCUMENT`, JNI `nativeOpenUri`. No terminal/LSP UI in v1.2.
- Verify: `cargo test -p lux-platform android::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  Not a Play Store build. `android::available()` is false on macOS.
