# 0009. Search engine: embedded ripgrep crates; fuzzy matching: `nucleo` despite stagnation

## Status

Accepted.

## Context

**Find in Files / Goto Anything** need a fast, `.gitignore`-aware search
engine and a fast fuzzy matcher, both usable in-process (no shell-out, for
the same reasons as ADR 0010 on Git — shelling out to `rg` or `fzf` is not
viable on mobile and adds a process-management surface v0 already got wrong
with its terminal).

- `ignore` (0.4.31), `grep-searcher` (0.1.17), and `grep-regex` (0.1.14) are
  the actual ripgrep engine, embeddable as a library. (Correction against the
  original master-prompt table: it cited `grep-regex` "~0.4.25," a version
  that doesn't exist for this crate — likely a mix-up with the `grep` facade
  crate. Doesn't change the choice; see `docs/DEPENDENCIES.md`.)
- `nucleo` 0.5.0 is the fuzzy matcher Helix uses. It has had no release since
  April 2024, confirmed still true on re-verification. It works and has real
  adoption, but it is a maintenance risk: if it becomes a blocker, the
  project should write its own matcher rather than wait on upstream — v0
  already had a serviceable one, per the "what v0 did right" list in ADR
  0001.

## Decision

`lux-search` embeds `ignore` + `grep-searcher` + `grep-regex` for Find in
Files, and `nucleo` for fuzzy matching in Goto Anything and the command
palette. Write characterization tests around `nucleo`'s observed ranking
behavior so that a fork or a hand-written replacement is cheap if it's ever
needed, rather than discovering the coupling only at the moment it breaks.

## Alternatives considered

Shelling out to a system `rg`/`fzf` binary — rejected: unavailable on mobile,
reintroduces child-process management the project is explicitly trying to
avoid (golden rule #6, and the terminal's real PTY requirement in ADR 0012
already covers the one place child processes are legitimate). Writing a
from-scratch fuzzy matcher immediately instead of using `nucleo` — rejected
as premature; `nucleo` works today and rewriting it isn't justified until it
actually blocks something.

## Consequences

Both `grep-regex` and `nucleo`'s version numbers should be spot-checked
again before Phase 3 and Phase 4 land, per the re-verification cadence in
`docs/DEPENDENCIES.md`. If `nucleo` truly stalls further and starts failing
to build against newer Rust, forking it is preferable to blocking Goto
Anything on an upstream fix.
