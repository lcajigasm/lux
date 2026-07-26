# 0010. Git backend: `gix`, in-process, no shell-out to the `git` binary

## Status

Accepted.

## Context

Lux needs real Git integration: gutter diffs, staging, commit, history,
blame, branches, stash, and merge conflict resolution (Phase 6). Two ways to
get there: shell out to the system `git` binary and parse its output, or use
an in-process library.

Shelling out is what many editors do, but it's unavailable on mobile — iOS
sandboxed apps cannot spawn child processes at all (ADR 0006), and even on
desktop it means parsing `git`'s human-oriented text output, which is
fragile and slow for UI-driving state (gutter markers on every keystroke,
for instance).

`gix` 0.86.0 (bumped from the 0.84.0 cited in the original research; a minor,
non-breaking update) is a pure-Rust, in-process Git implementation with real
adoption (7.5M+ recent downloads at time of research).

## Decision

`lux-git` is built entirely on `gix`. No shell-out to `git`, on any platform,
for any operation, including on desktop where shelling out would technically
work.

## Alternatives considered

Shell out to system `git` — rejected: doesn't work on mobile, and even on
desktop it's the wrong shape for UI state that needs to update on every
keystroke or file-watcher event (golden rule #5, event-driven, not polling
a subprocess's text output). `git2` (`libgit2` bindings) — a viable
alternative not chosen here because `gix` is pure Rust (no C toolchain
dependency to cross-compile for mobile targets) and was the version already
researched and adopted in the master prompt; revisit only with a concrete
`gix` limitation in hand.

## Consequences

If `gix` turns out not to support some required operation with sufficient
maturity, the master prompt is explicit: **do not silently shell out**.
Write up the finding, and if shelling out is genuinely the only path, that
requires its own ADR superseding this one, with the operation marked
unavailable in the mobile capability profile (ADR 0006) if it can't be done
in-process. Phase 6's acceptance criteria include testing against a real,
large repository specifically to surface this kind of gap early.
