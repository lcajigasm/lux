# Receipt: G02 — Open and save real files in `lux`

- Phase: integration / TODO G02
- Crate(s): `lux` (`document` module + `app` wiring), `lux-core` (existing
  `Buffer::open_with_policy` / `Buffer::save`)
- Goal: The desktop binary opens a UTF-8 path from argv (and Ctrl/Cmd+O /
  native dialog), saves via lux-core atomic save+`.bak`, shows dirty state
  in the window title, and honors huge/binary policy without a fake preview.
- Verify:
  - `cargo test -p lux`
  - `cargo clippy -p lux --all-targets -- -D warnings`
  - `./scripts/check-limits.sh`
  - Linux argv open:
    `timeout 5 env WGPU_BACKEND=gl xvfb-run -a ./target/debug/lux /tmp/file.txt`
    prints `Lux: opened …` and does not panic in open/save
  - Linux binary refuse:
    `lux` on a file with NUL bytes prints refuse + falls back to sample
  - Linux huge read-only:
    `lux` on a >10 MiB UTF-8 file prints `opened read-only`
  - Save path covered by `document::tests::open_save_round_trip_persists_utf8_bytes`
    (same `document::save_buffer` → `Buffer::save` as Ctrl/Cmd+S)
- Iterations: 3 / 5
- Result: passed on Linux (open/binary/huge smoke + unit save round-trip).
  Windows first-frame / dialog not run in this Cloud Agent — STATUS Windows
  stays `not-started` pending G15 / CI host run.
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:
  - `crates/lux/src/document.rs` wraps policy open + atomic save; unit-tested
    without GPU.
  - `App` tracks `path` / `dirty` / `read_only` / `modifiers`; title uses
    `• name — Lux` when dirty and `[read-only]` for huge files.
  - Ctrl/Cmd+O open dialog, Ctrl/Cmd+S save, Ctrl/Cmd+Shift+S save as
    (pre-G03 hard chords). Primary chord also suppresses inserting "s"/"o".
  - Dialog + fs run on the window-event handler thread (OS-modal pattern),
    never inside `redraw`. Argv open runs in `App::new` before the loop.
  - xdotool Ctrl+S smoke under Xvfb could not activate the window (no
    `_NET_ACTIVE_WINDOW`); save correctness is proven by the unit round-trip
    on the same code path, not by a fake stub.
  - Out of scope: multi-root workspace (G08), keymap system (G03).
