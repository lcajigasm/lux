# Git

Lux reads and writes Git through `lux-git`, which is `gix` in-process.
Nothing in this path shells out to the `git` binary (ADR 0010). Disk work
runs on a `lux-git` thread. The UI thread applies the result when that
thread wakes it.

| Surface | What it does | Test |
|---|---|---|
| Gutter | Markers for lines that differ from `HEAD` in the open buffer: green bar added, orange bar modified, red triangle removed. Recomputed on a worker 250 ms after edits pause, without saving | `gutter_marks_the_added_line`, `typing_in_a_tracked_file_updates_the_gutter_after_the_debounce_without_saving`, `continuous_typing_shows_marks_even_when_the_diff_is_slower_than_the_keys`, `every_gutter_mark_is_visible_on_every_builtin_gutter` |
| Status | Changed, untracked, conflicted and ignored paths in the worktree. The file tree shows `M`, `A`, `D`, `U` or `C` and colors the name; a directory takes the strongest state below it; ignored paths are dimmed. Read when a folder opens, after file-watcher batches, saves and Lux git operations, and on `Git: Refresh Status`. An untracked directory with no tracked file below it is one `U` entry, as `git status` shows `?? dir/`; every row inside it is `U`. A tracked file replaced by a directory is `D`; each file inside it is `U` | `status_lists_the_changed_file`, `a_tracked_file_replaced_by_a_directory_stays_a_deletion`, `a_staged_new_file_is_added_and_an_unstaged_one_untracked`, `an_unmerged_index_entry_is_conflicted`, `ignored_paths_collapse_to_the_ignored_directory`, `an_untracked_directory_is_reported_once`, `rows_inside_a_collapsed_untracked_directory_are_untracked`, `a_new_untracked_file_appears_in_the_tree_after_a_watcher_event`, `watcher_paths_outside_the_repository_do_not_refresh_its_status`, `opening_a_folder_reads_its_status_once_without_a_command`, `every_tree_state_is_legible_on_every_builtin_sidebar` |
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

A file refresh does not walk the worktree. Status does, on the
`lux-git-status` worker: one walk at a time, and requests that arrive
during a walk collapse into one more. A watcher batch whose paths all
sit outside the repository (another open folder) or inside ignored
directories (build output) does not start a walk. Like `git status`, the
walk trusts the index's stat cache: a tracked file whose mtime, ctime,
size, inode and owner still match its index entry is not read, and only
stat-dirty files are compared byte for byte (`only_stat_dirty_tracked_files_are_read`;
timings for a 10,000-file repository in `docs/PERFORMANCE.md`). An
ignored directory is reported once and not entered; an untracked one is
entered only until its first untracked file. The gutter diff runs on the
`lux-git-gutter` worker, which waits for edits to pause for 250 ms (1 s at
most) and diffs only the newest buffer copy; huge-file mode diffs once on
open only. A diff that finishes after newer edits were queued is still
shown, since it is the newest finished one for that file, so marks appear
during continuous typing even when one diff takes longer than the gap
between keys; they may trail the text by the edits typed meanwhile. State colors have fixed hues (green added
or untracked, orange modified, red deleted or conflicted) moved toward
the theme's contrast until they reach WCAG 4.5:1 on the sidebar and 3:1
on the gutter. Blame and history describe the committed
file, not unsaved buffer text. Staging reads the file on disk, so an
unsaved buffer is not what gets staged.
