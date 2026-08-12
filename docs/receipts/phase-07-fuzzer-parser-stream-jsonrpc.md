# Receipt: Fuzzer del parser del stream JSON-RPC (Phase 7)

- Phase: 07
- Crate(s): lux-lsp (fuzz/, new subcrate standalone, same pattern as
  `crates/lux-core/fuzz/`; framing.rs, new module in the main crate)
- Goal: The "Fuzzer del parser del stream JSON-RPC" deliverable of Phase 7
  ("Cliente LSP real") from `LUX_PROMPT_MAESTRO.md` is complete and its
  gate passes with no failures and no ignored tests.
- Verify: `cargo fuzz run lsp_stream_fuzz -- -max_total_time=1800`
- Iterations: 5 / N (two real bugs found and fixed, then a clean 30-minute run)
- Result: passed
- Opened: 2026-08-12
- Closed: 2026-08-12
- Notes:

  **Closes out at `passed` after fixing what it found, per explicit
  project-owner direction ("Soluciona el bug") once the finding below was
  reported.** The fuzzer's first run found a genuine, reproducible crash
  within seconds; the honest response was to fix it in `lux-lsp`, not to
  narrow the harness until it stopped finding it. Final verification:
  **53,608,974 executions in 1801 seconds, zero crashes, zero artifacts.**

  **What was built.** `crates/lux-lsp/fuzz/` (standalone `cargo-fuzz`
  subcrate, empty `[workspace]` table, same isolation reasoning as
  `lux-core/fuzz`). `async-lsp`'s `Message::read` -- the Content-Length-
  framing + `serde_json` deserialization that parses every byte a
  language server sends -- is a private method with no public hook, so
  the target drives it through the real production path: raw fuzzer
  bytes wrapped by `lux_lsp::guard_stream` (this deliverable's own fix,
  see below), fed into `async_lsp::MainLoop::run_buffered` against a
  minimal, route-free `Router<()>`, driven synchronously with
  `futures::executor::block_on`. Raw `&[u8]` input, no `Arbitrary`-
  structured generation, deliberately -- doesn't pre-judge which
  malformed shapes are "worth" testing. Seeded
  `fuzz/corpus/lsp_stream_fuzz/` with small valid/near-valid example
  messages to bootstrap coverage past the "discover the `Content-Length:`
  header shape" barrier.

  **Environment note, same shape as Phase 1's fuzzing receipt.** `cargo
  fuzz` needs nightly; this machine's plain `cargo` resolves to Homebrew's
  pinned-stable 1.97.1. `cargo-fuzz`'s own target-probing step shells out
  to a bare `rustc` by name regardless of which `cargo` launched it, so
  neither `cargo +nightly fuzz` nor invoking the rustup-proxy `cargo`
  directly was enough. Fixed by prepending the nightly toolchain's own
  `bin/` directory to `PATH` for the fuzz commands specifically.

  **Finding #1: an unbounded allocation from an attacker- or bug-
  controlled `Content-Length`.** A 20-second sanity run crashed in
  ~79,000 executions: `async-lsp` 0.2.4's `Message::read` parses the
  `Content-Length` header and immediately does
  `vec![0u8; content_len]` with no upper bound before allocating -- a
  single header line (`Content-Length: 88888888888888\r\n\r\n`) aborts
  the whole client process, not the server. Reproduced deterministically
  twice. Checked for an easy escape hatch first: `async-lsp` 0.2.4 is the
  latest version on crates.io, and its source has no size-limiting
  option -- a live, unfixed characteristic of the dependency, and its
  `Message::read` is private, so it can't be wrapped or patched from
  outside. **Fix**: `crates/lux-lsp/src/framing.rs`, `guard_stream` --
  wraps the raw byte stream, re-parses just the framing layer (not the
  JSON-RPC payload, still entirely `async-lsp`'s job) as a transparent
  pass-through, and rejects any claimed `Content-Length` over 64 MiB
  (`MAX_MESSAGE_BYTES`) with a clean I/O error before an allocation is
  ever attempted -- also caps the header block itself at 64 KiB so an
  unterminated header can't grow unbounded either. Wired into
  `session.rs`'s real child-process stdout (`guard_stream(BufReader::
  new(stdout))`, replacing the raw `stdout` previously passed to
  `run_buffered`), and into the fuzz target itself, so both production
  and the fuzzer exercise the exact same two-layer chain.

  **Finding #2, from re-fuzzing after the fix: the guard's own header
  parsing didn't exactly match `async-lsp`'s.** A follow-up crash (a
  different, `Vec` "capacity overflow" panic, same `vec![0u8; ...]` call
  site) came from a two-line input: a well-formed `Content-Length` line
  with a huge-but-legitimately-parseable `usize` value (fits under
  `usize::MAX`, exceeds `isize::MAX`), followed by a malformed second
  line that only *looked* like another `Content-Length` header under the
  guard's original looser `split_once(':')` match. The guard treated the
  second line as a failed re-declaration and reset its tracked value back
  to "none found," forwarding the frame unguarded -- while `async-lsp`'s
  own stricter `split_once(": ")` (colon-space, not colon alone) never
  recognized that second line as `Content-Length` at all, so it still
  read the real, huge value from the first line and tried to allocate
  for it. **Fix**: rewrote the guard's header-line matching to be
  byte-for-byte identical to `async-lsp`'s own rule (exact `"\r\n"`
  terminator, exact `": "` separator), and to error out immediately on
  an unparseable `Content-Length` value instead of resetting state and
  continuing -- so a later line can never mask an earlier one's real
  value, by construction rather than by hoping the two parsers happen to
  agree. Pinned down with a regression test reproducing this exact input
  shape (`framing::tests::a_valid_huge_length_is_not_masked_by_a_later_malformed_header_line`).

  **Verified on macOS (aarch64-apple-darwin):** `crates/lux-lsp/src/
  framing.rs` unit tests (8, all pure, no process spawned) cover both
  findings directly plus well-formed pass-through, multi-frame streaming,
  a missing-Content-Length frame (left for `async-lsp`'s own stricter
  parser to reject), and clean EOF. `cargo fuzz run lsp_stream_fuzz --
  -max_total_time=60` (60s sanity, both after the first fix and again
  after the second): 0 crashes each time. Full gate,
  `cargo fuzz run lsp_stream_fuzz -- -max_total_time=1800`: **53,608,974
  executions in 1801 seconds, 0 crashes, 0 artifacts.** Full crate suite
  re-confirmed clean after the fix (`cargo test -p lux-lsp`, all targets),
  `cargo clippy -p lux-lsp --all-targets -- -D warnings` clean,
  `cargo fmt --check -p lux-lsp` clean, `./scripts/check-limits.sh` clean.
