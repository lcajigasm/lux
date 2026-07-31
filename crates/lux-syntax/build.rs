//! Compiles each vendored tree-sitter grammar's generated C sources into a
//! static lib linked into `lux-syntax`. See `docs/adr/0008-tree-sitter-grammars-compiled-in.md`
//! for why grammars are vendored and compiled in rather than pulled in as
//! independently-versioned crates.

use std::path::Path;

/// One vendored grammar: directory name under `vendor/`, and whether it
/// ships an external scanner (`scanner.c`, alongside the generated parser).
struct Grammar {
    dir: &'static str,
    has_scanner: bool,
}

const GRAMMARS: &[Grammar] = &[
    Grammar {
        dir: "rust",
        has_scanner: true,
    },
    Grammar {
        dir: "typescript",
        has_scanner: true,
    },
    Grammar {
        dir: "tsx",
        has_scanner: true,
    },
    Grammar {
        dir: "javascript",
        has_scanner: true,
    },
    Grammar {
        dir: "python",
        has_scanner: true,
    },
    Grammar {
        dir: "go",
        has_scanner: false,
    },
    Grammar {
        dir: "c",
        has_scanner: false,
    },
    Grammar {
        dir: "cpp",
        has_scanner: true,
    },
    Grammar {
        dir: "java",
        has_scanner: false,
    },
    Grammar {
        dir: "kotlin",
        has_scanner: true,
    },
    Grammar {
        dir: "swift",
        has_scanner: true,
    },
    Grammar {
        dir: "html",
        has_scanner: true,
    },
    Grammar {
        dir: "css",
        has_scanner: true,
    },
    Grammar {
        dir: "json",
        has_scanner: false,
    },
    Grammar {
        dir: "yaml",
        has_scanner: true,
    },
    Grammar {
        dir: "markdown",
        has_scanner: true,
    },
    Grammar {
        dir: "bash",
        has_scanner: true,
    },
    Grammar {
        dir: "toml",
        has_scanner: true,
    },
];

fn main() {
    for grammar in GRAMMARS {
        let src = Path::new("vendor").join(grammar.dir).join("src");
        let mut build = cc::Build::new();
        build.include(&src).std("c11").file(src.join("parser.c"));
        if grammar.has_scanner {
            build.file(src.join("scanner.c"));
        }
        build
            .warnings(false)
            .compile(&format!("tree-sitter-{}", grammar.dir));
        println!("cargo:rerun-if-changed={}", src.display());
    }
}
