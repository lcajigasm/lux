# Contributing

See `CLAUDE.md` for the ten rules. Conventional Commits, English, one
coherent change per PR. `main` always builds.

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
./scripts/check-limits.sh
```

Do not implement items in `docs/NON-GOALS.md`.

## OK gate

A pull request is allowed to merge when its head commit status `ok-gate`
is success. Luis sets that context as a required check in the repository
ruleset. This repo does not change rulesets, secrets, or settings.

Comment on the pull request conversation (not a review):

```
OK <sha>
```

`<sha>` is lowercase hex, 7 to 40 characters, and a prefix of the current
head. `OK` is case-sensitive. Another author, `ok`, extra words, or an OK
for an older push does not pass. After a new push, comment again with the
new head. Editing or deleting the comment re-evaluates it. That comment
hook runs from the workflow on `main`.

When the status turns success, the workflow queues a squash auto-merge
for that head. If the repository has not enabled auto-merge, the status
stays success and the log warns.

A merge made with `GITHUB_TOKEN` does not start `push` workflows on
`main`. To run those, add a fine-grained PAT as an Actions secret and use
it for the merge, or run the follow-up from `workflow_run`. Do not commit
the token.

## Screenshots

The website and the README show the editor only as PNGs from
`scripts/screenshots.sh`. `.github/workflows/screenshots.yml` runs that
script on `workflow_dispatch` and when a GitHub Release is published, on
the standard `macos-15` runner. It builds `lux`, opens
`docs/screenshots/example.rs`, and captures the live window. Do not add
mockups, renders, or SVG drawings of the UI. A Linux checkout can run the
same script under Xvfb when `xvfb-run` and ImageMagick are installed.
