# Receipt: Build-error link detection (Phase 8)

- Phase: 08
- Crate(s): lux-term
- Goal: rustc `--> path:line:col`, gcc/clang `path:line:col:`, and Python
  `File "path", line N` in terminal text become `SourceLocation` values.
- Verify: `cargo test -p lux-term links::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  `detect_links` + `Pty::buffer_text` / `Pty::detect_links`. Jump is
  returning path+line+column, not opening a buffer. URLs with `://` are
  skipped. ponytail: three patterns, not a compiler-output grammar.

  Unit test covers the three shapes; a real `/bin/sh` printf of a rustc
  arrow is detected on the live PTY. `cargo test -p lux-term links::` clean.
