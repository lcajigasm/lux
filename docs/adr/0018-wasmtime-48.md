# 0018. Plugin host stays on wasmtime, pinned to 48.0.8

## Status

Accepted. Supersedes the version pin in [ADR 0016](0016-wasmtime-plugin-host.md).
The Component Model, fuel, `StoreLimits`, and capability grants in 0016
are unchanged.

## Context

`cargo deny` on 2026-10-31 failed on wasmtime 47.0.4:

- RUSTSEC-2026-0422
- RUSTSEC-2026-0423
- RUSTSEC-2026-0424

The published fix is wasmtime >= 48.0.8, or >= 49.0.5. There is no
patched 47.x release. `lux-plugin` calls `Component`, `Linker`, `Engine`,
`Store`, and `StoreLimits` only. Those calls compile, and the existing
host tests pass, on 48.0.8.

## Decision

Pin `wasmtime` to 48.0.8 with the same features as ADR 0016: `cranelift`,
`component-model`, `runtime`, `gc-drc`, default features off. Do not
ignore the three advisories.

## Alternatives considered

Ignore the advisories in `deny.toml`. Rejected: a patched release exists
and the call sites did not need an ABI change.

Jump to 49.0.5. Rejected for this change. 48.0.8 is the smallest patched
line.

## Consequences

The lockfile moves Cranelift to 0.135.8 and the wasm-tools crates to
0.254.2. `deny.toml` still allows `Apache-2.0 WITH LLVM-exception`.
Guests stay components. ADR 0016's capability model is unchanged.
