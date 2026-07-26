---
name: add-tree-sitter-grammar
description: Add a new tree-sitter grammar to lux-syntax — compile it into the binary, wire highlight queries, and write the golden token test the gate requires.
---

# Add a tree-sitter grammar

Procedure for adding language support to `lux-syntax` (Phase 3 onward). Keep
each addition to one language per PR.

1. **Compile the grammar into the binary.** Do not depend on a published
   grammar crate whose `tree-sitter` version may drift from the workspace's
   pinned `0.26.x` (see `docs/adr/0008-tree-sitter-grammars-compiled-in.md`
   and upstream issue #3095 for why). Vendor the grammar's generated
   `parser.c` (and `scanner.c` if present) and build it with the `cc` crate
   in `lux-syntax`'s `build.rs`.
2. **Add highlight queries.** Place `highlights.scm` (and `injections.scm`,
   `locals.scm` if the language needs them) under
   `crates/lux-syntax/queries/<language>/`. Follow the capture names already
   used by the existing languages — a new, inconsistent naming scheme forks
   every theme.
3. **Register the language** in the extension-to-language table (file
   extension → grammar → default indent rules).
4. **Write the golden token test.** Add a fixture file under
   `crates/lux-syntax/tests/fixtures/<language>/` and a golden token dump it's
   compared against. A grammar that "compiles" but was never run through the
   golden test is not verified — it's the same failure mode CLAUDE.md's
   golden rule #1 exists to prevent.
5. **Gate:** `cargo test -p lux-syntax highlight:: --test golden_tokens`.

This skill is intentionally thin — expand it with real gotchas (query syntax
quirks, scanner build issues per platform) the first time one actually bites.
