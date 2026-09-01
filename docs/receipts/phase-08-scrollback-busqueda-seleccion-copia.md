# Receipt: Scrollback, búsqueda, selección/copia (Phase 8)

- Phase: 08
- Crate(s): lux-term
- Goal: The "Scrollback, búsqueda, selección/copia" deliverable of Phase 8
  ("Terminal PTY real") from `LUX_PROMPT_MAESTRO.md` is complete and its
  gate passes with no failures and no ignored tests.
- Verify: `cargo test -p lux-term scrollback::`
- Iterations: 1 / 8
- Result: passed
- Opened: 2026-08-14
- Closed: 2026-08-14
- Notes:

  **Scope.** Adds `crates/lux-term/src/scrollback.rs` on top of the prior
  two Phase 8 deliverables (PTY real/signals/resize, ConPTY). No
  `lux-ui` wiring — same headless-library boundary every other receipt
  in this repo follows. Multiple named terminals/shell profiles, link
  detection, and the task panel are separate, not-yet-started
  deliverables in the same phase.

  **This is almost entirely a thin API over capability
  `alacritty_terminal` already had — reuse, not reimplementation.**
  `Config::scrolling_history` (already `10_000` by default, set since the
  first deliverable) means the VT grid was already keeping scrollback the
  whole time; nothing needed to change there. `alacritty_terminal::term::
  search` is a full DFA-compiled regex search engine
  (`RegexSearch`/`Term::search_next`) that already searches the entire
  buffer, scrollback included, not just the viewport.
  `alacritty_terminal::selection::Selection` plus
  `Term::selection_to_string` already implement real select-and-copy
  (used by alacritty itself). `scrollback.rs` wraps these three with a
  small, `Pty`-shaped surface: `Pty::scroll` (viewport navigation),
  `Pty::scrollback_position` (history depth + current offset),
  `Pty::search` (regex search across scrollback + viewport, auto-scrolls
  the match into view), `Pty::select`/`Pty::clear_selection`
  (select-and-copy). `Pty.term`'s field visibility was narrowed from
  private to `pub(crate)` (and the `ChannelListener` type it's generic
  over, likewise) so `scrollback.rs` — a sibling module, not `pty.rs`
  itself — can reach the shared `Term` without a needless accessor
  method; this is the only change to `pty.rs` beyond the bug described
  below.

  **A real, pre-existing bug the new scrollback tests surfaced:**
  `Pty::snapshot` (from the PTY-real deliverable) always rendered
  `Line(0)..Line(screen_lines)` unconditionally — correct only when the
  viewport has never scrolled (`display_offset == 0`, true for every
  prior test, since nothing scrolled it before now). `alacritty_terminal`'s
  own `display_iter` formula (`Line(-(display_offset) - 1)` as the
  iterator's start) makes clear the real top-of-viewport line is
  `Line(-display_offset)`, not always `Line(0)`. Fixed by computing that
  offset in `snapshot()` before indexing each row. No regression: when
  `display_offset == 0` the formula reduces to the exact prior behavior,
  confirmed by the full prior-deliverable suite staying green.

  **No stuck iterations.** Every test passed on the first run — the two
  design decisions that could have gone either way were resolved by
  reading `alacritty_terminal`'s own source rather than guessing, before
  writing any test: (1) `line_to_string`'s trim-to-`line_length()`
  behavior means a selection's end column can safely be generous (e.g.
  the terminal's full width) without manually computing exact content
  length — confirmed correct immediately in the selection test, no
  off-by-one chase needed; (2) `Direction::Left` searching backward from
  the live viewport's origin correctly reaches into already-scrolled-out
  history on the first attempt, matching the intuitive "history is
  behind/left of now" reading of the type (`Side`/`Direction` are the
  literal same type in `alacritty_terminal`, `pub type Side =
  Direction`).

  **Verified on macOS (aarch64-apple-darwin), real shells throughout, no
  fixtures or canned VT byte sequences.** Exact gate, `cargo test
  -p lux-term scrollback::`, re-run 4 times — 0 failures. Three tests:
  `scroll_reveals_history_the_live_viewport_no_longer_shows` (a real
  `/bin/sh` prints 100 lines into a 24-row viewport; confirms `line-1`
  has genuinely scrolled out of the live view, confirms real scrollback
  history exists via `scrollback_position()`, scrolls to `Top` and
  confirms `line-1` is visible again, scrolls to `Bottom` and confirms
  live output is back); `search_finds_a_match_that_has_scrolled_into_
  history` (a marker line printed early, then buried under 95 more lines
  so it's genuinely out of the live viewport; a real regex search finds
  it in scrollback, `select` on the returned range extracts the exact
  matched text, and the viewport is confirmed to have scrolled to reveal
  it); `select_extracts_exact_text_and_clear_selection_drops_it` (a known
  string selected end-to-end returns exactly that string;
  `clear_selection` confirmed via the underlying `Option<Selection>`
  actually becoming `None`). Full crate suite, `cargo test -p lux-term`
  (7 unit tests + `lanza_vim_real`, all passing, none ignored — the 4
  PTY-real tests and the 1 vim integration test from prior deliverables
  are unaffected by the `snapshot()` fix or the `pub(crate)` visibility
  change). Also verified: `cargo clippy -p lux-term --all-targets --
  -D warnings` (clean), `cargo fmt -p lux-term -- --check` (clean after
  one `cargo fmt` pass), `./scripts/check-limits.sh` (clean —
  `scrollback.rs` is 215 lines, `pty.rs` 320, both well under the
  600-line limit; kept as separate files specifically so `pty.rs` won't
  keep growing as later Phase 8 deliverables land), `cargo build
  --workspace` (no regressions elsewhere), and `git status --short`
  (only `crates/lux-term/*` and `docs/*` touched).
