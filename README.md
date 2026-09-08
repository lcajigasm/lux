# Lux

A fast, free, cross-platform text and code editor. The quality bar is
Sublime Text 3: instant startup, imperceptible input latency, flawless
handling of huge files, first-class multicursor, and a UI that never gets in
the way. Target platforms, in order: macOS, Windows, Linux, then iPadOS/iOS,
then Android.

MIT OR Apache-2.0. Free and open source. No paid tier, no telemetry by
default.

## Status

The desktop binary is an early editor shell with GPU text rendering,
file opening/saving, transactional keyboard editing and multicursor selection.
Domain crates contain additional
functionality, but most is not connected to the application. This is not yet
a finished editor. See [the implementation audit](docs/AUDIT.md) for confirmed
integration gaps and [the capability matrix](docs/STATUS.md) for historical
verification notes and platform limitations.

## Building

    cargo build --workspace
    cargo test --workspace
    cargo run -p xtask      # local quality checks

Run `cargo run -p lux -- path/to/file` to open a document, or omit the path
for a new document. Cmd on macOS / Ctrl on other desktops: O opens, S saves,
Shift+S saves as, N starts a new document, A selects all, D selects the next
occurrence, Z undoes, Shift+Z redoes, W requests close. Shift+arrows select.
Open/save and close confirmation have been exercised on macOS; Windows and
Linux application behavior remains unverified.

See `rust-toolchain.toml` for the pinned toolchain.

## Repository layout

- `crates/` — the Cargo workspace (`lux-core` through `lux-ui`, plus the
  `lux` desktop binary). See `docs/ARCHITECTURE.md`.
- `docs/adr/` — architectural decisions, with context and alternatives.
- `docs/STATUS.md`, `docs/NON-GOALS.md`, `docs/PERFORMANCE.md` — what works,
  what's explicitly out of scope, and the measured performance budgets.
- `GOALS.md` — the phase-by-phase roadmap.
- `docs/receipts/` — historical implementation and verification records.

## Contributing

Read `CLAUDE.md` first — it's the working agreement, not just AI-assistant
config. Conventional Commits, English, one coherent change per commit, PRs
against `main` with CI green.
