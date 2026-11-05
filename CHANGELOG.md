# Changelog

## Unreleased

### Fixed

- Command+F and Option+Command+F open find and replace from the physical key. Shift+Command+F opens find in files. The find bar shows "n of m". Option+Command+S saves every named dirty file. The snippet picker moves to Option+Command+J. Shift+Command+T reopens a closed tab. Shift+Command+[ and Shift+Command+] switch tabs. Command+K Command+B toggles the sidebar. Command+=, Command+-, and Command+0 zoom the font. Command+, opens settings. A key repeat inserts one character and a key release inserts nothing. The command palette shows shortcuts and lists the last command first.
- Command+Shift+P opens the command palette and runs the chosen command. Command+R, Command+Shift+R, and Control+G open symbol, project symbol, and line jump. Goto Anything previews `:line`, `@symbol`, and `#word` from the keyboard. Command+click adds a cursor and Option+drag selects a column. Command+Enter and Shift+Command+Enter insert a line. Command+Y redoes. Command+E uses the word under the cursor as the find query. Typing groups undo by word.
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
