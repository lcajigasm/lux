# 0019. Sessions live in the data directory and carry undo history

## Status

Accepted, 2026-11-25.

## Context

Hot exit (ST-TAB-03) must bring back undo history, every caret with its
selection, and the split layout, not just text and caret heads. The
session used to be written only at quit, into `.lux-session` inside the
project tree, or `~/.cache/lux/last-session.json` with no folder open.
A file in the project tree shows up in `git status`. A crash lost every
change since launch.

## Decision

**Location.** One session per project under the platform data directory:
`$XDG_DATA_HOME/lux` when set, else `~/Library/Application Support/lux`
(macOS), `%LOCALAPPDATA%\lux` (Windows), `~/.local/share/lux` (other).
The file is `sessions/<fnv64 of the project path>.json`; the folderless
window uses `sessions/global.json`. While that file does not exist, the
old `.lux-session` (or the old cache file) is read once. It is not
deleted: it is in the user's tree and may be tracked there. Recovery
snapshots keep their current location.

**Undo history.** Each tab with undo or redo steps gets one binary file in
`sessions/<key>.history/`, encoded by `lux_core::history::codec`: magic,
format version, FNV-1a hash and char length of the text it was recorded
against, the undo and redo stacks, and a trailing checksum. Decoding
rejects another version, a damaged file, or text that is not exactly that
text. A file changed on disk while Lux was closed therefore restores with
no undo, never with edits replayed at the wrong offsets. Each history is
capped at 4 MiB by dropping the oldest undo steps first. The file name is
the checksum, so one name always means one content.

**Transaction.** History files are written first, each atomically. The
session JSON is then written atomically and names only files that exist.
Only after that do files the JSON no longer names get pruned.

**When.** An edit, a key, a click, or a layout change records a timestamp.
A helper thread, started on demand, sleeps until 750 ms pass with no
change and wakes the event loop through the platform waker. The loop then
takes one snapshot: rope clones, carets, and each history behind an `Arc`
that is cloned only when its revision changed. A history's undo steps are
themselves shared, so that clone copies pointers, not edit text. The
session file is derived from the workspace root at each snapshot, never
pinned at restore, so a write always lands in the open project's session.
A reload from disk is recorded as one undo step, so a persisted history
always describes the text it is stored with. The latest-only worker that
writes recovery snapshots also writes sessions: it encodes, writes, and
prunes off the UI thread. A newer snapshot replaces one still waiting.
Quit writes on the quitting thread, after the write in flight, and marks
the session clean.

## Consequences

The project tree stays clean. A crash loses at most the last quiet period.
The debounce thread is a timer on an in-process timestamp, not a poll of
the outside world; it exits after each write, so an idle editor runs no
thread for sessions. A restored dirty tab stays dirty even if undo walks
back to the text that was on disk, because nothing proves the file still
holds that text. Restoring still reads the session and files on the UI
thread at launch, as before this change.
