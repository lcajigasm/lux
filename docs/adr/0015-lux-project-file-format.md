# 0015. `.lux-project` file format: plain JSON via `serde`/`serde_json`

## Status

Accepted.

## Context

Phase 4's first `lux-workspace` deliverable needs a project file,
`.lux-project`, that declares a multi-root workspace: a list of folders,
exclusion globs, and arbitrary settings. `docs/DEPENDENCIES.md` already
earmarks `serde` + JSONC for `lux-config` (Phase 5), but explicitly defers
picking a JSONC crate and exact versions to when that phase lands — it does
not commit `.lux-project` to the same choice.

Two options for `.lux-project` itself:

1. Plain JSON (`serde_json`) — no comments, no trailing commas.
2. JSONC (comments, trailing commas) — friendlier for a hand-edited file, but
   requires either an extra crate now or hand-rolling a stripping step ahead
   of `serde_json`, before Phase 5 has picked and vetted that crate.

## Decision

`.lux-project` is plain JSON, parsed and written with `serde` + `serde_json`
(1.0.229 / 1.0.151, no other dependencies pulled in — see
`docs/DEPENDENCIES.md`). No comment support.

The file is expected to be mostly machine-written (add/remove a folder from
the UI) and occasionally hand-edited; losing comments on a rewrite is an
acceptable cost for not building or half-building the JSONC decision that
belongs to Phase 5. If Phase 5 lands a JSONC parser for `lux-config`,
`.lux-project` can switch to it in a follow-up ADR — `ProjectFile`'s
`Serialize`/`Deserialize` derives make that a parser swap, not a schema
change.

## Alternatives considered

TOML — rejected: the settings field needs to hold arbitrary,
deeply-nested user data (mirroring editor settings), which reads and writes
more naturally as JSON `Value` than as TOML's table model, and every other
prospective consumer of `.lux-project` (an eventual UI, Phase 5's config
layering) will already be JSON-shaped.

Hand-rolling a minimal JSON parser instead of taking a dependency — rejected
as reinventing a solved, load-bearing format: `serde`/`serde_json` are the de
facto standard, already the documented intent for config elsewhere in this
project, and orders of magnitude better tested than anything written for
this deliverable alone.

## Consequences

`serde` and `serde_json` become the first non-domain-specific dependencies in
`lux-workspace`. Both are MIT/Apache-2.0, already-allowed licenses in
`deny.toml`; `cargo deny check` passes with no new entries needed. Re-verify
their versions before Phase 5 picks the JSONC crate for `lux-config`, in case
the two decisions want to share a `serde` version bump.
