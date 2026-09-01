# Receipt: WASM plugin host (Phase 9)

- Phase: 09
- Crate(s): lux-plugin
- Goal: wasmtime Component Model host with fuel, memory limits, capability
  grants, local trusted install, three example plugins, sandbox-escape tests.
- Verify: `cargo test -p lux-plugin`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-09-01
- Closed: 2026-09-01
- Notes:

  ADR 0016 pins wasmtime 47. Component `run: func() -> u32`. Fuel kills an
  infinite loop; StoreLimits reject a 200-page memory; file read without
  FileRead does not return disk bytes. Install requires `trust.json`.
  Examples: echo, theme-lux-dark-plus, identity-fmt. API:
  `docs/plugins/API.md`. `deny.toml` allows Apache-2.0 WITH LLVM-exception.
