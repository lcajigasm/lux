# Contributing

See `CLAUDE.md` for the ten rules. Conventional Commits, English, one
coherent change per PR. `main` always builds.

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
./scripts/check-limits.sh
```

Do not implement items in `docs/NON-GOALS.md`.
