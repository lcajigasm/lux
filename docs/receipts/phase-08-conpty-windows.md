# Receipt: ConPTY en Windows (Phase 8)

- Phase: 08
- Crate(s): lux-term
- Goal: The "ConPTY en Windows" deliverable of Phase 8 ("Terminal PTY real")
  from `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no
  failures and no ignored tests.
- Verify: `cargo test -p lux-term --target x86_64-pc-windows-msvc conpty::`
- Iterations: 9 / 10
- Result: passed
- Opened: 2026-08-14
- Closed: 2026-08-18
- Notes:

  **Blocked immediately, on this machine, for a reason no amount of
  iteration fixes: this is a macOS box with no way to execute a Windows
  binary.** `cargo test` doesn't just compile — it runs the resulting
  test binary and reports what actually happened. ConPTY
  (`CreatePseudoConsole`) is a real Windows Console API; `portable-pty`'s
  `NativePtySystem` already resolves to its own `win::conpty::ConPtySystem`
  on Windows (confirmed by reading `portable-pty`'s own source,
  `src/lib.rs`: `#[cfg(windows)] pub type NativePtySystem =
  win::conpty::ConPtySystem;`), so `Pty::spawn` from the prior deliverable
  would already exercise it *if run on Windows*. There is no code to write
  that makes that runnable here — writing `conpty`-specific tests and
  never running them would mean shipping unverified assertions about
  behavior nobody on this machine can observe, exactly what golden rule
  #1 ("nothing simulated ... a stub that returns plausible data is worse
  than a compile error") and rule #10 exist to forbid.

  **What was actually checked before concluding this, in order:**
  1. `wine` / `wine64`: not installed. No way to run a Windows PE binary
     on this Darwin host at all.
  2. `rustup target list --installed`: `x86_64-pc-windows-msvc` was not
     present; `x86_64-pc-windows-gnu` was, but the gate names `-msvc`
     specifically (real ConPTY tests should run under the toolchain a
     real Windows user actually ships with, not a cross-compiled GNU
     substitute), and neither one solves the execution problem below
     regardless.
  3. `rustup target add x86_64-pc-windows-msvc` succeeded (rust-std
     downloaded), but `cargo build -p lux-term --target
     x86_64-pc-windows-msvc` still failed with `error[E0463]: can't find
     crate for 'core'`. Root cause, confirmed via `which cargo` /
     `rustup show`: this repo's `rust-toolchain.toml` pins a
     rustup-managed toolchain, but the `cargo`/`rustc` actually first on
     `PATH` are Homebrew's own separate install
     (`/opt/homebrew/bin/{cargo,rustc}`, not rustup's shims), so
     `rustup target add` installed the target into a rustup toolchain
     directory that the `cargo` binary actually being invoked never
     reads. This is a real, fixable local environment gap — but fixing it
     only reaches "can cross-compile," never "can execute," which is what
     the gate's `cargo test` requires. Not pursued further since it
     wouldn't change the outcome.
  4. Even a hypothetical working MSVC cross-toolchain (e.g. via
     `cargo-xwin`, which downloads the MSVC CRT/SDK) only produces a
     Windows `.exe`; running it still needs Windows or Wine, neither
     available.

  **What would actually unblock this:** real execution on Windows. Three
  concrete options, roughly in order of how much they're already set up
  in this repo:
  1. **The existing CI matrix.** `.github/workflows/ci.yml`'s `test` job
     already runs `cargo test --workspace` natively on `windows-latest`
     for every push/PR. Once ConPTY-pinning tests exist in `lux-term`,
     that job is the real, honest place they get verified — not this
     machine. This receipt's code (if written) would need to go through
     that CI run before it could honestly be marked `passed` here.
  2. **A Windows VM or physical Windows machine reachable from this
     session** (Parallels/UTM locally, a cloud Windows instance, or
     hardware the user has) — if one exists, driving it directly would
     let this loop iterate normally instead of round-tripping through CI.
  3. **`cargo-xwin` for build-time-only sanity** (compiles against a real
     MSVC CRT without Windows) — useful to catch cross-compilation
     mistakes early, but explicitly not a substitute for the gate itself,
     which requires running the tests.

  **Iteration 2: wrote the tests, still refused to mark this `passed`.**
  After the above, `crates/lux-term/tests/conpty.rs` was added:
  `#![cfg(windows)]` at the file root, so it compiles to zero tests on
  every non-Windows target (confirmed: `cargo test -p lux-term --test
  conpty` on this macOS machine reports `0 passed; 0 failed` — an honest
  empty result, not a fake pass) and only exists for real once actually
  compiled for Windows. Three tests, mirroring the `pty::tests` module
  from the prior deliverable but against `cmd.exe` instead of Unix
  shells, nested in `mod conpty` so their qualified names satisfy the
  gate's `conpty::` filter directly (no `--test` binary restriction
  needed this time, unlike the previous deliverable):
  `conpty::spawns_a_real_process_through_conpty`,
  `conpty::resize_reaches_the_real_conpty_pseudoconsole`, and
  `conpty::kill_and_wait_reaps_a_conpty_child_without_hanging`. No new
  production code was needed in `pty.rs` itself — `portable-pty`'s
  `NativePtySystem` already switches to `win::conpty::ConPtySystem` under
  `#[cfg(windows)]` on its own, so the same `Pty` API from the prior
  deliverable already goes through real ConPTY on Windows; this file only
  adds the pinning tests ADR 0012 asks for. The `kill_and_wait` test's
  expectations (`TerminateProcess`, exit code 1, no signal name) were
  read directly from `portable-pty`'s own `src/win/mod.rs`
  (`WinChild`/`WinChildKiller`), not guessed — a real difference from the
  Unix `ChildKiller` path (`SIGHUP` → grace period → `SIGKILL`) worth
  knowing in advance. `cargo check -p lux-term --tests`, `cargo fmt -p
  lux-term -- --check`, and `cargo clippy -p lux-term --all-targets --
  -D warnings` all stay clean on macOS with this file present, and the
  full `pty::`/`lanza_vim_real` suite from the prior deliverable is
  unaffected (re-ran clean).

  This is real, reviewable code, not a stub — but it has never executed,
  so **the result stays `stuck`, not `passed`**, and `docs/STATUS.md`'s
  ConPTY row was deliberately left untouched (`not-started`). Marking
  either "done" on the strength of a compile-only check on the wrong
  platform would be exactly the simulated-feature failure golden rule #1
  exists to prevent — Windows-specific behavior needs a Windows run to
  say anything true about it. What was still not done, on purpose:
  pushing a branch to trigger the repo's own `windows-latest` CI job
  without asking first (a shared-state action outside this loop's
  authorization) and inventing exact console-output text this session
  has never observed (the `mode con`/PowerShell-size-echo idea considered
  during design was dropped in favor of asserting `Pty::size()` directly,
  which is verifiable from the same source-level reasoning as the
  `kill` behavior above, rather than guessed output formatting).
  Stopping there, at iteration 2, rather than spending the remaining
  budget rewriting tests this environment still cannot run, per the
  working agreement's rule #10 — until the user explicitly authorized
  pushing this branch and, separately, opening a PR against `main` so the
  repo's own `windows-latest` CI job could run it for real.

  **Iteration 3: the user's CI run found a real bug, on the first try —
  in the wrong deliverable's tests.** After the branch
  (`phase-8-terminal-pty-real`) and PR (#6) went up, `cargo test
  --workspace` on `windows-latest` failed, but not in `conpty::` — it
  never got that far. The prior deliverable's `pty::tests` module (all
  four tests, unconditionally spawning `/bin/sh`/`/bin/cat`) isn't
  cfg-gated, and `cargo test` stops running further test binaries in a
  crate after its `--lib` target fails, so `tests/conpty.rs` never even
  ran. The real error, straight from the CI log:
  `Spawn(CreateProcessW "/bin/cat\0" ... failed: The system cannot find
  the path specified. (os error 3))` — exactly what spawning a Unix path
  on Windows should do, not a ConPTY bug. (`ubuntu-latest` also failed in
  the same run, but on an unrelated, pre-existing flake in
  `crates/lux-config/src/layering/watch.rs` — out of scope for this
  deliverable, left untouched.) Fixed by gating the whole `mod tests` in
  `crates/lux-term/src/pty.rs` behind `#[cfg(all(test, unix))]` — the
  same class of fix this repo's own history already established for
  `lux-lsp` (`fix(lux-lsp): gate libc::kill/SIGKILL crash tests to
  #[cfg(unix)]`, in this branch's own git log). This narrowly touches the
  prior deliverable's file, but only to make *this* deliverable's gate
  reachable at all on Windows — `conpty::` can never run if the lib
  target it shares a crate with won't even build its test binary.
  Re-verified clean on macOS: `cargo test -p lux-term` (4 unit tests +
  `lanza_vim_real`, unaffected), `cargo fmt -p lux-term -- --check`,
  `cargo clippy -p lux-term --all-targets -- -D warnings`. Pushed as a
  follow-up commit to the same PR (#6); re-running CI to see whether
  `conpty::` itself passes once it can actually execute is the next
  step, not yet done as of this point in the loop.

  **Iteration 4: the re-run found a real deadlock — a 6-hour CI job, not
  a quick failure.** `windows-latest` came back `fail` after
  `6h0m6s` (GitHub's default job ceiling; it was canceled, not finished):
  `conpty::resize_reaches_the_real_conpty_pseudoconsole` passed,
  `conpty::spawns_a_real_process_through_conpty` failed after exactly the
  test's own 5s timeout, and
  `conpty::kill_and_wait_reaps_a_conpty_child_without_hanging` hung
  ("has been running for over 60 seconds") until the job was killed.
  `ubuntu-latest` failed again too, but on the exact same pre-existing
  `crates/lux-config` flake as iteration 3's run — confirmed by comparing
  the panic location (`watch.rs:266`) byte-for-byte across both runs;
  still out of scope, still untouched.

  Root-caused the hang from `portable-pty`'s own Windows source
  (`src/win/conpty.rs`), not guessed: `ConPtyMasterPty` and
  `ConPtySlavePty` share the *same* `Arc<Mutex<Inner>>`, where `Inner`
  owns the `PsuedoCon` (the actual `HPCON` from `CreatePseudoConsole`).
  Dropping `pair.slave` right after spawn — the exact pattern that works
  correctly on Unix, borrowed from `portable-pty`'s own `bash.rs`
  example — is a complete no-op on Windows: it just decrements a
  refcount, since `Inner`'s real teardown (`ClosePseudoConsole`, in
  `PsuedoCon`'s `Drop`) only happens when the *last* reference goes away.
  `Pty` keeps `master` alive for its entire lifetime (`resize` needs it),
  so `Pty::kill_and_wait`'s prior implementation — waiting for the child,
  then joining the reader thread while `self` (and therefore `master`)
  was still alive — could never let the pseudoconsole close, so the
  reader thread's blocking read on the ConPTY output pipe could never see
  EOF. A real, structural deadlock specific to the ConPTY backend, exactly
  the kind of "portable-pty Windows quirk" ADR 0012 asked this phase to
  pin. Fixed by destructuring `self` inside `kill_and_wait` and
  explicitly dropping `writer` and `master` — releasing the last `Inner`
  reference, triggering `ClosePseudoConsole` — *before* joining the
  reader thread. Confirmed harmless on Unix (where EOF already doesn't
  depend on `master`'s lifetime): `kill_and_wait_leaves_no_orphan` and
  the rest of the crate suite stayed green across 3 re-runs on macOS
  after the rewrite.

  For the `spawns_a_real_process_through_conpty` failure, ruled out one
  real candidate cause before treating it as a timing issue:
  `filedescriptor` (the crate `portable-pty` builds its Windows pipes on)
  uses plain `CreatePipe` — an anonymous, always-synchronous pipe, never
  `FILE_FLAG_OVERLAPPED` — so a blocking `.read()` on it is the correct,
  supported way to consume it; there's no overlapped-I/O misuse to fix.
  `resize_reaches_the_real_conpty_pseudoconsole` (same run) spawned
  `cmd.exe` via the same ConPTY path and returned in well under a second
  when it didn't also wait on output, meaning ConPTY/`conhost.exe`
  creation itself isn't the slow part — specifically waiting for output
  to travel child → ConPTY → pipe → reader thread → VT parser is what hit
  the wall. GitHub's own Windows runners are documented to be
  meaningfully slower than Linux/macOS ones for process-spawn-heavy
  work, which fits a first-ConPTY-use cold start better than a
  structural bug — but this is reasoned, not proven the way the deadlock
  fix is. Raised the test's timeout from 5s to 20s as a defensive,
  honest improvement, not a confirmed fix; documented here as such and
  in the STATUS.md scope note (see below) — this receipt does not claim
  `spawns_a_real_process_through_conpty` is fixed, only that the deadlock
  is, until the next real Windows run says otherwise.

  Re-verified clean on macOS after both changes: `cargo test -p lux-term`
  (7 unit tests + `lanza_vim_real`, 3 re-runs, 0 failures), `cargo clippy
  -p lux-term --all-targets -- -D warnings`, `cargo fmt -p lux-term --
  --check`, `./scripts/check-limits.sh`, `cargo build --workspace`.
  Pushed as a second follow-up commit to PR #6.

  **Iteration 5: the deadlock fix held — `kill_and_wait_reaps_a_conpty_
  child_without_hanging` now passes, in `7m22s` total instead of a
  6-hour cancellation. `spawns_a_real_process_through_conpty` still
  failed, but the 20s timeout ruled out iteration 4's "maybe it's just
  cold-start latency" theory: it used the full 20 seconds and still
  found nothing — `pty.snapshot()` came back completely blank, all 24
  rows, not partial or garbled output. That's a real, distinct data-flow
  bug, not a timing one, and one this session cannot diagnose further by
  reading source alone: `spawn_command`'s `CreateProcessW` call clearly
  succeeded (no panic there), `resize_reaches_the_real_conpty_
  pseudoconsole` proves ConPTY creation itself isn't the problem, and
  `filedescriptor`'s plain (non-overlapped) `CreatePipe` was already
  ruled out as a misuse candidate in iteration 4. The remaining
  candidates — the reader thread never actually starting, its `read()`
  call erroring immediately, or something specific to how `cmd.exe`'s
  output reaches the pipe under `PSEUDOCONSOLE_WIN32_INPUT_MODE` — can't
  be told apart from a blank grid alone, and `Pty`'s reader thread was
  silently swallowing `Err` the same as clean `Ok(0)` EOF, discarding
  exactly the information that would disambiguate them.

  Fixed that silent swallowing on its own merits, not as a throwaway
  debug hack: a real I/O error ending the reader thread should leave a
  trace instead of the terminal just quietly going stale forever with no
  signal to anything watching it. `crates/lux-term/src/pty.rs`'s reader
  loop now `eprintln!`s the actual `io::Error` before breaking, which
  `cargo test` captures and surfaces automatically on the next failure —
  no `--nocapture` needed. This is the fifth iteration touching
  Windows-specific behavior on this deliverable; each one has found and
  fixed something concrete and different (Unix-only test gating, a real
  ConPTY deadlock, and now instrumenting an unexplained data-flow gap) —
  not a repeat of the same failure, so continuing was still warranted
  under the "three repeats" rule, but if this round's real error message
  doesn't point to a fixable, confident root cause, the honest move next
  is to stop and report rather than keep guessing blind.

  Re-verified clean on macOS: `cargo test -p lux-term` (7 unit tests +
  `lanza_vim_real`), `cargo clippy -p lux-term --all-targets --
  -D warnings`, `cargo fmt -p lux-term -- --check`. Pushed as a third
  follow-up commit to PR #6; the real error text from the next
  `windows-latest` run is what determines whether this deliverable can
  actually close.

  **Iteration 6: the instrumentation ran, and printed nothing — which is
  itself the finding.** `kill_and_wait_reaps_a_conpty_child_without_
  hanging` and `resize_reaches_the_real_conpty_pseudoconsole` both still
  pass, quickly (job total `8m28s`, no hang). `spawns_a_real_process_
  through_conpty` failed again, using the full 20s, grid still
  completely blank — but the reader thread's new `eprintln!` on the
  `Err` path never fired. Ruled out: cmdline quoting
  (`CommandBuilder::cmdline`'s Windows quoting produces `cmd.exe /C
  "echo hello-lux-conpty"`, the standard, documented `cmd /c "..."`
  invocation shape, not a malformed one). Added one more diagnostic in
  the same spirit as iteration 5, not yet pushed and observed as of this
  entry: the `Ok(0)` (clean EOF) branch now also `eprintln!`s, with a
  running byte counter, so the next real Windows run distinguishes "the
  pipe closed almost immediately, before or without ever delivering
  `cmd.exe`'s output" from "the reader is still blocked in `read()` when
  the test gives up and the process exits" — the two remaining
  explanations this receipt cannot currently tell apart. Re-verified
  clean on macOS (`cargo test -p lux-term`, `cargo clippy`, `cargo fmt
  --check`).

  **This is the boundary.** Six iterations went into this one
  deliverable, three of them chasing this single test's failure
  specifically. Each round ruled out a real, distinct candidate
  (Unix-only test paths, the ConPTY-shared-`Inner` deadlock, overlapped
  I/O misuse, cmdline quoting, a swallowed I/O error) rather than
  repeating the same guess — genuine progress, not a stall — but this
  session has no way to attach a debugger or add a breakpoint on real
  Windows; every remaining hypothesis can only be tested by pushing,
  waiting roughly ten minutes for a CI round-trip, and reading whatever
  text comes back.

  **Iteration 7: the promised boundary. Neither `eprintln!` fired —
  which is itself the answer.** The run this session was waiting on
  (`windows-latest`, job `95646925207`, `8m18s`) came back with
  `resize_reaches_the_real_conpty_pseudoconsole` and
  `kill_and_wait_reaps_a_conpty_child_without_hanging` both still
  passing, and `spawns_a_real_process_through_conpty` still failing —
  but with **no `"lux-term: pty reader"` line anywhere in the log**,
  neither the `Ok(0)`/EOF branch added in iteration 5 nor the `Err`
  branch added in iteration 6. That rules out both remaining
  explanations this receipt was choosing between: the pipe did not close
  early (no EOF message), and the read did not fail (no error message).
  The only account left standing: the reader thread's `reader.read(&mut
  buf)` call itself blocks forever on real Windows for this exact
  scenario, receiving zero bytes, for the full 20 seconds this session
  waited and (since the thread is a plain detached `std::thread::spawn`,
  outliving the test's own timeout) indefinitely beyond that too — a
  real, live gap in `Pty::spawn`'s data path on Windows, not a timing
  artifact, not a swallowed error, not a quoting bug.

  Per this session's own stated boundary, stopping here rather than
  attempting a seventh guess with no new evidence to narrow it further.
  What was ruled out, with the evidence for each, so the next attempt
  doesn't repeat this session's work: overlapped I/O misuse
  (`filedescriptor` uses plain `CreatePipe`, never
  `FILE_FLAG_OVERLAPPED` — a blocking `.read()` is the documented,
  correct way to consume it); cmdline quoting (`CommandBuilder::cmdline`
  produces `cmd.exe /C "echo hello-lux-conpty"`, the standard, documented
  `cmd /c "..."` invocation shape); the ConPTY control channel itself
  (`resize_reaches_the_real_conpty_pseudoconsole`'s real, HRESULT-checked
  `ResizePseudoConsole` call succeeds, so `CreatePseudoConsole` and the
  pseudoconsole object are genuinely alive and responsive — only the
  *output data* channel is silent); a swallowed I/O error or an
  early-but-unlogged EOF (both paths now instrumented, neither fired).
  What remains unverified and untested: whether `try_clone_reader`'s
  handle duplication timing relative to `spawn_command` matters on
  Windows specifically (this session's ordering — clone after spawning
  and after dropping `slave` — was never varied and re-tested, since each
  variant costs a full CI round-trip); whether this is specific to
  GitHub Actions' Windows Server runner image (antivirus/console-hook
  interference with anonymous pipes is a documented class of issue on
  hosted CI runners, though unconfirmed here); and whether comparing
  against `wezterm`'s own production usage of this exact same
  `portable-pty` version would surface a required step this
  implementation is missing.

  **What this means for the deliverable.** Two of `lux-term`'s three
  ConPTY-specific behaviors are now genuinely verified on real Windows:
  process spawn + live resize, and kill + reap with no hang and no
  orphan. The third — receiving a spawned Windows process's actual
  output back through ConPTY — remains a real, open, *failing* check on
  `windows-latest`, left failing rather than skipped or marked
  `#[ignore]`, per this loop's own instruction that no test may be
  ignored to call this done. `docs/STATUS.md`'s ConPTY row stays
  `not-started`, not because nothing was learned, but because a user
  opening a real terminal on Windows right now would see a permanently
  blank pane for any command whose output they need to read — the exact
  failure golden rule #1 exists to prevent papering over.

  **Iteration 8: found the actual root cause — with the user's help
  attacking it from a different angle than another blind CI round.**
  Reading source alone had been exhausted; the user asked whether there
  was a way to approach this differently without a Windows machine, at
  all, using GitHub CI. `WebSearch` (free, no CI round-trip) found the
  exact answer within one query: `portable-pty` 0.9.0 has a known,
  reported, still-open upstream regression on Windows
  (wezterm/wezterm#6783) — reading returns nothing useful starting
  exactly at 0.9.0, while 0.8.1 worked. Reading the full issue thread
  (via `gh api`, not `WebFetch`'s lossy HTML-to-markdown summary) surfaced
  the real mechanism, explained by wezterm's own maintainer: Windows'
  ConPTY sends `ESC[6n` (a cursor position query, part of its own startup
  handshake) and **blocks all further output until it receives a
  reply** — the `PSEUDOCONSOLE_INHERIT_CURSOR` flag `portable-pty` passes
  to `CreatePseudoConsole` is what triggers this. Any real terminal
  emulator must answer that query; `lux-term`'s reader thread only ever
  *read* bytes, never *wrote* a reply — a real, universal terminal
  correctness gap (not Windows-specific in principle), just one that
  happened to never matter on Unix because the shells and `vim` this
  crate's own tests spawn don't query cursor position at startup. Several
  independent projects (`distant`, `senpi`, `deno-pty`, linked from the
  same issue thread) found and fixed the identical symptom by
  hand-detecting `ESC[6n` and writing back a *hardcoded* `ESC[1;1R`.
  `alacritty_terminal::Term` already implements this correctly and
  generically — `device_status(6)` computes the real cursor position and
  emits it as `Event::PtyWrite(text)` (confirmed by reading
  `term/mod.rs` directly) — `lux-term` was just never consuming that
  event and writing it back.

  **The fix.** `Pty::spawn`'s reader thread now drains `Term`'s event
  channel after every read and, for `Event::PtyWrite`, writes the reply
  straight back to the pty itself (matching wezterm's own real,
  production handling of this event, not the hardcoded-answer workaround
  the other projects needed because they aren't running a real VT
  emulator); every other event still republishes through the existing
  `events` channel unchanged, so `try_recv_event` keeps working the same
  way for callers. This meant `writer` had to become shareable
  (`Arc<Mutex<Box<dyn Write + Send>>>` instead of a bare
  `Box<dyn Write + Send>`) so both the reader thread and `Pty::write`
  can use it; `kill_and_wait`'s drop-order fix from iteration 4 stays
  correct since `master`, not `writer`, was always the load-bearing one
  there. Added `pty::device_status_tests::
  cursor_position_queries_get_a_real_answer`, a fast, fully portable
  unit test (`Term` + `Processor` directly, no real process, so it runs
  identically on every OS including Windows) pinning the exact mechanism
  the fix depends on: feeding `Term` a raw `ESC[6n` byte sequence and
  asserting it emits `Event::PtyWrite("\x1b[1;1R")`.

  Verified on macOS (aarch64-apple-darwin): full crate suite,
  `cargo test -p lux-term` (8 unit tests + `lanza_vim_real`, up from 7 —
  the new device-status test — all passing, 3 re-runs, 0 failures),
  `cargo clippy -p lux-term --all-targets -- -D warnings` (clean),
  `cargo fmt -p lux-term -- --check` (clean), `./scripts/check-limits.sh`
  (clean — `pty.rs` grew to 435 lines, still well under 600),
  `cargo build --workspace` (no regressions elsewhere).

  **Iteration 9: confirmed on real Windows. All three `conpty::` tests
  pass.** PR #6's `windows-latest` job (run `32119332445`, job
  `95655925891`, `12m16s`, native `cargo test --workspace` — no
  cross-compilation involved, a real Windows runner with a real MSVC
  toolchain): `conpty::resize_reaches_the_real_conpty_pseudoconsole`,
  `conpty::kill_and_wait_reaps_a_conpty_child_without_hanging`, and —
  the one that took eight iterations to actually fix —
  `conpty::spawns_a_real_process_through_conpty`, all `ok`. Every check
  on the PR is green: `cargo test` on `macos-latest`, `ubuntu-latest`,
  and `windows-latest`, `cargo clippy`, `cargo fmt --check`,
  `cargo deny check`, `lux-core` UI-isolation check, and the file/struct
  size limit check.

  **What actually closed this out, worth remembering.** Six iterations
  of source-reading, targeted logging, and elimination got the deadlock
  fixed and ruled out every plausible cause this session could reach
  through static analysis and CI round-trips alone — but the actual
  answer (a documented upstream `portable-pty` 0.9.0 regression, a known
  ConPTY startup handshake requirement, and prior art from three
  unrelated projects) was one `WebSearch` query away the whole time and
  was only tried after the user explicitly asked whether there was a way
  to attack the problem differently without a Windows machine, using
  GitHub CI. Worth promoting as a durable lesson: when a bug resists
  several rounds of pure source-reading and blind CI iteration, checking
  whether it's already a known, reported issue in the exact dependency
  version in use is a cheap, zero-round-trip step that should come
  *before* further blind guessing, not after eight iterations of it.
