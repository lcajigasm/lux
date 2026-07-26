# Lux — Working Agreement

Lux is a fast, free, cross-platform text and code editor. The quality bar is
Sublime Text 3: instant startup, imperceptible input latency, flawless handling
of huge files. Read `docs/adr/` before proposing architectural changes.

## Non-negotiable rules

1. **Nothing simulated.** A feature is either fully implemented or it does not
   exist. Never add UI (button, menu item, command, panel) for an unimplemented
   capability. Partial work goes behind an `experimental.*` flag, off by
   default, and is listed as experimental in `docs/STATUS.md`. A stub that
   returns plausible data is worse than a compile error.
2. **The core knows nothing about the UI.** `lux-core` and all domain crates
   must compile with no graphics dependency. CI enforces this.
3. **Performance budgets are tests.** See `docs/PERFORMANCE.md`. Gate them
   only on repeatable hardware; "feels fast" is not a measurement.
4. **No god objects.** No state struct over 25 fields. No file over 600 lines.
   Enforced by `scripts/check-limits.sh`.
5. **Event-driven, never polling.** No `if now - last_check > interval` to
   observe the outside world. Use FS watchers, child-process notifications, and
   channels. If you think you need polling, write an ADR.
6. **No I/O on the UI thread.** Ever.
7. **Scope is closed.** `docs/NON-GOALS.md` is binding. Good ideas go to
   `docs/IDEAS.md`, not into the codebase. Do not extend the roadmap on your
   own initiative.
8. **`docs/STATUS.md` is the source of truth** for what works, per platform. It
   is updated in the same PR that changes behavior. The README promises nothing
   STATUS does not confirm.
9. **Decisions go in ADRs.** `docs/adr/NNNN-title.md`. To reverse a decision,
   supersede it with a new ADR; never rewrite an old one.
10. **When a target is unreachable, stop and report.** Write up the finding and
    propose alternatives. This is explicitly preferred over shipping something
    that looks like it works.

## Why these rules exist

A previous attempt (`lux-edit`, Rust + egui, ~12,700 LOC) failed for specific,
documented reasons: a UI toolkit that could not reach three of the five target
platforms; a 130-field god object; no core/UI separation; a large surface of
features that were simulated rather than implemented (fake debugger call stack,
"collaboration" via a local JSON file, a one-shot `Command` posing as a
terminal, string-replace posing as semantic refactoring); and a roadmap that
chased simultaneous parity with VS Code, JetBrains, Cursor and Zed. Every rule
above maps to one of those failures. See `docs/adr/0001-lessons-from-v0.md`.

## Commands

    cargo build --workspace
    cargo test --workspace
    cargo clippy --workspace -- -D warnings
    cargo fmt --check
    cargo bench -p lux-benches          # runs the Criterion harness
    ./scripts/check-limits.sh
    cargo run -p xtask                 # everything CI runs

## Conventions

Conventional Commits, English, imperative. One coherent change per commit.
`main` always builds and passes CI; work happens on branches with PRs.
All repository artifacts — code, comments, commits, docs, issues — in English.
