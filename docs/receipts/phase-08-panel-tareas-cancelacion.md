# Receipt: Task panel with real cancellation (Phase 8)

- Phase: 08
- Crate(s): lux-term
- Goal: A named task runs on a real PTY; cancel kills and reaps it (ESRCH),
  and cancelling one task does not cancel another.
- Verify: `cargo test -p lux-term tasks::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  `TaskPanel` is a data panel (same idea as `lux-git::diff_panel`), not a
  wgpu widget. `cancel` is `Pty::kill_and_wait`. No simulated “cancelled”
  flag while the child keeps running.

  Verified on macOS: `sleep 30` cancelled → `kill(pid, 0)` is ESRCH; a
  sibling task stays alive. `cargo test -p lux-term tasks::` clean.
