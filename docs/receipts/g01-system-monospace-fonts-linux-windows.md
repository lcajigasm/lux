# Receipt: G01 system monospace fonts (Linux + Windows)

- Phase: integration / TODO G01 (post phase-2 shell)
- Crate(s): `lux`
- Goal: `cargo run -p lux` finds a real system monospace font on Linux and
  Windows (not only macOS paths) and does not panic inside `font::load_*`;
  failure mode stays honest.
- Verify:
  - `cargo test -p lux`
  - `cargo clippy -p lux --all-targets -- -D warnings`
  - Linux: `timeout 5 env WGPU_BACKEND=gl xvfb-run -a ./target/debug/lux`
    must print `Lux: using system font …` and must **not** panic in
    `font::` (exit 124 from `timeout` is success for this check)
  - Windows: `cargo test -p lux` on `windows-latest` (CI) must pass the
    desktop font assertion in `font::tests`
- Iterations: 2 / 5
- Result: passed (Linux font + event-loop smoke); Windows first-frame
  binary not run on this host — recorded below
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:
  - Replaced macOS-only candidate list with `cfg(target_os=…)` lists for
    macOS / Linux / Windows; Linux adds one-shot `fc-match` without linking
    `yeslogic-fontconfig-sys` (DEPENDENCIES conflict); Windows also probes
    `%WINDIR%\Fonts` and `%LOCALAPPDATA%\…\Fonts`.
  - Bytes are validated with `swash::FontRef::from_index` before accept.
  - Linux Cloud Agent: loaded
    `/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf`. Bare
    `DISPLAY=:1 ./lux` then panicked in `render.rs` (no GPU adapter).
    Under `xvfb-run` + `WGPU_BACKEND=gl`, process stayed up until timeout
    (no panic) — font path cleared; full Linux Phase 2 STATUS still not
    claimed (ADR 0017).
  - Windows: loader + unit tests land in this PR; no Windows desktop
    session here — do **not** flip Windows STATUS shell rows (G15).
  - Out of scope per TODO: bundling a font (possible future ADR).
