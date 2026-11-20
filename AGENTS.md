# Agent contract

This file is the working contract for automated agents contributing to Lux.
Human contributors follow `CONTRIBUTING.md`. The ten product rules in
`CLAUDE.md` still apply here.

## Brief and delivery

The Notion card in the "🤖 Encargos IA" view of Lista de tareas is the only
brief and the only status. Chat messages are not a substitute brief.

Deliver by posting the full commit SHA on the card. The pull request head
(`headRefOid`, or `git ls-remote origin refs/heads/<branch>`) must match
that SHA. A claim without a matching SHA is not a delivery.

## Branches and pull requests

- Branch from `main` using a `feat/`, `fix/`, or `chore/` prefix.
- Open pull requests as **draft**. Do not mark them ready unless asked.
- Never merge to `main` or any protected branch. Luis decides merges.

## Local worktrees

On the MacStudio, work in a worktree at `~/Dev/Me/lux/.wt/<ID>`, where
`<ID>` is the encargo identifier. Do not commit in the primary checkout.

## Safety

Never use `--dangerously-skip-permissions` or `--yolo`. Never export
`ANTHROPIC_API_KEY` or `OPENAI_API_KEY`.

## Verification

Before pushing, run the checks from `CLAUDE.md` and `CONTRIBUTING.md`:

    ./scripts/pre-push.sh
    cargo fmt --check
    cargo clippy --workspace -- -D warnings
    cargo test --workspace

`./scripts/pre-push.sh` is the required pre-push gate (`cargo fmt --all
--check`, `cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace`). Do not push when it is red. CI also runs
`./scripts/check-limits.sh`.

Repository artifacts (code, comments, commits, docs, issues) are English.
Use Conventional Commits in the imperative mood, one coherent change per PR.
