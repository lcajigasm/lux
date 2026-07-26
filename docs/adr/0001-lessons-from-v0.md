# 0001. Lessons from v0 (`lux-edit`)

## Status

Accepted.

## Context

A previous attempt at this editor, `lux-edit` (Rust + `egui`/`eframe`,
~12,700 LOC, 24 commits, v0.0.1), was abandoned. Its post-mortem is the
reason every rule in `CLAUDE.md` exists, so it is recorded here rather than
left as oral history.

1. **The architecture could not reach the goal.** `eframe`/`egui` is
   desktop-only. iOS, iPadOS, and Android — three of five target platforms —
   were never reachable. The UI toolkit was chosen before checking it against
   the platform targets.
2. **God object.** `LuxApp` grew to ~130 fields mixing editing state, Git,
   LSP, debug, terminal, collaboration, telemetry, secrets, i18n, RTL, and
   packaging concerns. `src/app/mod.rs` was 2,170 lines. Any change risked
   regressing everything else.
3. **No core/UI separation.** There was no text engine independent of the
   UI. Editing logic was interleaved with `egui` code: not unit-testable in
   isolation, not portable, not reusable by another frontend.
4. **Simulated features presented as real.** The most damaging failure. The
   debugger's call stack was fabricated. "Real-time collaboration" was a
   local JSON file. The "terminal" ran `std::process::Command` once and
   showed stdout — not a PTY. "Semantic refactors" were word-boundary
   string replace. The "marketplace" was a placeholder. Code folding was a
   line-indentation heuristic. Notarization and code signing were comments.
   The UI had buttons for all of it, so the editor *looked* ten times more
   capable than it was.
5. **The roadmap destroyed the project.** `TODO.md` targeted simultaneous
   parity with VS Code, JetBrains, Cursor, and Zed: 150+ open tasks spanning
   collaboration, a signed marketplace, encrypted sync, RTL i18n, versioned
   telemetry, symbolized crash reports. The work cycle was "review the TODO,
   add more" — infinite breadth, zero depth. There was never a core to be
   proud of.
6. **Polling instead of events.** Git refreshed on a timer, file changes were
   detected by comparing `mtime` in a loop, the file explorer rescanned
   periodically with `rg --files` and was capped at 400 files in a flat list.

What was worth rescuing (conceptually — rewrite, don't copy): automatic
detection of indentation style, line endings, and encoding on file open;
atomic save with backup (`atomic_write_with_backup`); the recency-scored
command palette category model; and the Lux Dark visual identity
(indigo-black `#12121B` base, warm amber `#F0B442` accent).

## Decision

Every non-negotiable rule in `CLAUDE.md` maps directly to one of the six
failures above:

| Rule | Failure it prevents |
|---|---|
| #1 Nothing simulated | (4) fabricated features behind real-looking UI |
| #2 Core knows nothing about UI | (1) and (3) — toolkit lock-in, no isolation |
| #3 Performance budgets are tests | drift toward "feels fine," never measured |
| #4 No god objects | (2) the 130-field `LuxApp` |
| #5 Event-driven, never polling | (6) timer-based Git/FS/explorer refresh |
| #6 No I/O on the UI thread | a consequence of (3), UI and logic entangled |
| #7 Scope is closed | (5) the 150-task roadmap |
| #8 `STATUS.md` is the source of truth | (4) again — a place that can't lie |
| #9 Decisions go in ADRs | undocumented choices that nobody could revisit |
| #10 Stop and report | shipping "looks like it works" instead of admitting a gap |

## Alternatives considered

None — this ADR records a post-mortem, not a choice between options.

## Consequences

Every subsequent ADR in this project is expected to reference this one when
a decision exists specifically to avoid repeating one of these six failures.
If a future contributor proposes relaxing a rule in `CLAUDE.md`, they must
explain, with reference to this document, why the original failure mode no
longer applies — not just why the relaxation is convenient.
