# Phase 4 release notes — Projects, files, and global search

Commits from `7e249b0` (end of Phase 3) to `HEAD` (9 commits).

## Highlights

- **`lux-workspace`** (new crate, four modules): `project` — real
  multi-root workspaces (`Workspace`/`Root`/`RootId`, longest-prefix
  `root_for_path` resolution) and the `.lux-project` file format (plain
  JSON, `docs/adr/0015-lux-project-file-format.md`); `watcher` — native,
  fully event-driven filesystem watching (`notify` 8.2.0 +
  `notify-debouncer-full` 0.7.0), classifying changes as
  Created/Removed/Reload/Conflict, with two real macOS FSEvents
  particularities found and worked around by testing against the live
  backend, not assumed from documentation; `index` — an incremental
  workspace file index fed purely by watcher events after one baseline
  scan, never a periodic rescan; `session` — session persistence
  (folders/tabs/cursors/scroll) with a real crash-detection protocol
  (`clean_shutdown`, flipped `true` only as the last step of a normal
  quit) and a separate unsaved-buffer recovery store.
- **`lux-ui::file_tree`**: a virtualized tree explorer — lazy
  per-directory loading (no v0-style item cap), viewport-sliced row
  output, and real filesystem operations (create, rename, move — the
  drag & drop primitive — delete to the system trash via the `trash`
  crate, reveal-active-file).
- **`lux-search`**: `find_in_files`, the embedded ripgrep engine
  (`ignore` + `grep-searcher` + `grep-regex`) — real `.gitignore`-aware,
  glob-filterable, multi-threaded search streaming results through a
  channel, navigable the same way `search_bar` already is for in-buffer
  search; `replace_preview`, replace-in-project with a real diff preview
  and per-hunk/per-file confirmation — every hunk starts unaccepted, and
  `apply` re-checks each accepted hunk's original line against the
  file's current content immediately before writing it.
- **`lux-core`**: `binary_detect` (a private Phase 1 heuristic promoted
  to a real, standalone, public capability, now reading only a bounded
  file prefix instead of the whole file) and `huge_file` (a configurable
  size threshold, cheap metadata-only classification); `Buffer::
  open_with_policy` combines both into a real, queryable
  `is_huge_file_mode()` flag.
- **A real 1 GB performance budget, measured**: `find_in_files` against a
  real ~1.07 GB on-disk corpus (2,000 files, 30,000 matches), **43.6 ms**
  against the < 2 s budget — see `docs/PERFORMANCE.md`.

## Findings during close-out, not just numbers filled into a table

- **macOS FSEvents has two real, sharp particularities that would have
  shipped a wrong watcher if skipped**, both found only by testing
  against the live backend: editing a file that already existed before
  `watch()` was called is reported as another `Create`, not a `Modify`
  (fixed by seeding a `known`-paths set from a scan at `watch()` time);
  and deleting a file shortly after creating it — even across genuinely
  separate syscalls, not a same-instant race — makes FSEvents coalesce
  the whole lifecycle into one raw batch, which trips
  `notify-debouncer-full`'s own create-then-remove cancellation heuristic
  and silently drops the real `Remove` event. Fixed by treating any event
  whose path no longer exists on disk as `Removed`, regardless of the
  kind the debouncer attached to it.
- **`ignore`'s `.gitignore` support defaults to requiring an actual `.git`
  directory** — right for a `git status`-like tool, wrong for an editor
  opening a plain project folder. Both `find_in_files` and
  `lux_workspace::index` set `require_git(false)` explicitly, with the
  reasoning recorded inline, not just the flag.
- **A dependency version drifted between Phase 0's research and this
  phase's wiring, caught by re-verifying instead of trusting the old
  table**: `ignore` was cited at 0.4.31; it now resolves to 0.4.33.
  Recorded as a correction in `docs/DEPENDENCIES.md`, per that file's own
  re-verification cadence.
- **`notify`'s license (`CC0-1.0`) wasn't in `deny.toml`'s allowlist** —
  added with the reasoning (public-domain-equivalent, no copyleft) rather
  than silently widening the allowlist.
- **Read-only enforcement for huge files was deliberately not built into
  `Buffer::insert`/`delete`.** Making them silently no-op under
  `huge_file_mode` would desync `Edit::Delete`'s captured text (read
  *before* the delete happens) from what the rope actually contains after
  a no-op'd delete — a real correctness hazard in Phase 1 code this
  deliverable had no reason to touch. The flag is real and queryable;
  enforcing it against edit commands is a future editor-session layer's
  job.

## Known gaps (honest, documented, not silently deferred)

- **No Phase 4 capability is wired into `crates/lux`, the real running
  desktop binary.** Every module above is real, tested, and correct at
  the crate level, but the running app still only opens one fixed file
  with no project/folder concept — no file tree panel, no Find in Files
  UI, no session restore, nothing. This means **this release looks
  identical to `v0.2.0` in the actual running editor**, the same
  "engineering-complete, not user-visible" situation Phase 3 shipped
  with and disclosed the same way. Raised explicitly to the project
  owner before tagging; decision: document the gap and proceed, matching
  how Phase 1's RSS overrun was handled — tracked, not hidden, not
  blocking.
- **Three of the phase's four acceptance criteria were never measured,
  only architecturally argued**: "opening a 100k-file project never
  blocks the UI" (no real 100k-file tree was opened in a test), "10,000
  file changes in 10 seconds doesn't degrade frame time" (no stress test
  exists, and there is no render loop wired to any watcher yet to measure
  frame time against at all), and "opening a 1 GB file doesn't exhaust
  memory" (never measured at 1 GB; Phase 1's ~3.16× RSS overhead at
  100 MB is unchanged, neither fixed nor worsened by this phase's
  huge-file work, which only adds pre-checks before opening). Only "find
  in files over 1 GB in < 2 s" has a real number behind it. See
  `docs/STATUS.md`'s Phase 4 close-out section for the full breakdown.
- Windows and Linux remain `not-started` for the whole phase — no
  non-macOS dev environment available, same constraint as every phase
  before it.
- `Workspace::excludes()` (from `.lux-project`) still has no matching
  engine attached anywhere — the glob patterns are stored, not applied,
  a gap flagged in deliverable 1's own receipt and unchanged since.
- The `trash` crate's Windows/Linux backends, and `notify`'s
  inotify/`ReadDirectoryChangesW` backends, are entirely unexercised
  (macOS-only verification throughout).

## Commits

```
086616a docs: close out Phase 4 -- STATUS, dependency corrections, performance history
627dd30 feat(lux-core): add explicit huge and binary file handling
b127ca5 feat(lux-workspace): add sessions and crash recovery
62dd5de feat(lux-workspace): add incremental workspace index fed by watchers
eae1d98 feat(lux-search): add replace-in-project with diff preview
9625a10 feat(lux-search): add Find in Files with the embedded ripgrep engine
5357632 feat(lux-workspace): add native file watchers (notify + debouncer)
715f632 feat(lux-ui): add virtualized file tree explorer
1547618 feat(lux-workspace): add multi-root workspace model and .lux-project file
```
