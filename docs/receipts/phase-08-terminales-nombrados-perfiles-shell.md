# Receipt: Named terminals + shell profiles (Phase 8)

- Phase: 08
- Crate(s): lux-term
- Goal: Multiple named PTY sessions can run at once from a `ShellProfile`;
  killing one session does not kill another.
- Verify: `cargo test -p lux-term sessions::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  `crates/lux-term/src/sessions.rs`: `ShellProfile` (program/args/cwd/env)
  plus `Terminals` keyed by `TerminalId` and a unique display name. Spawn
  is still `Pty::spawn`. Duplicate names fail. No `lux-ui` wiring.

  Verified on macOS with real `/bin/sh`: two `sleep 30` sessions, kill
  one, `kill(pid, 0)` still succeeds on the other; cwd+env reach the
  child via `printf`. `cargo test -p lux-term sessions::` clean.
