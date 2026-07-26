# 0007. Text buffer on `ropey` 1.6.1, not the 2.0 beta

## Status

Accepted.

## Context

`lux-core` needs a rope data structure for the text buffer: efficient
insert/delete at arbitrary offsets, line indexing, and grapheme-aware
navigation on files up to hundreds of megabytes, without the "clone the
whole document per undo step" mistake `lux-edit` v0 made.

`ropey` 1.6.1 is the current stable release. A `2.0.0-beta.1` was published
in August 2025 but had seen no further movement as of both the original
2026-07-25 research and the 2026-07-26 re-verification for this phase — it
is not progressing toward a stable release on any visible timeline.

## Decision

Use `ropey` 1.6.1. Do not track the 2.0 beta.

## Alternatives considered

`ropey` 2.0.0-beta.1 — rejected as stalled. Hand-rolling a rope or piece-table
structure — rejected as unnecessary risk; `ropey` 1.x is proven and widely
used (Helix, among others), and v0's actual buffer bug was architectural
(whole-document snapshots for undo, see ADR 0001) rather than a `ropey`
limitation.

## Consequences

If `ropey` 2.0 ships stable later with a compelling reason to migrate, that
migration gets its own ADR superseding this one — it is not adopted
opportunistically mid-phase. `lux-core`'s public API should not leak `ropey`
types directly into its own signatures where reasonably avoidable, so that a
future migration stays contained.
