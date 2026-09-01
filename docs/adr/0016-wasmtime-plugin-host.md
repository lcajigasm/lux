# 0016. Plugin host: `wasmtime` 47.0.2 with the Component Model

## Status

Accepted.

## Context

Phase 9 needs sandboxed, locally installed plugins — not a marketplace
(permanent non-goal). `docs/DEPENDENCIES.md` already named `wasmtime`
47.0.2 (newer than the brief's 45.0.1; no unpatched RUSTSEC either way).
The Component Model is the versioned guest/host ABI; core modules alone
would force a private ABI we would have to throw away.

## Decision

`lux-plugin` hosts guests with `wasmtime` **47.0.2**, Component Model
enabled, Cranelift, fuel for CPU, and `StoreLimits` for memory. Capabilities
(file read/write, network, process) are explicit, user-granted, and stored
as JSON per workspace. Guests reach the host only through those imports;
a missing grant is a hard error, not a silent no-op.

No remote registry. Install is a folder/`*.wasm` copy plus a trust file.

## Alternatives considered

Core wasm only — rejected, private ABI. A JavaScript plugin runtime —
rejected, extra engine and a second sandbox story.

## Consequences

`deny.toml` must allow whatever licenses `wasmtime` 47 pulls in. Guests
are `wasm32-wasip2` components (or equivalent WAT in tests). App Store
review of WASM on iOS remains Phase 12's call (ADR 0006).
