# 0005. `accesskit` wired in from Phase 2, not retrofitted

## Status

Accepted.

## Context

`accesskit` 0.24.1 is the only piece of the whole stack that already covers
all five target platforms: `accesskit_windows` 0.34, `accesskit_macos`
0.26.3, and `accesskit_unix` 0.22.1 are mature; `accesskit_android` 0.7.5 is
real (16 releases since March 2025); `accesskit_ios` 0.1.2 exists since May
2026 (experimental — treat it as such, budget time to find and report its
bugs in Phase 12). Its adapters already cover single- and multi-line text
controls, and `parley` 0.11 exposes the layout-level accessibility properties
`accesskit` needs (content, cursor, selection geometry).

Retrofitting accessibility onto an existing text editor is expensive: cursor
and selection state, focus order, and live-region updates all need to be
threaded through code that was written without them in mind. Building it in
from the first render is comparatively close to free, and it's the only
order of operations that avoids a expensive, high-risk retrofit later.

## Decision

`lux-ui`'s editor view is exposed as an `accesskit` text control — content,
cursor position, and selection — starting with the first working render in
Phase 2, verified with VoiceOver, NVDA, and Orca as a Phase 2 acceptance
criterion, not deferred to Phase 10 ("Accessibility, i18n & polish"). Phase
10 completes the picture (full keyboard navigation everywhere, contrast
audits, RTL) but does not introduce `accesskit` for the first time.

## Alternatives considered

Deferring accessibility to a dedicated later phase, matching how most
editors historically shipped it — rejected specifically because that is the
expensive-retrofit path this ADR exists to avoid, and because the tooling
cost of doing it early is low given `accesskit`'s current maturity.

## Consequences

Every future change to editor rendering or interaction must keep the
`accesskit` tree in sync as part of the same change, not as a follow-up.
`accesskit_ios`'s experimental status means Phase 12 should expect to spend
real time on upstream bug reports, possibly contributing fixes.
