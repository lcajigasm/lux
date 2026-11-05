# Changelog

## Unreleased

### Fixed

- Sublime macOS chords: Command+T opens Goto Anything, Control+Shift+Up/Down adds a cursor, Command+] / Command+[ indent from the keymap, and Command+Option+/ toggles a block comment. Command+/ comments HTML, CSS, and Markdown with a block comment, and JSON with `//`. Command+Left stops on the first non-space character. Control+A / Control+E move to the ends of the line on macOS.
- Enter indents from the language tree when that language has a checked body, so a brace inside a string does not add a level.

### Added

- Tree-sitter highlighting for PHP, Ruby, SQL, XML, Dockerfile, Makefile, Lua, C#, Diff, Zig, GraphQL, INI, Protocol Buffers, HCL, Nix, CMake, Gitignore, SCSS, Prisma, Fish, Scheme, Clojure, Regex, Erlang, Gleam, R, Elixir, Scala, Dart, Julia, Haskell, OCaml, and Common Lisp. File name, extension, and shebang select the grammar. Command+/ comments a line where the language has a line comment.
- Markdown inline highlighting (emphasis, strong, code spans, links) and a preview of the parsed note on Command+Shift+V (Control+Shift+V elsewhere).

Already highlighted on 1.5.1: Rust, TypeScript, TSX, JavaScript, Python, Go, C, C++, Java, Kotlin, Swift, HTML, CSS, JSON, YAML, Markdown, Bash, and TOML.

Left out: Perl, PowerShell, and LaTeX (no committed parser), Vue/Svelte/Astro (embedded host injections), JSONC and Solidity (no usable grammar repo), reStructuredText (scanner does not compile on this clang), Vim (brittle query), Sass/Less (SCSS only), OCaml interfaces (separate grammars), and Zsh (a zsh shebang still uses Bash). See `crates/lux-syntax/vendor/SOURCES.md`.

## 1.5.1 — 2026-11-05

### Fixed

- Route modal input and open a new tab with Command+N (#29).
- Keep overlay typing, the wheel, and IME off the document, and resolve shortcuts from the physical key (#31).
- Drop the Spanish dead grave that a command chord commits, so Option+Command+F leaves the document unchanged (#32).
