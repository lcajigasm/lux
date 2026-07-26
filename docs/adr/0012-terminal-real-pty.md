# 0012. Terminal: real PTY via `portable-pty` + `alacritty_terminal`

## Status

Accepted.

## Context

v0's "terminal" was a one-shot `std::process::Command` that captured stdout —
not a terminal at all: no persistent shell, no signals, no interactivity, no
real VT emulation. That is exactly the kind of simulated feature golden rule
#1 exists to forbid.

A real terminal needs: a persistent child shell connected via an actual
pseudo-terminal (PTY on Unix, ConPTY on Windows), correct resize/signal
handling, and a VT100/xterm-compatible emulator for scrollback, selection,
and rendering.

`portable-pty` 0.9.0 provides the cross-platform PTY layer including ConPTY
on Windows. It has had no release since 2025-02-11 (confirmed still true on
re-verification) but works and has real adoption — the same
maintenance-risk profile as `nucleo` (ADR 0009). `alacritty_terminal` 0.26.0
provides a proven VT emulator, extracted from a real terminal emulator
rather than built from scratch for this project.

## Decision

`lux-term` is built on `portable-pty` 0.9.0 (PTY + ConPTY) and
`alacritty_terminal` 0.26.0 (VT emulation). Every terminal Lux ships is a
real, persistent, signal-capable shell process — not a captured one-shot
command. Per ADR 0006, this capability does not exist at all on iOS/iPadOS,
by platform constraint, and that exclusion is reflected in `docs/STATUS.md`
rather than worked around.

## Alternatives considered

Writing a PTY layer and VT parser from scratch — rejected as high-risk,
high-effort duplication of two problems (cross-platform PTY handling,
VT100/xterm conformance) that mature, adopted crates already solve
correctly; v0's home-grown "terminal" is the cautionary example of what
happens when this surface is under-built. Continuing to shell out
one-shot commands — rejected outright, it's the exact failure this ADR
exists to correct.

## Consequences

`portable-pty`'s maintenance stagnation means Phase 8 should write tests
that pin its currently-observed behavior (resize, signal delivery, ConPTY
quirks on Windows), so that a fork is a bounded amount of work if it's ever
needed. Phase 8's acceptance criteria already require running `vim`,
`htop`, `tmux`, and a long, high-output build under this stack — the
directly observable proof that this is a real terminal, not another
simulation.
