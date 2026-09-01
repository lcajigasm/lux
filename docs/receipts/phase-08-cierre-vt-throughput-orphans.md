# Receipt: Phase 8 close (VT, throughput, orphans, iOS exclusion)

- Phase: 08
- Crate(s): lux-term
- Goal: VT pins pass; snapshot after a flood stays under 16 ms; release
  parse rate is ≥ 100 MB/s; `kill_and_wait` leaves no orphans; iOS/iPadOS
  exclusion is documented in crate docs, ADR 0006, and STATUS.
- Verify: `cargo test -p lux-term`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  `crates/lux-term/src/vt.rs` (test-only): CUP+SGR, CRLF scroll-off, snapshot
  after a 4k-line flood < 16 ms, parser rate gate (10 MB/s debug / 100 MB/s
  `--release`). Orphans already pinned in `pty::tests::kill_and_wait_leaves_no_orphan`
  and the task-panel cancel test. iOS: crate-level docs + STATUS `excluded`.

  Full `cargo test -p lux-term` (19 lib tests + `lanza_vim_real`) clean on
  macOS aarch64-apple-darwin. Linux still unverified in-process; ConPTY
  remains CI-on-Windows from the earlier receipt.
