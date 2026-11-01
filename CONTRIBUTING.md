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

## Screenshots

The website and the README show the editor only as PNGs from
`scripts/screenshots.sh`. `.github/workflows/screenshots.yml` runs that
script on `workflow_dispatch` and when a GitHub Release is published, on
the standard `macos-15` runner. It builds `lux`, opens
`docs/screenshots/example.rs`, and captures the live window. Do not add
mockups, renders, or SVG drawings of the UI. A Linux checkout can run the
same script under Xvfb when `xvfb-run` and ImageMagick are installed.
