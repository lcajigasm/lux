# Phase 8 release notes — Terminal PTY real

Headless `lux-term` (`portable-pty` + `alacritty_terminal`, ADR 0012).

## Highlights

- Real PTY (signals, resize, `vim` e2e) and ConPTY on Windows.
- Scrollback, regex search, select/copy over `alacritty_terminal`.
- Named sessions + `ShellProfile` (cwd/env/program).
- Build-error `file:line` detection as data (`SourceLocation`).
- Task panel: real kill/reap cancellation.
- VT pins, snapshot-under-flood frame proxy, orphan reaping.

Not wired into `lux-ui` / the `lux` binary (same boundary as Phases 6–7).
This capability does not exist on iOS/iPadOS (ADR 0006).
