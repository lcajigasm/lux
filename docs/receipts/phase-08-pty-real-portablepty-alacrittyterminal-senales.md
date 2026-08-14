# Receipt: PTY real (portable-pty + alacritty_terminal), señales, resize (Phase 8)

- Phase: 08
- Crate(s): lux-term
- Goal: The "PTY real (portable-pty + alacritty_terminal), señales, resize"
  deliverable of Phase 8 ("Terminal PTY real") from `LUX_PROMPT_MAESTRO.md`
  is complete and its gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-term pty:: --test lanza_vim_real`
- Iterations: 1 / 12
- Result: passed
- Opened: 2026-08-14
- Closed: 2026-08-14
- Notes:

  **Scope.** `lux-term` (`crates/lux-term`) started as an empty crate — a
  `Cargo.toml` with a description and a zero-byte `src/lib.rs`, per ADR
  0012's decision to build it on `portable-pty` 0.9 (pty + ConPTY) and
  `alacritty_terminal` 0.26 (VT emulation), never a hand-rolled pty layer
  or parser. This loop adds exactly the three things the deliverable
  names — real pty-backed process spawn, signal handling, and resize —
  and nothing else in Phase 8's larger deliverable list (scrollback,
  search, selection, multiple named terminals/shell profiles, link
  detection, task panel). No `lux-ui` wiring; same headless-library
  boundary every other phase's receipts follow until a UI crate exists to
  consume it.

  **Design.** `Pty::spawn` opens a real OS pty via `portable-pty`'s
  `NativePtySystem`, spawns the child into the slave side, then drops the
  slave's own copy (the `bash.rs` example in `portable-pty` itself shows
  why: the child's inherited stdio is what keeps the slave alive; dropping
  ours is what lets the master reader see real EOF when the child exits,
  instead of hanging forever). A dedicated thread blocks on
  `reader.read()` — a real blocking syscall, not a polled interval, so
  golden rule #5 holds — and feeds every chunk through
  `alacritty_terminal::vte::ansi::Processor::advance` into a
  `FairMutex<Term<_>>` shared with the rest of the struct. `Pty::resize`
  updates the kernel's `TIOCSWINSZ` winsize and the VT grid's dimensions
  in the same call. `Pty::write` is the only "signal" API needed: control
  bytes like `0x03` become real `SIGINT` via the kernel's own tty line
  discipline, the same mechanism every real terminal relies on — no
  bespoke signal-sending method was added, since one would just
  reimplement what the kernel already does correctly.
  `Pty::kill_and_wait` sends `portable-pty`'s own `ChildKiller::kill`
  (`SIGHUP`, escalating to `SIGKILL` after a grace period — read from
  `portable-pty`'s own source, not assumed) and then blocks on `wait()` to
  actually reap the child, so no zombie or orphan is left behind. Errors
  are a small `thiserror` enum (matching `lux-lsp`'s established
  convention in this workspace, not `anyhow` in the public API) wrapping
  the `anyhow::Error` that `portable-pty`'s own trait methods return.

  **What got stuck, in order, and the actual fix — three real failures
  inside the single iteration, none of them repeats of each other:**
  1. The literal gate command, `cargo test -p lux-term pty::
     --test lanza_vim_real`, ran **zero** tests on the first try. Cargo
     parses `pty::` as a name filter and `--test lanza_vim_real` as
     "only this one integration-test binary" — the two combine as an
     intersection, not a union, and a top-level `fn lanza_vim_real()`
     doesn't contain the substring `pty::`. Fixed by nesting the test in
     `mod pty { #[test] fn lanza_vim_real() { ... } }` inside
     `tests/lanza_vim_real.rs`, so its qualified name is
     `pty::lanza_vim_real` — matches both the binary selector and the
     filter, and reads naturally next to the crate's own `pty::tests::*`
     unit tests.
  2. The first resize assertion (`pty.snapshot().lines().count() == 40`)
     failed by exactly one even though the grid really had resized to 40
     rows. Cause: `Vec<String>.join("\n")` on 40 rows where the last row
     (vim's blank command-line) trims to `""` produces a string that
     *ends* in `\n`, and `str::lines()` doesn't count a trailing empty
     segment after a final separator — a classic off-by-one in that API,
     not a bug in `resize`. Fixed by adding a direct `Pty::size() ->
     (usize, usize)` reading the grid's own dimensions instead of parsing
     rendered text, and separately strengthened the test to ask **vim
     itself** `:echo &columns . "x" . &lines` after resizing — proof the
     real `SIGWINCH` reached the child, not just that the local grid
     object changed.
  3. `control_byte_delivers_a_real_signal` asserted
     `status.signal() == Some("SIGINT")` and failed with
     `Some("Interrupt: 2")`. `portable-pty`'s `ExitStatus::signal()` is
     `libc::strsignal`'s text, which is platform/locale-dependent (macOS:
     "Interrupt: 2", Linux typically: "Interrupt") — not the symbolic
     name. Fixed by asserting `!status.success()` plus a
     case-insensitive `contains("interrupt")` instead of an exact string.

  **Verified on macOS (aarch64-apple-darwin).** Exact gate,
  `cargo test -p lux-term pty:: --test lanza_vim_real`, re-run 4 times —
  0 failures across all runs; `tests/lanza_vim_real.rs` spawns a real
  `vim -u NONE -n`, waits for its real intro screen, resizes the pty live,
  confirms `vim` itself observed the new size (proof of real `SIGWINCH`
  delivery, not just a locally-resized grid), sends `:q!`, and confirms a
  clean exit. Full crate suite, `cargo test -p lux-term` (4 unit tests in
  `pty::tests` + `lanza_vim_real`, 0 ignored): `spawn_and_echo_reaches_the_grid`
  (a real `/bin/sh` process's real stdout reaches the VT grid),
  `resize_reaches_the_kernel_winsize` (a real `stty size` inside a real
  shell reports the resized value back), `control_byte_delivers_a_real_signal`
  (a real `cat` actually dies from a real `SIGINT` delivered by the
  kernel, not a simulated one), `kill_and_wait_leaves_no_orphan`
  (`libc::kill(pid, 0)` returns `ESRCH` after `kill_and_wait`, confirming
  the child was actually reaped, not left as a zombie or orphan — the
  literal "matar el terminal no deja procesos huérfanos" acceptance
  criterion). Also verified: `cargo clippy -p lux-term --all-targets --
  -D warnings` (clean), `cargo fmt -p lux-term -- --check` (clean after
  one `cargo fmt` pass), `./scripts/check-limits.sh` (clean — `pty.rs` is
  well under the 600-line limit), `cargo build --workspace` (no
  regressions elsewhere — `lux-term` has zero graphics dependencies,
  consistent with golden rule #2 even though it isn't `lux-core` itself),
  and `git status --short` (only `crates/lux-term/*`, `Cargo.lock`, and
  `docs/` touched — no scope creep beyond this one deliverable).
