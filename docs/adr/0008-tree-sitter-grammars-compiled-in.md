# 0008. Tree-sitter grammars compiled into the binary

## Status

Accepted.

## Context

`tree-sitter` 0.26.x has no 1.0 release. Its grammar-binding ecosystem has a
real, tracked integration cost: version conflicts between the core
`tree-sitter` crate and individually-versioned grammar crates (tracked
upstream as issue #3095), which bite hardest when grammars are loaded
dynamically or pulled in as independently-versioned dependencies that drift
out of sync with the core crate's ABI expectations.

## Decision

Compile all supported grammars directly into the `lux-syntax` binary output,
rather than loading them dynamically at runtime. Phase 3 ships with a fixed
set of languages (Rust, TypeScript/TSX, JavaScript, Python, Go, C, C++, Java,
Kotlin, Swift, HTML, CSS, JSON, YAML, Markdown, shell, TOML); adding a
language after that is a code change requiring a new release, not a
runtime-loadable plugin.

## Alternatives considered

Dynamic grammar loading (e.g., loading compiled `.so`/`.dylib`/`.wasm`
grammar objects at runtime) — rejected for Phase 3: it reopens exactly the
version-skew problem issue #3095 describes, in exchange for a flexibility
(installing a grammar without a Lux release) that isn't a stated goal for
v1.0. WASM-compiled grammars via `tree-sitter`'s WASM runtime — noted as a
possible future path if third-party grammar installation becomes a real
requirement post-1.0, but out of scope now under golden rule #7 (closed
scope) and belongs in `docs/IDEAS.md` if reconsidered.

## Consequences

Adding a seventeenth language requires a `lux-syntax` code change, a
version bump, and a release — this is an accepted tradeoff, documented so a
future contributor doesn't "fix" it into a plugin system without revisiting
this decision first. If `tree-sitter` reaches 1.0 with a resolved ABI/version
story, re-evaluate.
