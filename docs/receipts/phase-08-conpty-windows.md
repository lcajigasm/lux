# Receipt: ConPTY en Windows (Phase 8)

- Phase: 08
- Crate(s): lux-term
- Goal: The "ConPTY en Windows" deliverable of Phase 8 ("Terminal PTY real")
  from `LUX_PROMPT_MAESTRO.md` is complete and its gate passes with no
  failures and no ignored tests.
- Verify: `cargo test -p lux-term --target x86_64-pc-windows-msvc conpty::`
- Iterations: 3 / 10
- Result: stuck
- Opened: 2026-08-14
- Closed: 2026-08-14
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
