# Lux

A fast, free, cross-platform text and code editor. The quality bar is
Sublime Text 3: instant startup, imperceptible input latency, flawless
handling of huge files, first-class multicursor, and a UI that never gets in
the way. Target platforms, in order: macOS, Windows, Linux, then iPadOS/iOS,
then Android.

MIT OR Apache-2.0. Free and open source. No paid tier, no telemetry by
default.

## Status

Domain crates through the phase-10 library work exist; the shipping `lux`
binary is still a thin desktop shell (sample buffer, not a full editor).
Per-platform truth is always [`docs/STATUS.md`](docs/STATUS.md). The
ordered backlog to finish product wiring is [`TODO.md`](TODO.md) (see ADR
0017). Do not trust older "phase done" wording in isolation.

## Building

    cargo build --workspace
    cargo test --workspace
    cargo xtask verify      # everything CI runs, in one command

See `rust-toolchain.toml` for the pinned toolchain.

## Repository layout

- `crates/` — the Cargo workspace (`lux-core` through `lux-ui`, plus the
  `lux` desktop binary). See `docs/ARCHITECTURE.md`.
- `docs/adr/` — architectural decisions, with context and alternatives.
- `docs/STATUS.md`, `docs/NON-GOALS.md`, `docs/PERFORMANCE.md` — what works,
  what's explicitly out of scope, and the measured performance budgets.
- `LUX_PROMPT_MAESTRO.md` — the full phase-by-phase build plan.
- `LUX_LOOP_ENGINEERING.md` — how this project is built: verified loops,
  receipts, and the memory layers that keep sessions coherent over months.

## Contributing

Read `CLAUDE.md` first — it's the working agreement, not just AI-assistant
config. Conventional Commits, English, one coherent change per commit, PRs
against `main` with CI green.
