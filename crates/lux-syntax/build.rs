//! Compiles each vendored tree-sitter grammar's generated C sources into a
//! static lib linked into `lux-syntax`. See `docs/adr/0008-tree-sitter-grammars-compiled-in.md`
//! for why grammars are vendored and compiled in rather than pulled in as
//! independently-versioned crates.

use std::path::Path;

/// Directory names under `vendor/`. A grammar that ships `src/scanner.c`
/// is compiled in alongside `parser.c`.
const GRAMMARS: &[&str] = &[
    "rust",
    "typescript",
    "tsx",
    "javascript",
    "python",
    "go",
    "c",
    "cpp",
    "java",
    "kotlin",
    "swift",
    "html",
    "css",
    "json",
    "yaml",
    "markdown",
    "markdown_inline",
    "bash",
    "toml",
    "php",
    "ruby",
    "sql",
    "xml",
    "dockerfile",
    "make",
    "lua",
    "c_sharp",
    "diff",
    "zig",
    "graphql",
    "ini",
    "proto",
    "hcl",
    "nix",
    "cmake",
    "gitignore",
    "scss",
    "prisma",
    "fish",
    "scheme",
    "clojure",
    "regex",
    "erlang",
    "gleam",
    "r",
    "elixir",
    "scala",
    "dart",
    "julia",
    "haskell",
    "ocaml",
    "commonlisp",
];

fn main() {
    for dir in GRAMMARS {
        let src = Path::new("vendor").join(dir).join("src");
        let mut build = cc::Build::new();
        build.include(&src).std("c11").file(src.join("parser.c"));
        let scanner = src.join("scanner.c");
        if scanner.exists() {
            build.file(scanner);
        }
        build.warnings(false).compile(&format!("tree-sitter-{dir}"));
        println!("cargo:rerun-if-changed={}", src.display());
    }
}
