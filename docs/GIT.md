# Git

Lux reads and writes Git through `lux-git`, which is `gix` in-process.
Nothing in this path shells out to the `git` binary (ADR 0010). Disk work
runs on a `lux-git` thread. The UI thread applies the result when that
thread wakes it.

| Surface | What it does | Test |
|---|---|---|
| Gutter | Markers for lines that differ from `HEAD` in the open buffer | `gutter_marks_the_added_line` |
| Status | Changed paths in the worktree. `Git: Refresh Status` draws `M`, `A`, or `D` on that row in the file tree, and the same letter on each parent directory | `status_lists_the_changed_file`, `a_modified_file_marks_itself_and_its_parent`, `status_copies_the_mark_onto_the_sidebar` |
| Branch | The current `HEAD` name in the status bar. The click, `Git: Switch Branch`, and `Git: Create Branch` open the branch list. Enter checks out a name when the worktree is clean, or creates a branch at the current commit without rewriting files | `create_branch_keeps_a_dirty_file_and_moves_head`, `switch_branch_refuses_a_dirty_worktree`, `switch_branch_checks_out_a_clean_tree` |
| Diff | A unified diff tab of the open file against `HEAD` | `diff_shows_the_added_line` |
| Stage hunk | Writes one hunk into the index. The gutter popup and `Git: Stage Hunk` both use it | `stage_hunk_records_that_hunk` |
| Unstage hunk | Puts one staged hunk's `HEAD` lines back into the index. The file on disk stays. `Git: Unstage Hunk` uses the caret line | `unstage_hunk_returns_one_staged_edit_and_keeps_the_file` |
| Commit | Commits the index as `Lux User <lux@example.com>` | `commit_writes_the_message` |
| Blame | A tab of `short-id start+len` hunks for the file at `HEAD` | `blame_attributes_the_line` |
| History | A tab of the commits that changed the file, newest first | `history_lists_the_commit` |
| Conflicts | `<<<<<<<` blocks in the buffer. The buttons and `Git: Accept Current Change`, `Git: Accept Incoming Change`, and `Git: Accept Both Changes` resolve the block under the caret and keep that text on the tab | `resolve_conflict_replaces_text`, `accept_incoming_uses_the_caret_line` |

`the_worker_applies_gutter_markers` opens a file and waits until the
background snapshot lands on the gutter.

Palette commands: `Git: Refresh Status`, `Git: View File Diff`,
`Git: Stage Current File`, `Git: Unstage Current File`, `Git: Stage Hunk`,
`Git: Unstage Hunk`,
`Git: Commit Staged Changes`, `Git: Blame`, `Git: File History`.

A file refresh does not walk the worktree. Status does, and only when
`Git: Refresh Status` runs. The letters use the theme accent because the
theme has no separate git color. Blame and history describe the committed
file, not unsaved buffer text. Staging reads the file on disk, so an
unsaved buffer is not what gets staged.
