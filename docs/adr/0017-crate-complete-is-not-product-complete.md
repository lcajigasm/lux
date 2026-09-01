# ADR 0017 — Crate-complete is not product-complete

- Status: accepted
- Date: 2026-09-01
- Decides: how phase "done" relates to `docs/STATUS.md` and the `lux` binary

## Context

Phases 1–10 produced real, tested domain crates (`lux-core` through
`lux-plugin`) and many `lux-ui` widgets with unit tests. `GOALS.md` marked
those phases **done**. `docs/STATUS.md`, which is ground truth per golden
rule #8, still shows most user-facing rows as `not-started` on every
platform, and the `lux` desktop binary still depends only on
`lux-core` / `lux-platform` / `lux-render` / `lux-ui` / `lux-pack` — editing
an embedded sample document.

Calling a phase "done" at the crate boundary while STATUS stays
`not-started` recreated a milder form of the v0 failure mode in ADR 0001:
a large surface that *looks* finished in the roadmap without a shippable
editor feature. The crates are not stubs (rule #1 is largely held), but
the product is not assembled.

## Decision

1. **`docs/STATUS.md` remains the only claim of what works**, per platform.
   A library that passes `cargo test -p <crate>` does **not** move a
   STATUS row unless a user can exercise that capability through the
   shipping `lux` binary (or the documented mobile host) on that platform.
2. **`GOALS.md` phase checklists may say `library-done`** when the crate
   and receipts exist. They must not say product/`STATUS` done until the
   matching STATUS cells are `implemented`.
3. **Finishing v1.0 is an integration spine**, not more parallel domain
   crates. Work proceeds by wiring existing crates into `lux` in dependency
   order (boot → files → config/keymaps → syntax → chrome/navigation →
   workspace/search → git → LSP → terminal → plugins), then proving each
   STATUS row on macOS, Windows, and Linux.
4. **No new product surface** that `docs/NON-GOALS.md` forbids. Good ideas
   still go to `docs/IDEAS.md`. Sequencing unfinished STATUS work is not a
   roadmap extension; inventing IDE features is.
5. The executable backlog for agents lives in root **`TODO.md`**, with
   one `/goal`-sized item per integration slice and explicit verify
   commands.

## Alternatives considered

- **Keep marking phases done at crate close.** Rejected: systematically
  desynchronizes GOALS from STATUS and invites "finished" narratives.
- **Delete or freeze domain crates until UI exists.** Rejected: the
  libraries are real assets; the gap is assembly, not rewrite.
- **One giant "wire everything" goal.** Rejected: unverifiable; violates
  the receipt/loop method in `LUX_LOOP_ENGINEERING.md`.

## Consequences

- Agents should pick the next open item in `TODO.md`, not re-derive a
  phase-1–10 crate build.
- PRs that only add library API without `lux` wiring leave STATUS unchanged.
- Platform columns for Windows/Linux stay `not-started` until that
  platform actually runs the capability (CI `cargo test` alone does not
  flip desktop UI rows).
